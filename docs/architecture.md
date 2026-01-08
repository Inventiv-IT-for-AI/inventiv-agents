# Inventiv Agents Platform

## 1. Project Objective
**Inventiv Agents** is a **Sovereign**, **Open-Source**, and **Cloud-Agnostic** infrastructure for deploying, managing, and scaling Language Models (LLM) on demand.

The objective is to provide an intelligent orchestration layer capable of:
*   Dynamically provisioning GPU resources (via Scaleway, AWS, or On-Premise).
*   Deploying LLM models (via vLLM, TGI, etc.) in a standardized manner.
*   Exposing these models via a single OpenAI-compatible API.
*   Managing the complete lifecycle: from "Model Definition" to "Scaling-to-Zero".

## 2. Technical Architecture
The system relies on strict separation of responsibilities (**CQRS / Event-Driven** pattern) to ensure scalability and robustness.

### Components & Responsibilities

#### 1. Inventiv Backend (Product Plane - Synchronous)
*   **Role**: Purely transactional HTTP Request/Response interface.
*   **Responsibilities**:
    *   Manages public API (excluding inference), Authentication, Billing, and access control.
    *   Performs "Business" state reads/writes in the database.
    *   **Does not perform any background tasks** or asynchronous processing.
    *   Notifies the system of intent changes (e.g., "User wants to deploy X") via Events/DB.
    *   Real-time Relay: pushes notifications to the Frontend (MVP: **SSE** on API side).

#### 2. Inventiv Orchestrator (Control Plane - Asynchronous)
*   **Role**: Execution and monitoring engine (invisible to the public).
*   **Responsibilities**:
    *   Manages **all** asynchronous tasks and background jobs (Monitoring, Scaling, Provisioning).
    *   Supervises Instances, Traffic, and consumption counters (Tokens).
    *   Communicates with Workers and the Router.
    *   **Exposes no public endpoints** and never interacts directly with users.
*   **Communication Pattern**:
    *   **Writes**: Updates technical state in PostgreSQL (Instance statuses, IPs).
    *   **Read/React**: Consumes Backend events (via Redis Pub/Sub) to trigger immediate actions (Scale Up, Block API Key).

##### Autoscaling (objective)
In the "simple" scenario (Docker Compose on a few machines), the Orchestrator also hosts an **Autoscaler** component:
- **Input**: load/capacity signals (queue depth, latency/TTFT, GPU util, errors) + objectives per pool/model.
- **Decision**: calculates desired capacity (min/max) per pool.
- **Action**: calls the provider (Scaleway) to **create/terminate** instances, applying a **drain → terminate** policy.

#### 3. Inventiv Router (Data Plane) — *status*
*   **Planned** (OpenAI-compatible), but **not present** in the repo at this stage.
*   **Current state (repo)**: `inventiv-api` already exposes OpenAI-compatible endpoints (`/v1/*`) and routes to available workers.
*   The "Router" documentation remains useful for the product target, but should be read as **roadmap** until this service is reintroduced.

#### 4. Inventiv Worker (Agent Sidecar)
*   Deployed on GPU instances.
*   Locally drives the inference engine (vLLM).
*   **Objective 0.2.1**: expose reliable **readiness** (`/readyz`) and report **heartbeats/capacity** to the control plane.

##### Worker ↔ Control Plane (via API / Gateway)
In the target architecture (staging/prod), the Worker **does not speak directly** to the orchestrator:
- The Worker calls the **API domain** (gateway) on:
  - `POST /internal/worker/register`
  - `POST /internal/worker/heartbeat`
- The API (or edge: Nginx/Caddy) **proxies** these endpoints to `inventiv-orchestrator`.

This allows:
- avoiding exposing the orchestrator publicly,
- keeping a stable `CONTROL_PLANE_URL` (API domain) in dev/staging/prod.

##### Auth Worker (token per instance + bootstrap)
The Worker authenticates with a token associated with an **instance** (`instances.id`):
- **DB Storage**: `worker_auth_tokens` table (token hash, prefix, timestamps).
- **Bootstrap**: on first `register`, if no token exists yet for `instance_id`,
  the orchestrator can generate a token and return it to the worker (plaintext **only** in the response).
- **Subsequent requests**: `Authorization: Bearer <token>` required (register/heartbeat).

Important:
- behind a proxy, the orchestrator uses `X-Forwarded-For` (otherwise fallback on socket IP).
- the edge/gateway must be configured to **overwrite** `X-Forwarded-For` on client side (anti-spoofing),
  or to only trust `X-Forwarded-For` from the internal network.

##### Auth User (session)
The UI + public "product" API are protected by a **session** (JWT cookie):
- `POST /auth/login` (login = username or email) → sets a session cookie
- `POST /auth/logout` → invalidates session on client side (cookie)
- `GET/PUT /auth/me` + `PUT /auth/me/password` (profile + password change)
- All business endpoints of `inventiv-api` are protected (401 without session).

Admin bootstrap:
- on startup, `inventiv-api` can create `username=admin` if absent
- password is read from `DEFAULT_ADMIN_PASSWORD_FILE` (secret file mounted in `/run/secrets`)

### Communication & Data Flow

1.  **Backend -> Orchestrator** :
    *   **State (Cold)** : The Backend writes intent in PostgreSQL (e.g., `INSERT INTO instances status='provisioning'`).
    *   **Event (Hot)** : The Backend publishes a Redis event (e.g., `CMD:PROVISION_INSTANCE`) for immediate Orchestrator wake-up, avoiding frequent polling.

2.  **Orchestrator -> Backend (via DB/Redis)** :
    *   The Orchestrator updates status in the DB (`Booting` -> `Ready`).
    *   The API exposes an **SSE** stream (`GET /events/stream`) and the UI subscribes (instances/actions) for near real-time refresh.

3.  **Monitoring & Scaling** :
    *   The Orchestrator collects metrics (Workers/Router) in real time.
    *   It alone decides Scaling actions (Up/Down) and notifies the system via DB update + Event.

---

## 2.1 "Simple" Multi-Machine Deployment (Docker Compose)

### Principle
- **1 control-plane machine**: `inventiv-api`, `inventiv-orchestrator`, `postgres`, `redis`
- **N GPU machines**: `inventiv-worker` (agent + vLLM) + model cache volume

### Network
Docker Compose does not manage a multi-host overlay network. We therefore use a private network between machines:
- recommended: **Tailscale**
- alternative: **WireGuard**

The Worker then sends its heartbeats to the control-plane via the private IP (tailnet).

### Data Model (Simplified)
*   **User**: Resource owner.
*   **Provider**: Cloud abstraction (e.g., Scaleway).
*   **InstanceType**: Hardware definition (e.g., "RENDER-S", 1 GPU L4, 24GB VRAM).
*   **LlmModel** (The "Model-Code"): Virtual service definition (e.g., "My-Llama3-Sales-Bot") mapped to a technical *model_id* (e.g., `meta-llama/Llama-3-70b-chat-hf`) and hardware constraints.
*   **Instance**: A real provisioned virtual machine.

---

## 3. Model Management Workflow (Lifecycle)

This workflow describes the complete process, from need definition ("I want to run Llama 3") to decommissioning.

### Phase 1: Definition & Matching
1.  **Intent**: The user wants to run a specific LLM model (e.g., Llama 3 70B).
2.  **Constraints**: They define Hardware needs:
    *   Number of GPUs (e.g., 2).
    *   VRAM per GPU required (e.g., 80GB total -> 2x40GB or 1x80GB).
3.  **Provider Choice**: The system (or user) filters providers capable of providing this type of resource (e.g., Scaleway via H100 or L40S Instances).

### Phase 2: Allocation & Provisioning
4.  **Targeting**: The user (via API/UI) selects allocation parameters (Geographic Zone, precise Instance Type).
5.  **Verification**: The system checks availability (Quota, Provider Stock via API, or internal constraints).
6.  **Provisioning**: The Orchestrator launches creation of real instances (`POST /instances/provision`).
    *   *Technical action*: Cloud API call (e.g., Scaleway `create_server` + `poweron`).
7.  **Health Check**: The instance starts, the `Worker Agent` initializes, downloads the model, and signals "READY" to the Orchestrator.

### Phase 3: Exposure & Routing
8.  **Routing Setup**: The instance is registered as "Active" for this specific model. The Router updates its routing table (Redis/In-Memory).
9.  **Virtual Model-Code**: The model is exposed publicly via a logical identifier (e.g., `gpt-4-killer`). This is the name clients will use, not the machine IP.
10. **Security (API Keys)**: Creation of API keys (`sk-...`) linked to this Virtual Service. This allows revoking access or changing underlying instances without changing client code.

### Phase 4: Usage & Scaling
11. **Consumption**: The client calls the API:
    ```bash
    curl https://api.inventiv.com/v1/chat/completions -d '{"model": "gpt-4-killer", ...}'
    ```
12. **Auto-Scaling**:
    *   *Scale Up*: If load increases (queue latency), the Orchestrator provisions new instances (back to step 6).
    *   *Scale Down*: If inactivity, surplus instances are drained then terminated.

### Phase 5: End of Life
13. **Decommissioning**:
    *   Deletion of Virtual Model-Code.
    *   The Orchestrator sends `terminate` order to **all** associated instances.
    *   Complete cleanup of Cloud resources (Disks, IPs) to stop billing.
    *   Archiving of usage logs.

---

## 4. API Endpoints (Current MVP State)

### Orchestrator (`:8001`)
*   `GET /admin/status`: cluster state (instances count, etc.).
*   Provisioning/termination are mainly triggered via **Redis Pub/Sub** (`CMD:*`) published by the API.

### Router (`:8002`)
*   **Not present** to date (see Router section).
