import json
import os
import re
import subprocess
import shutil
import socket
import urllib.parse
import threading
import time
from http.server import BaseHTTPRequestHandler, HTTPServer
from uuid import uuid4
import hashlib

import requests

# Version information (updated on each release)
AGENT_VERSION = "1.0.0"  # Update this when making changes to agent.py
AGENT_BUILD_DATE = "2026-01-03"  # Update this when making changes

def _get_agent_checksum():
    """Calculate SHA256 checksum of this agent.py file."""
    try:
        agent_path = os.path.abspath(__file__)
        if os.path.exists(agent_path):
            with open(agent_path, "rb") as f:
                content = f.read()
                return hashlib.sha256(content).hexdigest()
    except Exception:
        pass
    return None

# Configuration
CONTROL_PLANE_URL = os.getenv("CONTROL_PLANE_URL", "").rstrip("/")
WORKER_AUTH_TOKEN = os.getenv("WORKER_AUTH_TOKEN", "").strip()
WORKER_AUTH_TOKEN_FILE = os.getenv("WORKER_AUTH_TOKEN_FILE", "").strip()

INSTANCE_ID = os.getenv("INSTANCE_ID", "").strip()
WORKER_ID = os.getenv("WORKER_ID", "").strip() or str(uuid4())

MODEL_ID = os.getenv("MODEL_ID", "").strip()
VLLM_BASE_URL = os.getenv("VLLM_BASE_URL", "http://127.0.0.1:8000").rstrip("/")
VLLM_READY_URL = f"{VLLM_BASE_URL}/v1/models"
VLLM_METRICS_URL = os.getenv("VLLM_METRICS_URL", f"{VLLM_BASE_URL}/metrics").rstrip("/")

WORKER_HEALTH_PORT = int(os.getenv("WORKER_HEALTH_PORT", "8080"))
WORKER_VLLM_PORT = int(os.getenv("WORKER_VLLM_PORT", "8000"))
HEARTBEAT_INTERVAL_S = float(os.getenv("WORKER_HEARTBEAT_INTERVAL_S", "4"))
WORKER_DISK_PATH = os.getenv("WORKER_DISK_PATH", "/").strip() or "/"
WORKER_ADVERTISE_IP = os.getenv("WORKER_ADVERTISE_IP", "").strip()

# Optional: simulate GPU metrics when running in environments without nvidia-smi (local/mock).
# This is off by default and only activates when WORKER_SIMULATE_GPU_COUNT > 0.
WORKER_SIMULATE_GPU_COUNT = int(os.getenv("WORKER_SIMULATE_GPU_COUNT", "0") or "0")
WORKER_SIMULATE_GPU_VRAM_MB = int(os.getenv("WORKER_SIMULATE_GPU_VRAM_MB", "24576") or "24576")

# State for rate/percent calculations (best-effort)
_PREV_CPU = None  # (total, idle)
_PREV_NET = None  # (rx_bytes, tx_bytes, ts)

# Worker event logging for diagnostics
_WORKER_LOG_FILE = os.getenv("WORKER_LOG_FILE", "/opt/inventiv-worker/worker-events.log")
_WORKER_LOG_MAX_SIZE = 10 * 1024 * 1024  # 10MB max log file size
_WORKER_LOG_MAX_LINES = 10000  # Keep last 10000 lines when rotating


def _auth_headers():
    if not WORKER_AUTH_TOKEN:
        return {}
    return {"Authorization": f"Bearer {WORKER_AUTH_TOKEN}"}


def _log_event(event_type: str, message: str, details: dict | None = None):
    """
    Log structured events to a file for diagnostics.
    Events are JSON lines for easy parsing.
    """
    import json
    from datetime import datetime
    
    try:
        # Ensure log directory exists
        log_dir = os.path.dirname(_WORKER_LOG_FILE)
        if log_dir and not os.path.exists(log_dir):
            os.makedirs(log_dir, exist_ok=True)
        
        # Rotate log file if too large
        if os.path.exists(_WORKER_LOG_FILE):
            size = os.path.getsize(_WORKER_LOG_FILE)
            if size > _WORKER_LOG_MAX_SIZE:
                # Rotate: keep last N lines
                try:
                    with open(_WORKER_LOG_FILE, "r", encoding="utf-8") as f:
                        lines = f.readlines()
                    if len(lines) > _WORKER_LOG_MAX_LINES:
                        lines = lines[-_WORKER_LOG_MAX_LINES:]
                    with open(_WORKER_LOG_FILE, "w", encoding="utf-8") as f:
                        f.writelines(lines)
                except Exception:
                    # If rotation fails, truncate
                    open(_WORKER_LOG_FILE, "w").close()
        
        # Write event as JSON line
        event = {
            "timestamp": datetime.utcnow().isoformat() + "Z",
            "event_type": event_type,
            "message": message,
            "worker_id": WORKER_ID,
            "instance_id": INSTANCE_ID or None,
            "details": details or {},
        }
        
        with open(_WORKER_LOG_FILE, "a", encoding="utf-8") as f:
            f.write(json.dumps(event) + "\n")
            f.flush()
    except Exception as e:
        # Don't fail if logging fails, but print to stderr
        print(f"[{WORKER_ID}] Failed to write event log: {e}", flush=True, file=sys.stderr)


def _local_ip_best_effort():
    """
    Best-effort container IP discovery.
    Prefer a route-based method (UDP connect) to avoid returning 127.0.0.1.
    """
    if WORKER_ADVERTISE_IP:
        return WORKER_ADVERTISE_IP
    try:
        if CONTROL_PLANE_URL:
            u = urllib.parse.urlparse(CONTROL_PLANE_URL)
            host = u.hostname
            port = u.port or (443 if u.scheme == "https" else 80)
            if host:
                s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
                try:
                    s.connect((host, port))
                    ip = s.getsockname()[0]
                    if ip and ip != "127.0.0.1":
                        return ip
                finally:
                    s.close()
    except Exception:
        pass
    try:
        ip = socket.gethostbyname(socket.gethostname())
        if ip and ip != "127.0.0.1":
            return ip
    except Exception:
        pass
    return None


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
        is_ready = resp.status_code == 200
        if is_ready:
            data = resp.json()
            if MODEL_ID:
                ids = []
                for item in data.get("data", []) or []:
                    mid = item.get("id")
                    if mid:
                        ids.append(mid)
                is_ready = MODEL_ID in ids
        else:
            # Log vLLM check failures periodically to avoid spam
            import random
            if random.randint(1, 20) == 1:
                _log_event("vllm_check_failed", f"vLLM readiness check failed", {
                    "status_code": resp.status_code,
                    "vllm_url": VLLM_READY_URL,
                })
        return is_ready
    except Exception as e:
        # Log vLLM check exceptions periodically to avoid spam
        import random
        if random.randint(1, 20) == 1:
            _log_event("vllm_check_exception", f"vLLM readiness check exception", {
                "exception_type": type(e).__name__,
                "error": str(e),
                "vllm_url": VLLM_READY_URL,
            })
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


def _fake_gpu_metrics(vllm: dict | None):
    """
    Synthetic GPU metrics for non-GPU environments (best-effort).
    Enabled only when WORKER_SIMULATE_GPU_COUNT > 0.
    """
    if WORKER_SIMULATE_GPU_COUNT <= 0:
        return {}
    qd = 0.0
    try:
        if isinstance(vllm, dict):
            qd = float(vllm.get("queue_depth") or 0.0)
    except Exception:
        qd = 0.0

    # Deterministic "load" derived from queue depth.
    base_util = max(0.0, min(95.0, 5.0 + (qd * 8.0)))
    gpus = []
    for idx in range(WORKER_SIMULATE_GPU_COUNT):
        util = max(0.0, min(100.0, base_util + (idx * 3.0)))
        mem_total = float(WORKER_SIMULATE_GPU_VRAM_MB)
        mem_used = max(0.0, min(mem_total, mem_total * (util / 100.0)))
        temp_c = 35.0 + (util * 0.5)
        power_limit_w = 300.0
        power_w = power_limit_w * (util / 100.0)
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
    avg_util = sum(x["gpu_utilization"] for x in gpus) / float(len(gpus))
    total_used = sum(x["gpu_mem_used_mb"] for x in gpus)
    total_total = sum(x["gpu_mem_total_mb"] for x in gpus)
    temps = [x["gpu_temp_c"] for x in gpus]
    powers = [x["gpu_power_w"] for x in gpus]
    power_limits = [x["gpu_power_limit_w"] for x in gpus]
    return {
        "gpu_utilization": avg_util,
        "gpu_mem_used_mb": total_used,
        "gpu_mem_total_mb": total_total,
        "gpu_temp_c": (sum(temps) / float(len(temps))) if temps else None,
        "gpu_power_w": (sum(powers) / float(len(powers))) if powers else None,
        "gpu_power_limit_w": (sum(power_limits) / float(len(power_limits))) if power_limits else None,
        "gpus": gpus,
    }

def _read_proc_stat_cpu():
    """
    Returns (total_ticks, idle_ticks) from /proc/stat, or None.
    """
    try:
        with open("/proc/stat", "r", encoding="utf-8") as f:
            for line in f:
                if line.startswith("cpu "):
                    parts = line.strip().split()
                    # cpu user nice system idle iowait irq softirq steal guest guest_nice
                    vals = [int(x) for x in parts[1:] if x.isdigit() or x.lstrip("-").isdigit()]
                    if len(vals) < 4:
                        return None
                    user, nice, system, idle = vals[0], vals[1], vals[2], vals[3]
                    iowait = vals[4] if len(vals) > 4 else 0
                    irq = vals[5] if len(vals) > 5 else 0
                    softirq = vals[6] if len(vals) > 6 else 0
                    steal = vals[7] if len(vals) > 7 else 0
                    idle_all = idle + iowait
                    non_idle = user + nice + system + irq + softirq + steal
                    total = idle_all + non_idle
                    return (total, idle_all)
    except Exception:
        return None
    return None


def _read_meminfo():
    """
    Returns dict with mem_total_bytes, mem_available_bytes.
    """
    out = {}
    try:
        with open("/proc/meminfo", "r", encoding="utf-8") as f:
            for line in f:
                if ":" not in line:
                    continue
                k, v = line.split(":", 1)
                k = k.strip()
                v = v.strip()
                if not v:
                    continue
                # Values are usually kB.
                m = re.match(r"^(\d+)\s+kB$", v)
                if not m:
                    continue
                bytes_v = int(m.group(1)) * 1024
                if k == "MemTotal":
                    out["mem_total_bytes"] = bytes_v
                elif k == "MemAvailable":
                    out["mem_available_bytes"] = bytes_v
    except Exception:
        return {}
    return out


def _read_loadavg():
    try:
        with open("/proc/loadavg", "r", encoding="utf-8") as f:
            parts = (f.read() or "").strip().split()
        if len(parts) < 3:
            return {}
        return {"load1": float(parts[0]), "load5": float(parts[1]), "load15": float(parts[2])}
    except Exception:
        return {}


def _disk_usage(path: str):
    try:
        du = shutil.disk_usage(path)
        return {
            "disk_path": path,
            "disk_total_bytes": int(du.total),
            "disk_used_bytes": int(du.used),
            "disk_free_bytes": int(du.free),
        }
    except Exception:
        return {}


def _read_netdev_totals():
    """
    Returns (rx_bytes, tx_bytes) summed across interfaces (excluding loopback), or None.
    """
    try:
        with open("/proc/net/dev", "r", encoding="utf-8") as f:
            lines = f.read().splitlines()
        rx = 0
        tx = 0
        for line in lines[2:]:
            if ":" not in line:
                continue
            iface, rest = line.split(":", 1)
            iface = iface.strip()
            if not iface or iface == "lo":
                continue
            parts = rest.split()
            if len(parts) < 16:
                continue
            rx += int(parts[0])
            tx += int(parts[8])
        return (rx, tx)
    except Exception:
        return None


def _collect_system_metrics():
    """
    Best-effort system metrics (works in containers with /proc mounted).
    Returns dict suitable for worker_metadata.system.
    """
    global _PREV_CPU, _PREV_NET
    now = time.time()

    out = {}
    out.update(_read_loadavg())
    out.update(_read_meminfo())
    out.update(_disk_usage(WORKER_DISK_PATH))

    # CPU usage percent (requires previous sample)
    cpu = _read_proc_stat_cpu()
    if cpu and _PREV_CPU:
        total, idle = cpu
        prev_total, prev_idle = _PREV_CPU
        dt_total = total - prev_total
        dt_idle = idle - prev_idle
        if dt_total > 0:
            out["cpu_usage_pct"] = max(0.0, min(100.0, (1.0 - (dt_idle / float(dt_total))) * 100.0))
    if cpu:
        _PREV_CPU = cpu

    # Network rates (bytes/sec) since last sample
    net = _read_netdev_totals()
    if net:
        rx, tx = net
        if _PREV_NET:
            prev_rx, prev_tx, prev_ts = _PREV_NET
            dt = max(0.001, now - prev_ts)
            out["net_rx_bps"] = max(0.0, (rx - prev_rx) / dt)
            out["net_tx_bps"] = max(0.0, (tx - prev_tx) / dt)
        _PREV_NET = (rx, tx, now)
        out["net_rx_bytes_total"] = rx
        out["net_tx_bytes_total"] = tx

    # Convenience derived percents
    mt = out.get("mem_total_bytes")
    ma = out.get("mem_available_bytes")
    if isinstance(mt, int) and mt > 0 and isinstance(ma, int):
        out["mem_used_bytes"] = max(0, mt - ma)
        out["mem_used_pct"] = max(0.0, min(100.0, ((mt - ma) / float(mt)) * 100.0))

    dt = out.get("disk_total_bytes")
    du = out.get("disk_used_bytes")
    if isinstance(dt, int) and dt > 0 and isinstance(du, int):
        out["disk_used_pct"] = max(0.0, min(100.0, (du / float(dt)) * 100.0))

    return out


def _parse_prometheus_metric(text: str, names):
    """
    Parse first matching metric among `names` from Prometheus text exposition.
    Returns float or None.
    """
    for name in names:
        # match: name{...} 12.3  OR name 12.3
        m = re.search(rf"(?m)^{re.escape(name)}(?:\{{[^}}]*\}})?\s+([0-9eE\.\+\-]+)\s*$", text)
        if m:
            try:
                return float(m.group(1))
            except Exception:
                pass
    return None


def _collect_vllm_signals():
    """
    Best-effort vLLM signals for load-balancing (queue depth / inflight).
    Returns dict.
    """
    try:
        resp = requests.get(VLLM_METRICS_URL, timeout=2)
        if resp.status_code != 200:
            return {}
        txt = resp.text or ""
    except Exception:
        return {}

    # Metric names vary across vLLM versions; try a small allowlist.
    waiting = _parse_prometheus_metric(
        txt,
        names=[
            "vllm_num_requests_waiting",
            "vllm:num_requests_waiting",
            "vllm_requests_waiting",
            "vllm:requests_waiting",
        ],
    )
    running = _parse_prometheus_metric(
        txt,
        names=[
            "vllm_num_requests_running",
            "vllm:num_requests_running",
            "vllm_requests_running",
            "vllm:requests_running",
        ],
    )
    out = {}
    if waiting is not None:
        out["requests_waiting"] = waiting
    if running is not None:
        out["requests_running"] = running
    if waiting is not None:
        try:
            out["queue_depth"] = int(max(0.0, waiting))
        except Exception:
            pass
    return out


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
            sysm = _collect_system_metrics()
            vllm = _collect_vllm_signals()
            lines = [
                "# HELP inventiv_worker_up Worker process is up (always 1).",
                "# TYPE inventiv_worker_up gauge",
                f"inventiv_worker_up {up}",
                "# HELP inventiv_worker_vllm_ready vLLM is ready (1/0).",
                "# TYPE inventiv_worker_vllm_ready gauge",
                f"inventiv_worker_vllm_ready {ready}",
            ]
            if "queue_depth" in vllm:
                lines += [
                    "# HELP inventiv_worker_queue_depth Best-effort queue depth (requests waiting).",
                    "# TYPE inventiv_worker_queue_depth gauge",
                    f"inventiv_worker_queue_depth {int(vllm['queue_depth'])}",
                ]
            if "requests_running" in vllm:
                lines += [
                    "# HELP inventiv_worker_requests_running Best-effort running requests.",
                    "# TYPE inventiv_worker_requests_running gauge",
                    f"inventiv_worker_requests_running {float(vllm['requests_running'])}",
                ]
            if "requests_waiting" in vllm:
                lines += [
                    "# HELP inventiv_worker_requests_waiting Best-effort waiting requests.",
                    "# TYPE inventiv_worker_requests_waiting gauge",
                    f"inventiv_worker_requests_waiting {float(vllm['requests_waiting'])}",
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
            # Per-GPU metrics (preferred for observability / balancing)
            if isinstance(gpu.get("gpus"), list):
                lines += [
                    "# HELP inventiv_worker_gpu_utilization_by_gpu GPU utilization percent by GPU index.",
                    "# TYPE inventiv_worker_gpu_utilization_by_gpu gauge",
                ]
                for g in gpu.get("gpus") or []:
                    try:
                        idx = int(g.get("index", 0))
                        util = float(g.get("gpu_utilization", 0.0))
                        lines.append(f'inventiv_worker_gpu_utilization_by_gpu{{gpu_index="{idx}"}} {util}')
                    except Exception:
                        continue
                lines += [
                    "# HELP inventiv_worker_gpu_mem_used_mb_by_gpu GPU memory used MB by GPU index.",
                    "# TYPE inventiv_worker_gpu_mem_used_mb_by_gpu gauge",
                ]
                for g in gpu.get("gpus") or []:
                    try:
                        idx = int(g.get("index", 0))
                        used = float(g.get("gpu_mem_used_mb", 0.0))
                        lines.append(f'inventiv_worker_gpu_mem_used_mb_by_gpu{{gpu_index="{idx}"}} {used}')
                    except Exception:
                        continue

            # System metrics
            if "cpu_usage_pct" in sysm:
                lines += [
                    "# HELP inventiv_worker_cpu_usage_pct CPU usage percent (host/container).",
                    "# TYPE inventiv_worker_cpu_usage_pct gauge",
                    f"inventiv_worker_cpu_usage_pct {float(sysm['cpu_usage_pct'])}",
                ]
            if "load1" in sysm:
                lines += [
                    "# HELP inventiv_worker_load1 Load average (1m).",
                    "# TYPE inventiv_worker_load1 gauge",
                    f"inventiv_worker_load1 {float(sysm['load1'])}",
                ]
            if "mem_used_bytes" in sysm:
                lines += [
                    "# HELP inventiv_worker_mem_used_bytes Memory used bytes.",
                    "# TYPE inventiv_worker_mem_used_bytes gauge",
                    f"inventiv_worker_mem_used_bytes {int(sysm['mem_used_bytes'])}",
                ]
            if "mem_total_bytes" in sysm:
                lines += [
                    "# HELP inventiv_worker_mem_total_bytes Memory total bytes.",
                    "# TYPE inventiv_worker_mem_total_bytes gauge",
                    f"inventiv_worker_mem_total_bytes {int(sysm['mem_total_bytes'])}",
                ]
            if "disk_used_bytes" in sysm:
                lines += [
                    "# HELP inventiv_worker_disk_used_bytes Disk used bytes for WORKER_DISK_PATH.",
                    "# TYPE inventiv_worker_disk_used_bytes gauge",
                    f"inventiv_worker_disk_used_bytes {int(sysm['disk_used_bytes'])}",
                ]
            if "disk_total_bytes" in sysm:
                lines += [
                    "# HELP inventiv_worker_disk_total_bytes Disk total bytes for WORKER_DISK_PATH.",
                    "# TYPE inventiv_worker_disk_total_bytes gauge",
                    f"inventiv_worker_disk_total_bytes {int(sysm['disk_total_bytes'])}",
                ]
            if "net_rx_bps" in sysm:
                lines += [
                    "# HELP inventiv_worker_net_rx_bps Network receive bytes/sec (aggregated).",
                    "# TYPE inventiv_worker_net_rx_bps gauge",
                    f"inventiv_worker_net_rx_bps {float(sysm['net_rx_bps'])}",
                ]
            if "net_tx_bps" in sysm:
                lines += [
                    "# HELP inventiv_worker_net_tx_bps Network transmit bytes/sec (aggregated).",
                    "# TYPE inventiv_worker_net_tx_bps gauge",
                    f"inventiv_worker_net_tx_bps {float(sysm['net_tx_bps'])}",
                ]
            self._write(200, "\n".join(lines) + "\n")
            return

        if self.path == "/info":
            info = {
                "agent_version": AGENT_VERSION,
                "agent_build_date": AGENT_BUILD_DATE,
                "agent_checksum": _get_agent_checksum(),
                "agent_path": os.path.abspath(__file__),
                "python_version": sys.version.split()[0],
                "worker_id": WORKER_ID,
                "instance_id": INSTANCE_ID or None,
                "model_id": MODEL_ID or None,
                "vllm_ready": check_vllm_ready(),
            }
            self._write(200, json.dumps(info, indent=2) + "\n", "application/json")
            return

        if self.path == "/logs" or self.path.startswith("/logs?"):
            # Parse query params: ?tail=N (default 1000 lines), ?since=ISO8601
            from urllib.parse import urlparse, parse_qs
            parsed = urlparse(self.path)
            params = parse_qs(parsed.query)
            tail = int(params.get("tail", ["1000"])[0])
            since = params.get("since", [None])[0]
            
            try:
                if not os.path.exists(_WORKER_LOG_FILE):
                    self._write(200, json.dumps({"events": [], "message": "Log file does not exist yet"}) + "\n", "application/json")
                    return
                
                with open(_WORKER_LOG_FILE, "r", encoding="utf-8") as f:
                    lines = f.readlines()
                
                # Filter by timestamp if since is provided
                events = []
                for line in lines[-tail:]:
                    line = line.strip()
                    if not line:
                        continue
                    try:
                        event = json.loads(line)
                        if since:
                            if event.get("timestamp", "") < since:
                                continue
                        events.append(event)
                    except Exception:
                        continue
                
                result = {
                    "log_file": _WORKER_LOG_FILE,
                    "total_events": len(events),
                    "events": events,
                }
                self._write(200, json.dumps(result, indent=2) + "\n", "application/json")
            except Exception as e:
                self._write(500, json.dumps({"error": str(e)}) + "\n", "application/json")
            return

        self._write(404, "not-found\n")


def _serve_http():
    server = HTTPServer(("0.0.0.0", WORKER_HEALTH_PORT), _Handler)
    server.serve_forever()


def register_worker_once():
    global WORKER_AUTH_TOKEN
    if not CONTROL_PLANE_URL or not INSTANCE_ID:
        _log_event("register_failed", "Missing CONTROL_PLANE_URL or INSTANCE_ID", {
            "control_plane_url": CONTROL_PLANE_URL or None,
            "instance_id": INSTANCE_ID or None,
        })
        return False
    try:
        _log_event("register_start", "Attempting to register worker", {
            "control_plane_url": CONTROL_PLANE_URL,
            "instance_id": INSTANCE_ID,
        })
        vllm = _collect_vllm_signals()
        gpu = _try_nvidia_smi() or {}
        if not gpu:
            gpu = _fake_gpu_metrics(vllm)
        payload = {
            "instance_id": INSTANCE_ID,
            "worker_id": WORKER_ID,
            "model_id": MODEL_ID or None,
            "vllm_port": WORKER_VLLM_PORT,
            "health_port": WORKER_HEALTH_PORT,
            "ip_address": _local_ip_best_effort(),
            "metadata": {
                **(gpu or {}),
                "system": _collect_system_metrics() or None,
                "vllm": vllm or None,
            }
            or None,
        }
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
                _log_event("register_success", "Worker registered successfully", {
                    "status_code": resp.status_code,
                    "token_received": True,
                })
            else:
                _log_event("register_success", "Worker registered successfully (no token)", {
                    "status_code": resp.status_code,
                    "token_received": False,
                })
        else:
            error_msg = resp.text[:200] if resp.text else "no response body"
            print(f"[{WORKER_ID}] register failed: {resp.status_code} {error_msg}", flush=True)
            _log_event("register_failed", f"Register request failed", {
                "status_code": resp.status_code,
                "error": error_msg,
            })
        return ok
    except Exception as e:
        error_str = str(e)
        print(f"[{WORKER_ID}] register exception: {error_str}", flush=True)
        _log_event("register_exception", f"Register request exception", {
            "exception_type": type(e).__name__,
            "error": error_str,
            "control_plane_url": CONTROL_PLANE_URL,
        })
        return False


def send_heartbeat(status: str):
    if not CONTROL_PLANE_URL or not INSTANCE_ID:
        _log_event("heartbeat_skipped", "Missing CONTROL_PLANE_URL or INSTANCE_ID", {
            "control_plane_url": CONTROL_PLANE_URL or None,
            "instance_id": INSTANCE_ID or None,
        })
        return False
    sysm = _collect_system_metrics()
    vllm = _collect_vllm_signals()
    gpu = _try_nvidia_smi() or {}
    if not gpu:
        gpu = _fake_gpu_metrics(vllm)
    agent_checksum = _get_agent_checksum()
    payload = {
        "instance_id": INSTANCE_ID,
        "worker_id": WORKER_ID,
        "status": status,
        "model_id": MODEL_ID or None,
        "queue_depth": vllm.get("queue_depth"),
        "gpu_utilization": gpu.get("gpu_utilization"),
        "gpu_mem_used_mb": gpu.get("gpu_mem_used_mb"),
        "ip_address": _local_ip_best_effort(),
        "agent_info": {
            "version": AGENT_VERSION,
            "build_date": AGENT_BUILD_DATE,
            "checksum": agent_checksum,
        },
        "metadata": {
            **(gpu or {}),
            "system": sysm or None,
            "vllm": vllm or None,
        }
        or None,
    }
    try:
        resp = requests.post(
            f"{CONTROL_PLANE_URL}/internal/worker/heartbeat",
            headers=_auth_headers(),
            json=payload,
            timeout=3,
        )
        ok = resp.status_code // 100 == 2
        if ok:
            # Log successful heartbeat periodically (every 10th heartbeat to avoid spam)
            import random
            if random.randint(1, 10) == 1:
                _log_event("heartbeat_success", "Heartbeat sent successfully", {
                    "status_code": resp.status_code,
                    "worker_status": status,
                    "vllm_ready": vllm.get("queue_depth") is not None,
                })
        else:
            error_msg = resp.text[:200] if resp.text else "no response body"
            print(f"[{WORKER_ID}] heartbeat failed: {resp.status_code} {error_msg}", flush=True)
            _log_event("heartbeat_failed", f"Heartbeat request failed", {
                "status_code": resp.status_code,
                "error": error_msg,
                "worker_status": status,
            })
        return ok
    except Exception as e:
        error_str = str(e)
        print(f"[{WORKER_ID}] heartbeat exception: {error_str}", flush=True)
        _log_event("heartbeat_exception", f"Heartbeat request exception", {
            "exception_type": type(e).__name__,
            "error": error_str,
            "control_plane_url": CONTROL_PLANE_URL,
            "worker_status": status,
        })
        return False

def loop():
    print(f"Agent Sidecar started for worker_id={WORKER_ID} instance_id={INSTANCE_ID or 'unset'}")
    print(f"Health endpoints on :{WORKER_HEALTH_PORT} (GET /healthz, /readyz, /metrics, /logs, /info)")
    
    _log_event("agent_started", "Agent sidecar started", {
        "worker_id": WORKER_ID,
        "instance_id": INSTANCE_ID or None,
        "model_id": MODEL_ID or None,
        "control_plane_url": CONTROL_PLANE_URL or None,
        "vllm_base_url": VLLM_BASE_URL,
        "health_port": WORKER_HEALTH_PORT,
        "vllm_port": WORKER_VLLM_PORT,
    })

    _load_token_from_file()

    http_thread = threading.Thread(target=_serve_http, daemon=True)
    http_thread.start()
    
    _log_event("http_server_started", "HTTP server started", {
        "port": WORKER_HEALTH_PORT,
    })

    registered = False
    vllm_ready_count = 0
    while True:
        is_ready = check_vllm_ready()
        status = "ready" if is_ready else "starting"
        
        # Log vLLM state changes
        if is_ready:
            vllm_ready_count += 1
            if vllm_ready_count == 1:
                _log_event("vllm_ready", "vLLM became ready", {
                    "vllm_url": VLLM_BASE_URL,
                })
        else:
            if vllm_ready_count > 0:
                _log_event("vllm_not_ready", "vLLM is not ready", {
                    "vllm_url": VLLM_BASE_URL,
                })
                vllm_ready_count = 0

        if not registered:
            # Register early to allow token bootstrap before first heartbeat.
            registered = register_worker_once()

        hb_ok = False
        if WORKER_AUTH_TOKEN or registered:
            hb_ok = send_heartbeat(status=status)
        else:
            print(f"[{WORKER_ID}] skipping heartbeat (no auth token yet)", flush=True)
            _log_event("heartbeat_skipped", "Skipping heartbeat (no auth token)", {})
        print(f"[{WORKER_ID}] vLLM ready={is_ready} status={status} registered={registered} heartbeat={hb_ok}", flush=True)
        time.sleep(HEARTBEAT_INTERVAL_S)

if __name__ == "__main__":
    loop()
