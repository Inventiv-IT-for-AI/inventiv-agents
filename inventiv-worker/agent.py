import json
import os
import subprocess
import threading
import time
from http.server import BaseHTTPRequestHandler, HTTPServer
from uuid import uuid4

import requests

# Configuration
CONTROL_PLANE_URL = os.getenv("CONTROL_PLANE_URL", "").rstrip("/")
WORKER_AUTH_TOKEN = os.getenv("WORKER_AUTH_TOKEN", "").strip()
WORKER_AUTH_TOKEN_FILE = os.getenv("WORKER_AUTH_TOKEN_FILE", "").strip()

INSTANCE_ID = os.getenv("INSTANCE_ID", "").strip()
WORKER_ID = os.getenv("WORKER_ID", "").strip() or str(uuid4())

MODEL_ID = os.getenv("MODEL_ID", "").strip()
VLLM_BASE_URL = os.getenv("VLLM_BASE_URL", "http://127.0.0.1:8000").rstrip("/")
VLLM_READY_URL = f"{VLLM_BASE_URL}/v1/models"

WORKER_HEALTH_PORT = int(os.getenv("WORKER_HEALTH_PORT", "8080"))
WORKER_VLLM_PORT = int(os.getenv("WORKER_VLLM_PORT", "8000"))
HEARTBEAT_INTERVAL_S = float(os.getenv("WORKER_HEARTBEAT_INTERVAL_S", "4"))


def _auth_headers():
    if not WORKER_AUTH_TOKEN:
        return {}
    return {"Authorization": f"Bearer {WORKER_AUTH_TOKEN}"}


def _load_token_from_file():
    global WORKER_AUTH_TOKEN
    if WORKER_AUTH_TOKEN or not WORKER_AUTH_TOKEN_FILE:
        return
    try:
        with open(WORKER_AUTH_TOKEN_FILE, "r", encoding="utf-8") as f:
            tok = (f.read() or "").strip()
        if tok:
            WORKER_AUTH_TOKEN = tok
            print(f"[{WORKER_ID}] loaded WORKER_AUTH_TOKEN from file", flush=True)
    except FileNotFoundError:
        return
    except Exception as e:
        print(f"[{WORKER_ID}] failed reading WORKER_AUTH_TOKEN_FILE: {e}", flush=True)


def _persist_token_to_file(token: str):
    if not WORKER_AUTH_TOKEN_FILE:
        return
    try:
        os.makedirs(os.path.dirname(WORKER_AUTH_TOKEN_FILE) or ".", exist_ok=True)
        with open(WORKER_AUTH_TOKEN_FILE, "w", encoding="utf-8") as f:
            f.write(token.strip() + "\n")
        print(f"[{WORKER_ID}] wrote WORKER_AUTH_TOKEN_FILE", flush=True)
    except Exception as e:
        print(f"[{WORKER_ID}] failed writing WORKER_AUTH_TOKEN_FILE: {e}", flush=True)

def check_vllm_health():
    try:
        resp = requests.get(VLLM_READY_URL, timeout=2)
        return resp.status_code == 200
    except Exception:
        return False

def check_vllm_ready():
    """
    Readiness = vLLM responds AND (if MODEL_ID specified) it is visible in /v1/models.
    """
    try:
        resp = requests.get(VLLM_READY_URL, timeout=2)
        if resp.status_code != 200:
            return False
        data = resp.json()
        if not MODEL_ID:
            return True
        ids = []
        for item in data.get("data", []) or []:
            mid = item.get("id")
            if mid:
                ids.append(mid)
        return MODEL_ID in ids
    except Exception:
        return False


def _try_nvidia_smi():
    """
    Best-effort GPU metrics (works when nvidia-smi is available).
    Returns dict or {}.
    """
    try:
        out = subprocess.check_output(
            [
                "nvidia-smi",
                "--query-gpu=index,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw,power.limit",
                "--format=csv,noheader,nounits",
            ],
            stderr=subprocess.DEVNULL,
            timeout=1,
            text=True,
        )
        gpus = []
        for line in out.strip().splitlines():
            parts = [p.strip() for p in line.split(",")]
            if len(parts) != 7:
                continue
            idx = int(parts[0])
            util = float(parts[1])
            mem_used = float(parts[2])
            mem_total = float(parts[3])
            temp_c = float(parts[4]) if parts[4] not in ("", "N/A") else None
            power_w = float(parts[5]) if parts[5] not in ("", "N/A") else None
            power_limit_w = float(parts[6]) if parts[6] not in ("", "N/A") else None
            gpus.append(
                {
                    "index": idx,
                    "gpu_utilization": util,
                    "gpu_mem_used_mb": mem_used,
                    "gpu_mem_total_mb": mem_total,
                    "gpu_temp_c": temp_c,
                    "gpu_power_w": power_w,
                    "gpu_power_limit_w": power_limit_w,
                }
            )
        if not gpus:
            return {}
        # Aggregate for backward compatibility fields
        avg_util = sum(x["gpu_utilization"] for x in gpus) / float(len(gpus))
        total_used = sum(x["gpu_mem_used_mb"] for x in gpus)
        total_total = sum(x["gpu_mem_total_mb"] for x in gpus)
        temps = [x["gpu_temp_c"] for x in gpus if isinstance(x.get("gpu_temp_c"), (int, float))]
        powers = [x["gpu_power_w"] for x in gpus if isinstance(x.get("gpu_power_w"), (int, float))]
        power_limits = [x["gpu_power_limit_w"] for x in gpus if isinstance(x.get("gpu_power_limit_w"), (int, float))]
        return {
            "gpu_utilization": avg_util,
            "gpu_mem_used_mb": total_used,
            "gpu_mem_total_mb": total_total,
            "gpu_temp_c": (sum(temps) / float(len(temps))) if temps else None,
            "gpu_power_w": (sum(powers) / float(len(powers))) if powers else None,
            "gpu_power_limit_w": (sum(power_limits) / float(len(power_limits))) if power_limits else None,
            "gpus": gpus,
        }
    except Exception:
        return {}


class _Handler(BaseHTTPRequestHandler):
    def _write(self, code: int, body: str, content_type: str = "text/plain"):
        self.send_response(code)
        self.send_header("Content-Type", content_type)
        self.end_headers()
        self.wfile.write(body.encode("utf-8"))

    def do_GET(self):
        if self.path == "/healthz":
            self._write(200, "ok\n")
            return

        if self.path == "/readyz":
            if check_vllm_ready():
                self._write(200, "ready\n")
            else:
                self._write(503, "not-ready\n")
            return

        if self.path == "/metrics":
            ready = 1 if check_vllm_ready() else 0
            up = 1
            gpu = _try_nvidia_smi()
            lines = [
                "# HELP inventiv_worker_up Worker process is up (always 1).",
                "# TYPE inventiv_worker_up gauge",
                f"inventiv_worker_up {up}",
                "# HELP inventiv_worker_vllm_ready vLLM is ready (1/0).",
                "# TYPE inventiv_worker_vllm_ready gauge",
                f"inventiv_worker_vllm_ready {ready}",
            ]
            if "gpu_utilization" in gpu:
                lines += [
                    "# HELP inventiv_worker_gpu_utilization GPU utilization percent.",
                    "# TYPE inventiv_worker_gpu_utilization gauge",
                    f"inventiv_worker_gpu_utilization {gpu['gpu_utilization']}",
                ]
            if "gpu_mem_used_mb" in gpu:
                lines += [
                    "# HELP inventiv_worker_gpu_mem_used_mb GPU memory used MB.",
                    "# TYPE inventiv_worker_gpu_mem_used_mb gauge",
                    f"inventiv_worker_gpu_mem_used_mb {gpu['gpu_mem_used_mb']}",
                ]
            if "gpu_mem_total_mb" in gpu:
                lines += [
                    "# HELP inventiv_worker_gpu_mem_total_mb GPU memory total MB.",
                    "# TYPE inventiv_worker_gpu_mem_total_mb gauge",
                    f"inventiv_worker_gpu_mem_total_mb {gpu['gpu_mem_total_mb']}",
                ]
            self._write(200, "\n".join(lines) + "\n")
            return

        self._write(404, "not-found\n")


def _serve_http():
    server = HTTPServer(("0.0.0.0", WORKER_HEALTH_PORT), _Handler)
    server.serve_forever()


def register_worker_once():
    global WORKER_AUTH_TOKEN
    if not CONTROL_PLANE_URL or not INSTANCE_ID:
        return False
    payload = {
        "instance_id": INSTANCE_ID,
        "worker_id": WORKER_ID,
        "model_id": MODEL_ID or None,
        "vllm_port": WORKER_VLLM_PORT,
        "health_port": WORKER_HEALTH_PORT,
        "metadata": _try_nvidia_smi() or None,
    }
    try:
        resp = requests.post(
            f"{CONTROL_PLANE_URL}/internal/worker/register",
            headers=_auth_headers(),
            json=payload,
            timeout=3,
        )
        ok = resp.status_code // 100 == 2
        if ok:
            # Bootstrap flow: orchestrator may return a freshly generated token for this worker.
            try:
                data = resp.json() if resp.text else {}
            except Exception:
                data = {}
            tok = (data or {}).get("bootstrap_token")
            if tok and not WORKER_AUTH_TOKEN:
                WORKER_AUTH_TOKEN = str(tok).strip()
                _persist_token_to_file(WORKER_AUTH_TOKEN)
                print(f"[{WORKER_ID}] received bootstrap_token prefix={(data or {}).get('bootstrap_token_prefix')}", flush=True)
        if not ok:
            print(f"[{WORKER_ID}] register failed: {resp.status_code} {resp.text[:200]}", flush=True)
        return ok
    except Exception as e:
        print(f"[{WORKER_ID}] register exception: {e}", flush=True)
        return False


def send_heartbeat(status: str):
    if not CONTROL_PLANE_URL or not INSTANCE_ID:
        return False
    gpu = _try_nvidia_smi()
    payload = {
        "instance_id": INSTANCE_ID,
        "worker_id": WORKER_ID,
        "status": status,
        "model_id": MODEL_ID or None,
        "queue_depth": None,
        "gpu_utilization": gpu.get("gpu_utilization"),
        "gpu_mem_used_mb": gpu.get("gpu_mem_used_mb"),
        "metadata": gpu or None,
    }
    try:
        resp = requests.post(
            f"{CONTROL_PLANE_URL}/internal/worker/heartbeat",
            headers=_auth_headers(),
            json=payload,
            timeout=3,
        )
        ok = resp.status_code // 100 == 2
        if not ok:
            print(f"[{WORKER_ID}] heartbeat failed: {resp.status_code} {resp.text[:200]}", flush=True)
        return ok
    except Exception as e:
        print(f"[{WORKER_ID}] heartbeat exception: {e}", flush=True)
        return False

def loop():
    print(f"Agent Sidecar started for worker_id={WORKER_ID} instance_id={INSTANCE_ID or 'unset'}")
    print(f"Health endpoints on :{WORKER_HEALTH_PORT} (GET /healthz, /readyz, /metrics)")

    _load_token_from_file()

    http_thread = threading.Thread(target=_serve_http, daemon=True)
    http_thread.start()

    registered = False
    while True:
        is_ready = check_vllm_ready()
        status = "ready" if is_ready else "starting"

        if not registered:
            # Register early to allow token bootstrap before first heartbeat.
            registered = register_worker_once()

        hb_ok = False
        if WORKER_AUTH_TOKEN or registered:
            hb_ok = send_heartbeat(status=status)
        else:
            print(f"[{WORKER_ID}] skipping heartbeat (no auth token yet)", flush=True)
        print(f"[{WORKER_ID}] vLLM ready={is_ready} status={status} registered={registered} heartbeat={hb_ok}", flush=True)
        time.sleep(HEARTBEAT_INTERVAL_S)

if __name__ == "__main__":
    loop()
