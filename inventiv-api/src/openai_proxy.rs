use axum::body::{Body, Bytes};
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::StreamExt;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth;
use crate::metrics;
use crate::simple_logger;
use crate::worker_routing;
use crate::AppState;

/// Proxy OpenAI-compatible requests to workers
pub async fn proxy_to_worker(
    state: &Arc<AppState>,
    path: &str,
    headers: HeaderMap,
    body: Bytes,
    user: Option<auth::AuthUser>,
) -> Response {
    // Generate correlation ID for end-to-end tracing
    let correlation_id = uuid::Uuid::new_v4().to_string();
    eprintln!(
        "[OPENAI_PROXY] [{}] START: path={}, body_size={}",
        correlation_id,
        path,
        body.len()
    );

    let v: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "[OPENAI_PROXY] [{}] ERROR: Invalid JSON body: {}",
                correlation_id, e
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"invalid_json"})),
            )
                .into_response();
        }
    };
    let requested_model = v.get("model").and_then(|m| m.as_str());
    eprintln!(
        "[OPENAI_PROXY] [{}] REQUEST: model={:?}, stream={}",
        correlation_id,
        requested_model,
        v.get("stream").and_then(|b| b.as_bool()).unwrap_or(false)
    );

    let model_id =
        match worker_routing::resolve_openai_model_id(&state.db, requested_model, user.as_ref())
            .await
        {
            Ok(m) => {
                eprintln!(
                    "[OPENAI_PROXY] [{}] MODEL_RESOLVED: model_id={}",
                    correlation_id, m
                );
                m
            }
            Err(e) => {
                eprintln!(
                    "[OPENAI_PROXY] [{}] ERROR: Model resolution failed",
                    correlation_id
                );
                return e.into_response();
            }
        };
    let stream = v.get("stream").and_then(|b| b.as_bool()).unwrap_or(false);

    // Sticky key: user-provided; forwarded to worker-local HAProxy to keep affinity in multi-vLLM mode.
    let sticky = worker_routing::header_value(&headers, "X-Inventiv-Session");

    let Some((instance_id, base_url)) =
        worker_routing::select_ready_worker_for_model(&state.db, &model_id, sticky.as_deref())
            .await
    else {
        eprintln!(
            "[OPENAI_PROXY] [{}] ERROR: No ready worker found for model_id={}",
            correlation_id, model_id
        );
        worker_routing::bump_runtime_model_counters(&state.db, &model_id, false).await;
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "error":"no_ready_worker",
                "message":"No READY worker found for requested model",
                "model": model_id
            })),
        )
            .into_response();
    };

    let target = format!("{}{}", base_url.trim_end_matches('/'), path);
    eprintln!(
        "[OPENAI_PROXY] [{}] WORKER_SELECTED: instance_id={}, target={}, stream={}",
        correlation_id, instance_id, target, stream
    );

    // Build HTTP client with appropriate timeouts
    let mut client_builder = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .tcp_keepalive(std::time::Duration::from_secs(60))
        .read_timeout(std::time::Duration::from_secs(300)); // 5 minutes for reading

    if stream {
        client_builder = client_builder.timeout(std::time::Duration::from_secs(3600));
    } else {
        client_builder = client_builder.timeout(std::time::Duration::from_secs(60));
    }

    let client = match client_builder.build() {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":"http_client_build_failed"})),
            )
                .into_response();
        }
    };

    // Prepare headers for upstream request
    let mut out_headers = reqwest::header::HeaderMap::new();
    if let Some(ct) = headers.get(axum::http::header::CONTENT_TYPE) {
        out_headers.insert(reqwest::header::CONTENT_TYPE, ct.clone());
    } else {
        out_headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
    }
    if let Some(acc) = headers.get(axum::http::header::ACCEPT) {
        out_headers.insert(reqwest::header::ACCEPT, acc.clone());
    } else {
        out_headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
    }
    if let Some(sid) = sticky.as_deref() {
        if let Ok(val) = reqwest::header::HeaderValue::from_str(sid) {
            out_headers.insert(
                reqwest::header::HeaderName::from_static("x-inventiv-session"),
                val,
            );
        }
    }

    // Send request to worker
    eprintln!(
        "[OPENAI_PROXY] [{}] UPSTREAM_REQUEST: sending POST to {}",
        correlation_id, target
    );
    let start_time = std::time::Instant::now();
    let upstream = match client
        .post(&target)
        .headers(out_headers)
        .body(body)
        .send()
        .await
    {
        Ok(r) => {
            let elapsed = start_time.elapsed();
            eprintln!(
                "[OPENAI_PROXY] [{}] UPSTREAM_RESPONSE: status={}, elapsed_ms={}",
                correlation_id,
                r.status(),
                elapsed.as_millis()
            );
            r
        }
        Err(e) => {
            let elapsed = start_time.elapsed();
            eprintln!("[OPENAI_PROXY] [{}] UPSTREAM_ERROR: elapsed_ms={}, error={}, is_timeout={}, is_connect={}", 
                correlation_id, elapsed.as_millis(), e, e.is_timeout(), e.is_connect());
            worker_routing::bump_runtime_model_counters(&state.db, &model_id, false).await;
            metrics::update_instance_request_metrics(
                &state.db,
                instance_id,
                false,
                None,
                None,
                None,
            )
            .await;
            let error_msg = if e.is_timeout() {
                "Worker request timeout"
            } else if e.is_connect() {
                "Cannot connect to worker"
            } else {
                "Worker request failed"
            };
            let _ = simple_logger::log_action_with_metadata(
                &state.db,
                "OPENAI_PROXY",
                "failed",
                Some(instance_id),
                Some("upstream_request_failed"),
                Some(json!({"target": target, "error": e.to_string(), "correlation_id": correlation_id})),
            )
            .await;
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error":"upstream_unreachable","message":error_msg})),
            )
                .into_response();
        }
    };

    let status = upstream.status();
    let mut resp_headers = axum::http::HeaderMap::new();
    // Preserve content-type for SSE streaming.
    if let Some(ct) = upstream.headers().get(reqwest::header::CONTENT_TYPE) {
        if let Ok(cts) = ct.to_str() {
            if let Ok(v) = axum::http::HeaderValue::from_str(cts) {
                resp_headers.insert(axum::http::header::CONTENT_TYPE, v);
            }
        }
    }

    if stream {
        handle_streaming_response(
            state,
            upstream,
            status,
            resp_headers,
            instance_id,
            &model_id,
            &correlation_id,
            user.as_ref(),
        )
        .await
    } else {
        handle_non_streaming_response(
            state,
            upstream,
            status,
            resp_headers,
            instance_id,
            &model_id,
            &correlation_id,
            user.as_ref(),
        )
        .await
    }
}

async fn handle_streaming_response(
    state: &Arc<AppState>,
    upstream: reqwest::Response,
    status: StatusCode,
    resp_headers: axum::http::HeaderMap,
    instance_id: Uuid,
    model_id: &str,
    correlation_id: &str,
    user: Option<&auth::AuthUser>,
) -> Response {
    eprintln!(
        "[OPENAI_PROXY] [{}] STREAMING_START: status={}, content_type={:?}",
        correlation_id,
        status,
        resp_headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
    );

    let success = status.is_success();
    worker_routing::bump_runtime_model_counters(&state.db, model_id, success).await;
    metrics::update_instance_request_metrics(&state.db, instance_id, success, None, None, None)
        .await;

    let correlation_id_for_stream = correlation_id.to_string();
    let correlation_id_for_tokens = correlation_id.to_string();
    let instance_id_for_tokens = instance_id;
    let model_id_for_tokens = model_id.to_string();
    let db_for_tokens = state.db.clone();
    let user_for_tokens = user.cloned();
    let chunk_count = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let chunk_count_clone = chunk_count.clone();

    // Use a channel to collect chunks for token extraction
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let tx_arc = std::sync::Arc::new(tx);

    // Spawn task to collect chunks and extract tokens
    tokio::spawn(async move {
        let mut buffer = Vec::<u8>::new();
        while let Some(chunk) = rx.recv().await {
            buffer.extend_from_slice(&chunk);
        }

        // Stream completed (channel closed), extract tokens
        let text = String::from_utf8_lossy(&buffer);
        eprintln!(
            "[OPENAI_PROXY] [{}] STREAM_BUFFER_SIZE: {} bytes",
            correlation_id_for_tokens,
            buffer.len()
        );
        if !buffer.is_empty() && buffer.len() < 2000 {
            eprintln!(
                "[OPENAI_PROXY] [{}] STREAM_BUFFER_PREVIEW: {}",
                correlation_id_for_tokens,
                text.chars().take(500).collect::<String>()
            );
        }
        let (input_tokens, output_tokens, total_tokens) =
            metrics::parse_tokens_from_sse_stream(&text);

        if input_tokens.is_some() || output_tokens.is_some() || total_tokens.is_some() {
            eprintln!(
                "[OPENAI_PROXY] [{}] STREAM_TOKENS_EXTRACTED: input={:?}, output={:?}, total={:?}",
                correlation_id_for_tokens, input_tokens, output_tokens, total_tokens
            );

            // Update instance metrics with tokens
            metrics::update_instance_request_metrics(
                &db_for_tokens,
                instance_id_for_tokens,
                success,
                input_tokens,
                output_tokens,
                total_tokens,
            )
            .await;

            // Store inference usage
            if let Some(model_uuid) =
                metrics::resolve_model_uuid(&db_for_tokens, &model_id_for_tokens).await
            {
                metrics::store_inference_usage(
                    &db_for_tokens,
                    instance_id_for_tokens,
                    model_uuid,
                    input_tokens,
                    output_tokens,
                    total_tokens,
                    None,
                    user_for_tokens.as_ref(),
                )
                .await;
            }
        } else {
            eprintln!("[OPENAI_PROXY] [{}] STREAM_TOKENS_NOT_FOUND: no usage found in stream (buffer_len={})", 
                correlation_id_for_tokens, buffer.len());
        }
    });

    // Collect all chunks, forward them, and close channel when stream ends
    let tx_for_stream = tx_arc.clone();
    let tx_for_close = tx_arc.clone();
    let correlation_id_for_fold = correlation_id_for_stream.clone();
    let chunk_count_for_fold = chunk_count_clone.clone();

    // Use fold to collect all chunks and detect stream end
    // fold returns a Future, so we wrap it in a stream and flatten
    let byte_stream = futures_util::stream::once(async move {
        let (results, _tx) = upstream
            .bytes_stream()
            .fold(
                (Vec::<Result<Bytes, std::io::Error>>::new(), tx_for_stream),
                move |(mut results, tx), chunk_result| {
                    let correlation_id_clone = correlation_id_for_fold.clone();
                    let chunk_count_clone = chunk_count_for_fold.clone();
                    async move {
                        match &chunk_result {
                            Ok(bytes) => {
                                let count = chunk_count_clone
                                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                                    + 1;
                                if count.is_multiple_of(10) || count <= 3 {
                                    eprintln!(
                                        "[OPENAI_PROXY] [{}] STREAM_CHUNK: count={}, size={}",
                                        correlation_id_clone,
                                        count,
                                        bytes.len()
                                    );
                                }
                                // Send chunk to token extraction task
                                let _ = tx.send(bytes.to_vec());
                                results.push(Ok(Bytes::copy_from_slice(bytes)));
                            }
                            Err(e) => {
                                let count =
                                    chunk_count_clone.load(std::sync::atomic::Ordering::Relaxed);
                                eprintln!(
                                    "[OPENAI_PROXY] [{}] STREAM_ERROR: chunk_count={}, error={:?}",
                                    correlation_id_clone, count, e
                                );
                                results.push(Err(std::io::Error::other("upstream_stream_error")));
                            }
                        }
                        (results, tx)
                    }
                },
            )
            .await;

        // Stream completed, close channel to trigger token extraction
        eprintln!(
            "[OPENAI_PROXY] [{}] STREAM_END: closing channel, collected {} chunks",
            correlation_id_for_stream,
            results.len()
        );
        drop(tx_for_close);
        // Return results as a stream
        futures_util::stream::iter(results)
    })
    .flatten();

    eprintln!(
        "[OPENAI_PROXY] [{}] STREAMING_RETURN: returning stream to client",
        correlation_id
    );
    (status, resp_headers, Body::from_stream(byte_stream)).into_response()
}

async fn handle_non_streaming_response(
    state: &Arc<AppState>,
    upstream: reqwest::Response,
    status: StatusCode,
    resp_headers: axum::http::HeaderMap,
    instance_id: Uuid,
    model_id: &str,
    correlation_id: &str,
    user: Option<&auth::AuthUser>,
) -> Response {
    eprintln!(
        "[OPENAI_PROXY] [{}] NON_STREAMING: reading response body",
        correlation_id
    );
    let bytes = match upstream.bytes().await {
        Ok(b) => {
            eprintln!(
                "[OPENAI_PROXY] [{}] NON_STREAMING_SUCCESS: body_size={}",
                correlation_id,
                b.len()
            );
            b
        }
        Err(e) => {
            eprintln!(
                "[OPENAI_PROXY] [{}] NON_STREAMING_ERROR: {}",
                correlation_id, e
            );
            let success = false;
            worker_routing::bump_runtime_model_counters(&state.db, model_id, success).await;
            metrics::update_instance_request_metrics(
                &state.db,
                instance_id,
                success,
                None,
                None,
                None,
            )
            .await;
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error":"upstream_read_failed","message":e.to_string()})),
            )
                .into_response();
        }
    };

    let success = status.is_success();
    worker_routing::bump_runtime_model_counters(&state.db, model_id, success).await;

    // Extract tokens from response JSON
    let (input_tokens, output_tokens, total_tokens) = if success {
        match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(json) => {
                let tokens = metrics::extract_token_usage(&json);
                eprintln!(
                    "[OPENAI_PROXY] [{}] TOKENS_EXTRACTED: input={:?}, output={:?}, total={:?}",
                    correlation_id, tokens.0, tokens.1, tokens.2
                );
                tokens
            }
            Err(e) => {
                eprintln!(
                    "[OPENAI_PROXY] [{}] TOKEN_EXTRACTION_ERROR: failed to parse JSON: {}",
                    correlation_id, e
                );
                (None, None, None)
            }
        }
    } else {
        (None, None, None)
    };

    // Update instance metrics with tokens
    metrics::update_instance_request_metrics(
        &state.db,
        instance_id,
        success,
        input_tokens,
        output_tokens,
        total_tokens,
    )
    .await;

    // Store inference usage if we have tokens
    if success && (input_tokens.is_some() || output_tokens.is_some() || total_tokens.is_some()) {
        if let Some(model_uuid) = metrics::resolve_model_uuid(&state.db, model_id).await {
            metrics::store_inference_usage(
                &state.db,
                instance_id,
                model_uuid,
                input_tokens,
                output_tokens,
                total_tokens,
                None, // API key ID - can be enhanced later
                user,
            )
            .await;
        }
    }

    (status, resp_headers, bytes).into_response()
}
