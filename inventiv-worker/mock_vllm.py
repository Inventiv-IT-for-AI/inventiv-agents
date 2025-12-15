from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os


MODEL_ID = os.getenv("MODEL_ID", "demo-model")
PORT = int(os.getenv("PORT", "8000"))


class Handler(BaseHTTPRequestHandler):
    def _json(self, code: int, payload: dict):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/v1/models":
            self._json(
                200,
                {"object": "list", "data": [{"id": MODEL_ID, "object": "model"}]},
            )
            return
        if self.path in ("/health", "/healthz"):
            self._json(200, {"status": "ok"})
            return
        self._json(404, {"error": "not_found"})


if __name__ == "__main__":
    print(f"mock-vllm listening on 0.0.0.0:{PORT} model_id={MODEL_ID}", flush=True)
    HTTPServer(("0.0.0.0", PORT), Handler).serve_forever()

