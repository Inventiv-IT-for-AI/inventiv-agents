from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os


MODEL_ID = os.getenv("MODEL_ID", "demo-model")
PORT = int(os.getenv("PORT", "8000"))
REQUESTS_WAITING = float(os.getenv("VLLM_NUM_REQUESTS_WAITING", os.getenv("MOCK_VLLM_REQUESTS_WAITING", "0")))
REQUESTS_RUNNING = float(os.getenv("VLLM_NUM_REQUESTS_RUNNING", os.getenv("MOCK_VLLM_REQUESTS_RUNNING", "0")))


class Handler(BaseHTTPRequestHandler):
    def handle_one_request(self):
        """Override to catch ValueError when connection is closed during flush."""
        try:
            super().handle_one_request()
        except (ValueError, BrokenPipeError, OSError) as e:
            # Connection closed by client during/after response - this is normal for SSE
            if "closed file" not in str(e).lower() and "broken pipe" not in str(e).lower():
                # Only log unexpected errors
                import sys
                print(f"Unexpected error: {e}", file=sys.stderr)
    
    def _json(self, code: int, payload: dict):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        try:
            self.wfile.write(body)
            self.wfile.flush()
        except (BrokenPipeError, OSError, ValueError):
            pass

    def _text(self, code: int, body: str, content_type: str = "text/plain; version=0.0.4"):
        raw = body.encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def _read_json(self):
        try:
            ln = int(self.headers.get("Content-Length") or "0")
        except Exception:
            ln = 0
        if ln <= 0:
            return {}
        try:
            raw = self.rfile.read(ln)
            return json.loads(raw.decode("utf-8") or "{}")
        except Exception:
            return {}

    def do_GET(self):
        if self.path == "/v1/models":
            self._json(
                200,
                {"object": "list", "data": [{"id": MODEL_ID, "object": "model"}]},
            )
            return
        if self.path == "/metrics":
            # Minimal Prometheus exposition compatible with the worker-agent parser.
            body = "\n".join(
                [
                    "# HELP vllm_num_requests_waiting Number of requests waiting in queue.",
                    "# TYPE vllm_num_requests_waiting gauge",
                    f"vllm_num_requests_waiting {REQUESTS_WAITING}",
                    "# HELP vllm_num_requests_running Number of requests currently running.",
                    "# TYPE vllm_num_requests_running gauge",
                    f"vllm_num_requests_running {REQUESTS_RUNNING}",
                    "",
                ]
            )
            self._text(200, body)
            return
        if self.path in ("/health", "/healthz"):
            self._json(200, {"status": "ok"})
            return
        self._json(404, {"error": "not_found"})

    def do_POST(self):
        # Minimal OpenAI-compatible responses so /v1/chat/completions can be tested end-to-end.
        if self.path == "/v1/chat/completions":
            import time
            request_start = time.time()
            request_id = f"{int(time.time() * 1000)}-{id(self) % 10000}"
            
            data = self._read_json()
            model = (data.get("model") or MODEL_ID) if isinstance(data, dict) else MODEL_ID
            user_msg = ""
            try:
                msgs = (data.get("messages") or []) if isinstance(data, dict) else []
                if msgs and isinstance(msgs, list):
                    last = msgs[-1]
                    if isinstance(last, dict):
                        user_msg = str(last.get("content") or "")
            except Exception:
                pass
            
            stream = data.get("stream") if isinstance(data, dict) else False
            content = f"mock-vllm ok (echo): {user_msg[:200]}"
            
            # Estimate tokens: roughly 1 token per 4 characters (approximation)
            prompt_tokens = max(1, len(user_msg) // 4) if user_msg else 1
            completion_tokens = max(1, len(content) // 4)
            total_tokens = prompt_tokens + completion_tokens
            
            print(f"[MOCK_VLLM] [{request_id}] REQUEST_START: path={self.path}, model={model}, stream={stream}, msg_length={len(user_msg)}, tokens={prompt_tokens}/{completion_tokens}/{total_tokens}", flush=True)
            
            if stream:
                # SSE streaming response
                print(f"[MOCK_VLLM] [{request_id}] STREAM_START: sending SSE response", flush=True)
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Cache-Control", "no-cache")
                # Use keep-alive initially, then let HTTP/1.0 close connection naturally after [DONE]
                # Don't set Connection header - let BaseHTTPRequestHandler handle it
                self.end_headers()
                
                # Send initial delta
                chunk = {
                    "id": "chatcmpl-mock",
                    "object": "chat.completion.chunk",
                    "model": model,
                    "choices": [{"index": 0, "delta": {"role": "assistant", "content": content}, "finish_reason": None}],
                }
                try:
                    chunk_data = f"data: {json.dumps(chunk)}\n\n".encode("utf-8")
                    self.wfile.write(chunk_data)
                    self.wfile.flush()
                    print(f"[MOCK_VLLM] [{request_id}] STREAM_CHUNK_1: sent initial delta, size={len(chunk_data)}", flush=True)
                except (BrokenPipeError, OSError, ValueError) as e:
                    print(f"[MOCK_VLLM] [{request_id}] STREAM_ERROR: failed to send initial chunk: {e}", flush=True)
                    return
                
                # Send final chunk with usage tokens
                final_chunk = {
                    "id": "chatcmpl-mock",
                    "object": "chat.completion.chunk",
                    "model": model,
                    "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
                    "usage": {"prompt_tokens": prompt_tokens, "completion_tokens": completion_tokens, "total_tokens": total_tokens},
                }
                try:
                    final_data = f"data: {json.dumps(final_chunk)}\n\n".encode("utf-8")
                    done_data = b"data: [DONE]\n\n"
                    self.wfile.write(final_data)
                    self.wfile.write(done_data)
                    self.wfile.flush()
                    # Let BaseHTTPRequestHandler close the connection naturally
                    # Don't call wfile.close() as it can cause issues with reqwest reading headers
                    elapsed = (time.time() - request_start) * 1000
                    print(f"[MOCK_VLLM] [{request_id}] STREAM_COMPLETE: sent final chunk + [DONE], elapsed_ms={elapsed:.1f}", flush=True)
                except (BrokenPipeError, OSError, ValueError) as e:
                    elapsed = (time.time() - request_start) * 1000
                    print(f"[MOCK_VLLM] [{request_id}] STREAM_ERROR: failed to send final chunk: {e}, elapsed_ms={elapsed:.1f}", flush=True)
                    pass
                return
            else:
                # Non-streaming JSON response
                print(f"[MOCK_VLLM] [{request_id}] NON_STREAM: sending JSON response", flush=True)
                self._json(
                    200,
                    {
                        "id": "chatcmpl-mock",
                        "object": "chat.completion",
                        "model": model,
                        "choices": [
                            {
                                "index": 0,
                                "message": {
                                    "role": "assistant",
                                    "content": content,
                                },
                                "finish_reason": "stop",
                            }
                        ],
                        "usage": {"prompt_tokens": prompt_tokens, "completion_tokens": completion_tokens, "total_tokens": total_tokens},
                    },
                )
                elapsed = (time.time() - request_start) * 1000
                print(f"[MOCK_VLLM] [{request_id}] REQUEST_COMPLETE: elapsed_ms={elapsed:.1f}", flush=True)
            return

        if self.path == "/v1/completions":
            data = self._read_json()
            model = (data.get("model") or MODEL_ID) if isinstance(data, dict) else MODEL_ID
            prompt = str((data.get("prompt") or "")) if isinstance(data, dict) else ""
            completion_text = f"mock-vllm ok: {prompt[:200]}"
            comp_prompt_tokens = max(1, len(prompt) // 4) if prompt else 1
            comp_completion_tokens = max(1, len(completion_text) // 4)
            comp_total_tokens = comp_prompt_tokens + comp_completion_tokens
            self._json(
                200,
                {
                    "id": "cmpl-mock",
                    "object": "text_completion",
                    "model": model,
                    "choices": [{"index": 0, "text": completion_text, "finish_reason": "stop"}],
                    "usage": {"prompt_tokens": comp_prompt_tokens, "completion_tokens": comp_completion_tokens, "total_tokens": comp_total_tokens},
                },
            )
            return

        if self.path == "/v1/embeddings":
            data = self._read_json()
            model = (data.get("model") or MODEL_ID) if isinstance(data, dict) else MODEL_ID
            input_text = ""
            try:
                input_data = (data.get("input") or "") if isinstance(data, dict) else ""
                if isinstance(input_data, str):
                    input_text = input_data
                elif isinstance(input_data, list):
                    input_text = " ".join(str(x) for x in input_data)
            except Exception:
                pass
            emb_prompt_tokens = max(1, len(input_text) // 4) if input_text else 1
            self._json(
                200,
                {
                    "object": "list",
                    "model": model,
                    "data": [{"object": "embedding", "index": 0, "embedding": [0.0, 0.0, 0.0]}],
                    "usage": {"prompt_tokens": emb_prompt_tokens, "total_tokens": emb_prompt_tokens},
                },
            )
            return

        self._json(404, {"error": "not_found"})


if __name__ == "__main__":
    print(f"mock-vllm listening on 0.0.0.0:{PORT} model_id={MODEL_ID}", flush=True)
    HTTPServer(("0.0.0.0", PORT), Handler).serve_forever()


