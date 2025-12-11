import time
import os
import requests
import sys

# Configuration
ORCHESTRATOR_URL = os.getenv("ORCHESTRATOR_URL", "http://localhost:8000")
WORKER_ID = os.getenv("WORKER_ID", "unknown-worker")
VLLM_URL = "http://localhost:8000/health" # Local vLLM health check

def check_vllm_health():
    try:
        resp = requests.get(VLLM_URL)
        return resp.status_code == 200
    except:
        return False

def register_worker():
    # In a real impl, we would send GPU specs, IP, etc.
    print(f"[{WORKER_ID}] Registering with {ORCHESTRATOR_URL}...")
    # For MVP Mock, Orchestrator creates us, so maybe we just send Heartbeats?
    # Or Orchestrator expects us to call a /register endpoint.
    pass

def loop():
    print(f"Agent Sidecar started for {WORKER_ID}")
    while True:
        is_healthy = check_vllm_health()
        status = "READY" if is_healthy else "STARTING"
        
        # Send Heartbeat / Metrics to Orchestrator (or Router)
        # requests.post(f"{ORCHESTRATOR_URL}/worker/heartbeat", json={"id": WORKER_ID, "status": status})
        
        print(f"[{WORKER_ID}] Health: {is_healthy} - Status: {status}")
        time.sleep(10)

if __name__ == "__main__":
    loop()
