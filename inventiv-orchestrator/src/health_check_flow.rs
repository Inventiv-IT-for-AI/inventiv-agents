use std::net::TcpStream;
use std::time::Duration as StdDuration;

use sqlx::{Pool, Postgres};

use crate::state_machine;
use crate::logger;

async fn check_instance_readyz_http(ip: &str, port: u16) -> bool {
    let clean_ip = ip.split('/').next().unwrap_or(ip);
    let url = format!("http://{}:{}/readyz", clean_ip, port);
    // Use short timeout to avoid stalling the job loop
    let client = reqwest::Client::builder()
        .connect_timeout(StdDuration::from_secs(2))
        .timeout(StdDuration::from_secs(3))
        .build();
    let Ok(client) = client else {
        return false;
    };
    match client.get(url).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Check instance health by testing SSH port connectivity.
async fn check_instance_ssh(ip: &str) -> bool {
    let clean_ip = ip.split('/').next().unwrap_or(ip);
    let addr = format!("{}:22", clean_ip);

    tokio::task::spawn_blocking(move || {
        let socket_addr = match addr.parse() {
            Ok(a) => a,
            Err(_) => return false,
        };
        TcpStream::connect_timeout(&socket_addr, StdDuration::from_secs(3)).is_ok()
    })
    .await
    .unwrap_or(false)
}

/// Health-check flow for BOOTING instances (probe + call state-machine transitions).
pub async fn check_and_transition_instance(
    instance_id: uuid::Uuid,
    ip: Option<String>,
    created_at: sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>,
    failures: i32,
    db: Pool<Postgres>,
) {
    // Mock provider: no real SSH. We emulate a successful startup so the rest of the platform can be tested.
    let provider_code: Option<String> = sqlx::query_scalar(
        "SELECT p.code FROM instances i JOIN providers p ON p.id = i.provider_id WHERE i.id = $1",
    )
    .bind(instance_id)
    .fetch_optional(&db)
    .await
    .ok()
    .flatten();

    let ip = match ip {
        Some(ip) => ip,
        None => {
            println!("⚠️  Instance {} has no IP, skipping health check", instance_id);
            return;
        }
    };

    if provider_code.as_deref() == Some("mock") {
        println!("✅ Instance {} is on mock provider: auto-ready", instance_id);
        let hc_start = std::time::Instant::now();
        let log_id = logger::log_event_with_metadata(
            &db,
            "HEALTH_CHECK",
            "in_progress",
            instance_id,
            None,
            Some(serde_json::json!({"ip": ip, "result": "success", "failures": failures, "mode": "mock"})),
        )
        .await
        .ok();
        let _ = state_machine::booting_to_ready(&db, instance_id, "Mock provider auto-ready").await;
        if let Some(lid) = log_id {
            let dur = hc_start.elapsed().as_millis() as i32;
            let _ = logger::log_event_complete(&db, lid, "success", dur, None).await;
        }
        return;
    }

    // Timeout after 5 minutes
    let age = sqlx::types::chrono::Utc::now() - created_at;
    if age.num_seconds() > 300 {
        println!(
            "⏱️  Instance {} timeout exceeded ({}s), marking as startup_failed",
            instance_id,
            age.num_seconds()
        );
        let _ = state_machine::booting_to_startup_failed(
            &db,
            instance_id,
            "STARTUP_TIMEOUT",
            "Instance failed to become healthy within 5 minutes",
        )
        .await;
        return;
    }

    // Prefer worker readiness endpoint when available; fallback to SSH to keep backward-compat.
    let worker_port: u16 = std::env::var("WORKER_HEALTH_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(8080);

    let is_ready_http = check_instance_readyz_http(&ip, worker_port).await;
    let is_healthy = if is_ready_http {
        true
    } else {
        check_instance_ssh(&ip).await
    };

    if is_healthy {
        println!("✅ Instance {} health check PASSED! Transitioning to ready", instance_id);
        let hc_start = std::time::Instant::now();
        let log_id = logger::log_event_with_metadata(
            &db,
            "HEALTH_CHECK",
            "in_progress",
            instance_id,
            None,
            Some(serde_json::json!({
                "ip": ip,
                "result": "success",
                "failures": failures,
                "mode": if is_ready_http { "worker_readyz" } else { "ssh_22" },
                "worker_health_port": worker_port
            })),
        ).await.ok();
        let _ = state_machine::booting_to_ready(&db, instance_id, "Health check passed").await;
        if let Some(lid) = log_id {
            let dur = hc_start.elapsed().as_millis() as i32;
            let _ = logger::log_event_complete(&db, lid, "success", dur, None).await;
        }
    } else {
        let new_failures = failures + 1;
        println!(
            "❌ Instance {} health check FAILED (attempt {}/30)",
            instance_id, new_failures
        );

        let hc_start = std::time::Instant::now();
        let log_id = logger::log_event_with_metadata(
            &db,
            "HEALTH_CHECK",
            "in_progress",
            instance_id,
            None,
            Some(serde_json::json!({
                "ip": ip,
                "result": "failed",
                "failures": new_failures,
                "mode": "ssh_22_or_worker_readyz",
                "worker_health_port": worker_port
            })),
        ).await.ok();

        let _ = state_machine::update_booting_health_failures(&db, instance_id, new_failures).await;

        if let Some(lid) = log_id {
            let dur = hc_start.elapsed().as_millis() as i32;
            let _ = logger::log_event_complete(&db, lid, "failed", dur, Some("Worker readyz not reachable and SSH port 22 not reachable")).await;
        }

        if new_failures >= 30 {
            println!(
                "❌ Instance {} exceeded max health check retries, marking as startup_failed",
                instance_id
            );
            let _ = state_machine::booting_to_startup_failed(
                &db,
                instance_id,
                "HEALTH_CHECK_FAILED",
                "Instance failed health checks after 30 attempts",
            )
            .await;
        }
    }
}

