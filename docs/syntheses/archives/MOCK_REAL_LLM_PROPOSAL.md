# Proposal: Real LLM Model on Local Mock VMs

## Objective

Replace the current `mock-vllm` (simulation) with a **real lightweight LLM model** to test the complete end-to-end inference chain.

---

## Feasibility

### ✅ **VIABLE** for functional tests

**Objective**: Test the **complete cycle** of provisioning (creation → routing → request processing → destruction) with real inference responses, **not to measure performance**.

**Advantages**:
- ✅ Real test of the complete inference chain
- ✅ Test of OpenAI-compatible routing with real responses
- ✅ Validation of the complete provisioning cycle
- ✅ Test of real vLLM metrics (queue_depth, requests_running)
- ✅ Compatible with all platforms (Windows, Linux, macOS)

**Constraints**:
- ⚠️ Runs in CPU-only mode locally (5-15s per request)
- ⚠️ Requires 6-8GB RAM
- ⚠️ Longer startup time (60-120s to load the model)

**Note**: Performance tests are done with real GPU VMs in production (Scaleway, etc.).

---

## Recommended Models

### Option 1: Ultra-Lightweight Models (CPU-friendly)

#### **Qwen2.5-0.5B-Instruct** ⭐ **RECOMMENDED**
- **Size**: ~1GB (4-bit quantized)
- **RAM required**: ~2-4GB
- **CPU**: Works well on modern CPU (4+ cores)
- **Quality**: Good for functional tests
- **Format**: Hugging Face (compatible with vLLM, llama.cpp)
- **Advantage**: Already mentioned in code (`Qwen/Qwen2.5-0.5B-Instruct`)

#### **TinyLlama-1.1B**
- **Size**: ~600MB (4-bit quantized)
- **RAM required**: ~2-3GB
- **CPU**: Very fast on CPU
- **Quality**: Basic but sufficient for tests

#### **Phi-2 (Microsoft)**
- **Size**: ~1.5GB (4-bit quantized)
- **RAM required**: ~3-4GB
- **CPU**: Good performance
- **Quality**: Excellent for its size

### Option 2: Lightweight Models (GPU recommended)

#### **Llama-3.2-1B-Instruct**
- **Size**: ~2GB (FP16)
- **VRAM required**: ~2-3GB
- **GPU**: Recommended but can run on CPU
- **Quality**: Very good

#### **Gemma-2B (Google)**
- **Size**: ~4GB (FP16)
- **VRAM required**: ~4GB
- **GPU**: Recommended
- **Quality**: Excellent

---

## Implementation Options

### Option A: vLLM with quantized model (RECOMMENDED)

**Advantages**:
- ✅ Compatible with existing architecture
- ✅ Native OpenAI-compatible API
- ✅ Integrated Prometheus metrics
- ✅ GPU and CPU support (with limitations)

**Configuration**:
```yaml
# docker-compose.mock-runtime.yml
mock-vllm-real:
  image: vllm/vllm-openai:latest
  environment:
    - MODEL=Qwen/Qwen2.5-0.5B-Instruct
    - QUANTIZATION=awq  # or gptq for 4-bit
    - TENSOR_PARALLEL_SIZE=1
    - MAX_MODEL_LEN=2048
    - PORT=8000
  # Optional: GPU if available
  deploy:
    resources:
      reservations:
        devices:
          - driver: nvidia
            count: 1
            capabilities: [gpu]
  # Or CPU-only (slower but works)
  # No GPU required
```

**Resources**:
- **With GPU**: ~2GB VRAM (4-bit quantized)
- **Without GPU (CPU)**: ~4-8GB RAM, 4+ CPU cores

---

### Option B: llama.cpp with GGUF (CPU-optimized)

**Advantages**:
- ✅ Very efficient on CPU
- ✅ Low memory consumption
- ✅ Fast startup
- ✅ Aggressive quantizations possible (Q4_K_M, Q5_K_M)

**Disadvantages**:
- ⚠️ Requires HTTP wrapper (llama-cpp-python)
- ⚠️ OpenAI-compatible API via `llama-cpp-python[server]`

**Configuration**:
```yaml
mock-vllm-real:
  build:
    context: ./inventiv-worker-mock
    dockerfile: Dockerfile.llamacpp
  environment:
    - MODEL_PATH=/models/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf
    - N_CTX=2048
    - N_THREADS=4
    - PORT=8000
```

**Resources**:
- **CPU-only**: ~2-3GB RAM, 2-4 CPU cores
- **Very lightweight**: Q4_K_M model ~500MB

---

### Option C: Hugging Face Transformers + FastAPI (Simple)

**Advantages**:
- ✅ Simple to implement
- ✅ Compatible with all Hugging Face models
- ✅ Full control over API

**Disadvantages**:
- ⚠️ Slower than vLLM/llama.cpp
- ⚠️ Requires more RAM
- ⚠️ No integrated Prometheus metrics

**Resources**:
- **CPU**: ~4-6GB RAM, 4+ CPU cores
- **GPU**: ~2-4GB VRAM

---

## Recommendation: Option A (vLLM) with Qwen2.5-0.5B

### Why this option?

1. **Consistency**: Uses the same stack as production (vLLM)
2. **Compatibility**: Native OpenAI-compatible API
3. **Metrics**: Integrated Prometheus metrics (queue_depth, etc.)
4. **Multi-platform**: Works on Windows, Linux, macOS (CPU-only)
5. **Model**: Already mentioned in code (`Qwen/Qwen2.5-0.5B-Instruct`)
6. **Objective**: Functional tests of complete cycle, not performance

---

## Proposed Implementation

### 1. New Dockerfile for real vLLM

**File**: `inventiv-worker-mock/Dockerfile.vllm-real`

```dockerfile
FROM vllm/vllm-openai:latest

# Optional: install additional dependencies if needed
# RUN pip install --no-cache-dir ...

# vLLM starts automatically with environment variables
# No explicit CMD needed (defined in base image)
```

### 2. Docker Compose configuration

**File**: `docker-compose.mock-runtime.yml` (modified)

```yaml
services:
  # Option: use real vLLM instead of mock-vllm
  mock-vllm-real:
    image: vllm/vllm-openai:latest
    environment:
      - MODEL=${MOCK_VLLM_MODEL:-Qwen/Qwen2.5-0.5B-Instruct}
      - QUANTIZATION=${MOCK_VLLM_QUANTIZATION:-awq}  # awq, gptq, or "" for FP16
      - TENSOR_PARALLEL_SIZE=1
      - MAX_MODEL_LEN=2048
      - PORT=8000
      - HOST=0.0.0.0
      # Optional: Hugging Face token if private model
      - HF_TOKEN=${WORKER_HF_TOKEN:-}
    # GPU if available (optional, also works on CPU)
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: 1
              capabilities: [gpu]
    networks:
      - controlplane
    # Healthcheck to wait for model loading
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8000/v1/models"]
      interval: 10s
      timeout: 5s
      retries: 10
      start_period: 60s  # Give 60s to load the model

  worker-agent:
    # ... rest identical ...
    depends_on:
      mock-vllm-real:
        condition: service_healthy
```

### 3. Environment variable to switch

**File**: `inventiv-providers/src/mock.rs` (modified)

```rust
// Add environment variable to choose mock vs real
let use_real_vllm = std::env::var("MOCK_USE_REAL_VLLM")
    .unwrap_or_else(|_| "0".to_string())
    .parse::<i32>()
    .unwrap_or(0) > 0;

if use_real_vllm {
    // Use mock-vllm-real instead of mock-vllm
    compose_file = "docker-compose.mock-runtime-real.yml";
} else {
    compose_file = "docker-compose.mock-runtime.yml";
}
```

### 4. New compose file for real vLLM

**File**: `docker-compose.mock-runtime-real.yml`

```yaml
services:
  mock-vllm-real:
    image: vllm/vllm-openai:latest
    environment:
      - MODEL=${MOCK_VLLM_MODEL:-Qwen/Qwen2.5-0.5B-Instruct}
      - QUANTIZATION=${MOCK_VLLM_QUANTIZATION:-}
      - TENSOR_PARALLEL_SIZE=1
      - MAX_MODEL_LEN=2048
      - PORT=8000
      - HOST=0.0.0.0
    networks:
      - controlplane
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8000/v1/models"]
      interval: 10s
      timeout: 5s
      retries: 10
      start_period: 60s

  worker-agent:
    # Identical to docker-compose.mock-runtime.yml
    build:
      context: ./inventiv-worker
      dockerfile: Dockerfile.agent
    network_mode: "service:mock-vllm-real"
    environment:
      - CONTROL_PLANE_URL=http://api:8003
      - WORKER_AUTH_TOKEN=${WORKER_AUTH_TOKEN:-dev-worker-token}
      - INSTANCE_ID=${INSTANCE_ID:?INSTANCE_ID is required}
      - MODEL_ID=${MOCK_VLLM_MODEL_ID:-demo-model}
      - WORKER_SIMULATE_GPU_COUNT=${WORKER_SIMULATE_GPU_COUNT:-1}
      - WORKER_SIMULATE_GPU_VRAM_MB=${WORKER_SIMULATE_GPU_VRAM_MB:-24576}
      - VLLM_BASE_URL=http://127.0.0.1:8000
      - WORKER_HEALTH_PORT=8080
      - WORKER_VLLM_PORT=8000
      - WORKER_HEARTBEAT_INTERVAL_S=5
      - PYTHONUNBUFFERED=1
    depends_on:
      mock-vllm-real:
        condition: service_healthy

networks:
  controlplane:
    external: true
    name: ${CONTROLPLANE_NETWORK_NAME:?CONTROLPLANE_NETWORK_NAME is required}
```

---

## Required Resources

### Standard Configuration (CPU-only, Multi-platform)

**Compatible with**: Windows, Linux, macOS (Intel and Apple Silicon)

- **RAM**: 6-8GB available (Docker)
- **CPU**: 4+ cores
- **Disk**: ~2GB for the model
- **Startup time**: 60-120 seconds (model loading)
- **Latency**: 5-15 seconds per request

**Note**: This configuration is sufficient to test the complete cycle. Performance tests are done with real GPU VMs in production.

### Configuration with NVIDIA GPU (Optional)

If you have a local NVIDIA GPU (Linux/Windows only):

- **GPU**: NVIDIA with 2GB+ VRAM
- **RAM**: 4GB+ system
- **Startup time**: 10-30 seconds
- **Latency**: 0.1-1 second per request

**Note**: Not available on macOS (no CUDA).

---

## Advantages for Testing

### Functional Tests (Complete Cycle)

- ✅ **Real responses**: Test with real text generations
- ✅ **Complete cycle**: Creation → routing → processing → destruction
- ✅ **OpenAI routing**: Complete test of OpenAI-compatible proxy
- ✅ **Real metrics**: Queue depth, requests running, etc.
- ✅ **Quality**: Validation that responses are coherent

### Integration Tests

- ✅ **End-to-end**: Test of complete chain Worker → API → Frontend
- ✅ **Streaming**: Test of response streaming (if implemented)
- ✅ **Tokens**: Real token counting in/out
- ✅ **Errors**: Real error handling (timeout, OOM, etc.)

**Note**: **Performance** tests (latency, throughput) are done with real GPU VMs in production (Scaleway, etc.).

---

## Limitations

### CPU-only (Local)

- ⚠️ **Latency**: 5-15 seconds per request (CPU-only)
- ⚠️ **Throughput**: Limited (1-2 concurrent requests)
- ⚠️ **No GPU acceleration**: Uses CPU only

**Acceptable**: These limitations are expected and acceptable because the goal is to test the complete cycle, not performance.

### Performance Tests

Performance tests are done with **real GPU VMs in production**:
- Scaleway (NVIDIA GPU)
- Other providers to come
- Real latency: 0.1-1 second per request
- Real throughput: N concurrent requests depending on GPU

---

## Implementation Plan

### Phase 1: Basic setup (1-2h)

1. Create `docker-compose.mock-runtime-real.yml`
2. Test vLLM with Qwen2.5-0.5B locally
3. Validate that OpenAI API works

### Phase 2: Integration (2-3h)

1. Modify `inventiv-providers/src/mock.rs` to support choice
2. Add environment variable `MOCK_USE_REAL_VLLM`
3. Test with Mock provider

### Phase 3: E2E Tests (1-2h)

1. Run `make test-worker-observability` with real vLLM
2. Validate real metrics
3. Test inference requests

### Phase 4: Documentation (1h)

1. Document usage
2. Add configuration examples
3. Update README

---

## Final Recommendation

✅ **YES, it's viable and recommended** for:

1. **Functional tests**: Validate the complete chain
2. **Integration tests**: Validate the complete provisioning cycle
3. **Development**: Debug inference issues
4. **Demonstration**: Show real responses

**Recommended model**: **Qwen2.5-0.5B-Instruct** (works in CPU-only)

**Configuration**: vLLM CPU-only (works on Windows, Linux, macOS)

**Objective**: Functional tests of complete cycle, **not performance tests**. Performance tests are done with real GPU VMs in production.

**Next step**: ✅ **Already implemented** — Use `MOCK_USE_REAL_VLLM=1`
