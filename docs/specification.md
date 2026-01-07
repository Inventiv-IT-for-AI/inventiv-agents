# General Specification: Scalable LLM Inference Infrastructure

## 1. Introduction

This document defines the architecture and features of an orchestration platform for Language Model (LLM) inference. The goal is to provide a scalable, hybrid (Cloud + Bare-Metal) and cost-effective infrastructure, capable of dynamically managing workloads.

## 2. Modular Architecture

The system is divided into 6 strictly independent micro-services/components:

### 2.1 Front-End (UI Platform)
*   **Role**: User interface for managing the Inventiv-Agents platform.
*   **Tech**: React/Next.js (or similar).
*   **Responsibilities**: Dashboard, Agent Configuration, Cost/metrics visualization. Interacts only with the *Backend API*.

### 2.2 Backend API (Business Logic)
*   **Role**: Business logic core of the Inventiv-Agents platform.
*   **Responsibilities**: User management, projects, billing, and business logic.
*   **Interaction**: Publishes commands (Events) to the *Orchestrator* and queries the DB for state.

### 2.3 Database (Persistence)
*   **Role**: Centralized and persistent storage.
*   **Tech**: PostgreSQL (Relational) + Redis (Cache/Queue) + TimescaleDB (Time series/Metrics).
*   **Data**: Server states, configurations, request logs, users.

### 2.4 Orchestrator (Control Plane)
*   **Role**: Infrastructure lifecycle manager (dynamic Infrastructure-as-Code).
*   **Responsibilities**:
    *   **Provisioning**: Creation of Cloud/Bare-Metal servers.
    *   **Configuration**: Deployment of Worker containers.
    *   **Scaling**: Decision to add/remove nodes.
    *   **Health**: Infrastructure health monitoring (Heartbeat).

### 2.5 Router / Gateway (Data Plane)
*   **Role**: Single entry point for inference requests.
*   **Responsibilities**:
    *   **Routing**: Intelligent distribution (Load Balancing) to available Workers.
    *   **Tracking**: Request logging, token counting.
    *   **Evaluation**: Real-time load analysis to inform the Orchestrator.
    *   **Security**: Auth (API Keys), Rate Limiting.

### 2.6 Worker Container (vLLM Agent)
*   **Role**: Execution unit deployed on each GPU server.
*   **Base**: Docker + vLLM + Lightweight Python agent.
*   **Features**:
    *   Compatible Cloud & Bare-Metal.
    *   Model download/caching.
    *   GPU memory management (Parallelism).
    *   Metrics reporting (GPU Load, Queue Depth).
    *   Batch & Streaming processing.

## 3. Data Flow

1.  **Control Flow**: Backend -> Redis (Events) -> Orchestrator -> (Provisioning) -> Worker.
2.  **Inference Flow**: Client -> Router -> Worker -> (LLM) -> Worker -> Router -> Client.
3.  **Monitoring Flow**: Worker metric -> Router/Orchestrator -> DB -> Backend/UI.

> Note (MVP repo): The UI receives near real-time updates via **SSE** from the API (instances + action logs).

## 4. State Machine & Progress Tracking

### 4.1 State Machine

The system uses an **explicit state machine** to manage instance lifecycle:

**States**: `provisioning` → `booting` → `ready` → `draining` → `terminating` → `terminated` → `archived`

**Transitions**: Managed by explicit functions in `inventiv-orchestrator/src/state_machine.rs`:
- `booting_to_ready`: Health check succeeded
- `booting_to_startup_failed`: Timeout or critical error
- `terminating_to_terminated`: Deletion confirmed
- `mark_provider_deleted`: Orphan detection

**History**: All transitions are recorded in `instance_state_history`.

See [docs/STATE_MACHINE_AND_PROGRESS.md](STATE_MACHINE_AND_PROGRESS.md) for more details.

### 4.2 Progress Tracking (0-100%)

The system automatically calculates a **progress percentage** based on completed actions:

**Steps**:
- **provisioning (0-20%)**: Request created (5%), Provider create (20%)
- **booting (20-100%)**: Provider start (30%), IP assigned (40%), SSH install (50%), vLLM HTTP (60%), Model loaded (75%), Warmup (90%), Health check (95%), Ready (100%)

**Module**: `inventiv-api/src/progress.rs`

See [docs/STATE_MACHINE_AND_PROGRESS.md](STATE_MACHINE_AND_PROGRESS.md) for more details.

## 5. Infrastructure Management (Modular Provisioning)

### 5.1 "Provider Adapters" Pattern
The Orchestrator uses specific modules (Adapters) to communicate with each provider.
Each Adapter encapsulates the provider's complexity (Proprietary API, CLI, or Terraform).

*   **Scaleway Adapter**: Uses `scaleway-cli` or Python API to command GPU instances and manage Flexible IPs.
*   **AWS Adapter**: Uses `boto3` for EC2.
*   **Bare-Metal Adapter**: Uses SSH/Ansible or PXE Boot to configure physical machines.

### 5.2 Server Lifecycle
Tools considered: Terraform or direct API calls (Provider SDK).
*   **Creation**: Specification of instance type, OS image.
*   **Initialization**: Launch of a *Node Agent* at startup.

## 6. Agent Version Management & Integrity

### 6.1 Versioning

The `agent.py` file contains version constants:
- `AGENT_VERSION`: Version number (e.g., "1.0.0")
- `AGENT_BUILD_DATE`: Build date (e.g., "2026-01-03")

### 6.2 SHA256 Checksum

- **Automatic calculation**: `_get_agent_checksum()` function calculates SHA256
- **Verification**: SSH bootstrap script verifies checksum if `WORKER_AGENT_SHA256` is defined
- **Endpoint `/info`**: Exposes version, build date, and checksum

### 6.3 Monitoring

- **Heartbeats**: Include `agent_info` with version/checksum
- **Health checks**: Verify `/info` and log information
- **Detection**: Problems detected automatically (incorrect version, invalid checksum)

### 6.4 CI/CD

- **Makefile**: Commands `agent-checksum`, `agent-version-bump`, `agent-version-check`
- **GitHub Actions**: Workflow `agent-version-bump.yml` for automatic bump
- **CI**: Verifies that version is up to date if `agent.py` has changed

See [docs/AGENT_VERSION_MANAGEMENT.md](AGENT_VERSION_MANAGEMENT.md) for more details.

## 7. Storage Management

### 7.1 Automatic Discovery

The system automatically discovers all attached volumes:
- **During creation**: After `PROVIDER_CREATE`
- **During termination**: Before deletion to ensure none are forgotten

### 7.2 Tracking

All volumes are tracked in `instance_volumes`:
- `provider_volume_id`: Unique identifier at the provider
- `volume_type`: Type (b_ssd, l_ssd, etc.)
- `delete_on_terminate`: Flag for automatic deletion
- `is_boot`: Indicates if it's a boot volume

### 7.3 Automatic Deletion

During termination:
1. Discovery of all attached volumes
2. Marking for deletion (`delete_on_terminate=true`)
3. Sequential deletion via `PROVIDER_DELETE_VOLUME`
4. Logging of each deletion

### 7.4 Special Cases

- **Boot volumes**: Automatically created by Scaleway, discovered and tracked
- **Local volumes**: Detection and rejection for types requiring boot diskless (L40S, L4)
- **Persistent volumes**: `delete_on_terminate=false` to preserve volumes

See [docs/STORAGE_MANAGEMENT.md](STORAGE_MANAGEMENT.md) for more details.

## 8. Model Management & Worker Flavors

### 8.1 Worker Flavors (Configuration by Provider/Family)
The Worker container is not monolithic. It adapts via configurable "Profiles" or "Flavors" at build or runtime:
*   **Flavor**: `NVIDIA-H100` -> Docker image optimized for Hopper, CUDA 12.x drivers.
*   **Flavor**: `AMD-MI300` -> ROCm-specific image.
*   **Provider Specifics**:
    *   *Scaleway*: Specific network configuration (Private Network), Block Storage volume mounting.
    *   *AWS*: S3 integration for model cache, EFA for networking.

### 8.2 Installation
*   Use of containers (Docker) for model isolation.
*   The *Node Agent* receives its config at startup (Env Variables injected by the Adapter).

### 8.3 Health Checks
*   **Liveness Probe**: Is the container running?
*   **Readiness Probe**: Is the model loaded in VRAM and ready to respond? (API call `/readyz` or `/v1/models`).
*   **Agent Info Check**: Version and checksum verification via `/info` endpoint.
*   **Heartbeat Priority**: Recent heartbeats (< 30s) take priority over active checks.
*   The Orchestrator only routes traffic to "Ready" nodes.

> **See**: [docs/STATE_MACHINE_AND_PROGRESS.md](STATE_MACHINE_AND_PROGRESS.md) for details on health checks and [docs/AGENT_VERSION_MANAGEMENT.md](AGENT_VERSION_MANAGEMENT.md) for the `/info` endpoint.

## 9. Load Distribution (Load Balancing)

### 9.1 Strategy
*   **Algorithm**: Least Outstanding Requests (LOR) or Queue Depth weighted by GPU capacity.
*   **Session Stickiness**: Optional, for context caching (KV Cache reuse), routing to the same node if possible.

### 9.2 Queue Management
A global queue in the Gateway to buffer requests in case of momentary saturation before scale-up.

## 10. Auto-scaling

### 10.1 Scale-Up (Provisioning)
Triggers (Configurable):
*   `Avg_Queue_Wait_Time` > threshold (e.g., 5s).
*   `Total_Active_Requests` / `Total_GPU_Count` > saturation.
Action: Order X new servers of the appropriate type.

### 10.2 Scale-Down (Release)
Triggers:
*   `GPU_Utilization` < threshold (e.g., 20%) for N minutes.
*   Respect `Min_Instance_Count`.
Action: Drain connections (no longer send new requests) -> Kill container -> Shutdown/Destroy server.

## 11. Bare-Metal Integration (Hybrid)

### 11.1 Agent-Based Architecture
For "On-Premise" or third-party Bare-Metal machines:
*   Installation of a lightweight binary (Agent) authenticated by Token/mTLS.
*   The agent opens a tunnel (e.g., **SSE/HTTP long-poll**, gRPC stream or WireGuard VPN) to the Control Plane to avoid exposing public ports.

### 11.2 Security & Multi-tenant/Sharing
*   Strict containerization isolation.
*   Data encryption in transit.
*   If shared: The machine owner can define time slots or allocation quotas to the global cluster.

## 12. Monitoring & Billing

### 12.1 Metrics (Prometheus/Grafana Stack)
*   **Infrastructure**: CPU, RAM, Disk, GPU Utils, GPU Memory, Temperature.
*   **Service**: Request Latency (TTFT - Time To First Token), Throughput (Tokens/sec), Error Rate, Queue Length.

### 12.2 Costs & Consumption
*   **Server Cost**: Tracking of uptime per instance * Unit price Provider.
*   **Client Consumption**: Logging of each request (Input Tokens, Output Tokens, Model ID).
*   **Dashboard**: Real-time aggregated view.

## 13. Technologies (Rust Stack)
*   **Language**: Rust (Performance, reliability, typing).
*   **Web Framework**: Axum (Backend, Orchestrator, Router).
*   **Http Client**: Reqwest.
*   **Async Runtime**: Tokio.
*   **Database**: PostgreSQL with `sqlx` (Asynchronous & typed Query builder).
*   **Inference (Nodes)**: vLLM (Python/C++) driven by a Rust or Python Agent (Sidecar).
*   **Structure**: Cargo Workspace (Monorepo to share types and DTOs).

