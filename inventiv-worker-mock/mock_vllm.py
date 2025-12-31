from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os


MODEL_ID = os.getenv("MODEL_ID", "demo-model")
PORT = int(os.getenv("PORT", "8000"))
REQUESTS_WAITING = float(os.getenv("VLLM_NUM_REQUESTS_WAITING", os.getenv("MOCK_VLLM_REQUESTS_WAITING", "0")))
REQUESTS_RUNNING = float(os.getenv("VLLM_NUM_REQUESTS_RUNNING", os.getenv("MOCK_VLLM_REQUESTS_RUNNING", "0")))


class Handler(BaseHTTPRequestHandler):
    def _json(self, code: int, payload: dict):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

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
                                "content": f"mock-vllm ok (echo): {user_msg[:200]}",
                            },
                            "finish_reason": "stop",
                        }
                    ],
                    "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
                },
            )
            return

        if self.path == "/v1/completions":
            data = self._read_json()
            model = (data.get("model") or MODEL_ID) if isinstance(data, dict) else MODEL_ID
            prompt = str((data.get("prompt") or "")) if isinstance(data, dict) else ""
            self._json(
                200,
                {
                    "id": "cmpl-mock",
                    "object": "text_completion",
                    "model": model,
                    "choices": [{"index": 0, "text": f"mock-vllm ok: {prompt[:200]}", "finish_reason": "stop"}],
                    "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
                },
            )
            return

        if self.path == "/v1/embeddings":
            data = self._read_json()
            model = (data.get("model") or MODEL_ID) if isinstance(data, dict) else MODEL_ID
            self._json(
                200,
                {
                    "object": "list",
                    "model": model,
                    "data": [{"object": "embedding", "index": 0, "embedding": [0.0, 0.0, 0.0]}],
                    "usage": {"prompt_tokens": 1, "total_tokens": 1},
                },
            )
            return

        self._json(404, {"error": "not_found"})


if __name__ == "__main__":
    print(f"mock-vllm listening on 0.0.0.0:{PORT} model_id={MODEL_ID}", flush=True)
    HTTPServer(("0.0.0.0", PORT), Handler).serve_forever()


