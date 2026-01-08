# Using Real vLLM with Mock Provider

## Overview

The Mock provider can now use a **real LLM model** (vLLM) instead of the simulated mock-vllm.

**Objective**: Test the complete provisioning cycle (creation → routing → request processing → destruction) with real inference responses, **not to measure performance**.

**Important note**: Real performance tests are done with real GPU VMs at Scaleway and other providers. Local vLLM (CPU-only) is only used to validate the complete chain functionality.

---

## Activation

### Environment variable

To enable real vLLM instead of mock, set:

```bash
export MOCK_USE_REAL_VLLM=1
```

**Default**: `MOCK_USE_REAL_VLLM=0` (uses simulated mock-vllm)

---

## Configuration

### Available environment variables

| Variable | Description | Default |
|----------|-------------|---------|
| `MOCK_USE_REAL_VLLM` | Enable real vLLM (1) or mock (0) | `0` |
| `MOCK_VLLM_MODEL` | Hugging Face model to load | `Qwen/Qwen2.5-0.5B-Instruct` |
| `MOCK_VLLM_QUANTIZATION` | Quantization (awq, gptq, or "" for FP16) | `""` |
| `MOCK_VLLM_MAX_LEN` | Maximum context length | `2048` |
| `WORKER_HF_TOKEN` | Hugging Face token (for private models) | - |
| `MOCK_VLLM_TRUST_REMOTE_CODE` | Allow remote code | `true` |

### Configuration examples

#### Minimal configuration (CPU-only)

```bash
export MOCK_USE_REAL_VLLM=1
export MOCK_VLLM_MODEL="Qwen/Qwen2.5-0.5B-Instruct"
# No GPU required, works on CPU (slower)
```

#### Configuration with GPU

```bash
export MOCK_USE_REAL_VLLM=1
export MOCK_VLLM_MODEL="Qwen/Qwen2.5-0.5B-Instruct"
export MOCK_VLLM_QUANTIZATION="awq"  # 4-bit quantization to save VRAM
# Requires NVIDIA GPU with 2GB+ VRAM
```

#### Configuration with custom model

```bash
export MOCK_USE_REAL_VLLM=1
export MOCK_VLLM_MODEL="microsoft/Phi-2"
export MOCK_VLLM_MAX_LEN="4096"
export WORKER_HF_TOKEN="hf_..."  # If private model
```

---

## Usage

### 1. Start stack with real vLLM

```bash
# Enable real vLLM
export MOCK_USE_REAL_VLLM=1

# Optional: configure model
export MOCK_VLLM_MODEL="Qwen/Qwen2.5-0.5B-Instruct"

# Start the stack
make up
```

### 2. Create a Mock instance

The Mock instance will automatically use real vLLM if `MOCK_USE_REAL_VLLM=1` is set.

```bash
# Via UI: create a Mock instance
# Or via API:
curl -X POST http://127.0.0.1:8003/deployments \
  -H "Content-Type: application/json" \
  -b cookies.txt \
  -d '{
    "instance_type_id": "...",
    "zone_id": "...",
    "model_id": "..."
  }'
```

### 3. Wait for model loading

The model takes **30-90 seconds** to load depending on:
- CPU vs GPU
- Model size
- Quantization

**Verification**:
```bash
# Check Mock runtime logs
docker ps | grep mockrt-
docker logs <container-id>

# Verify vLLM is ready
curl http://<instance-ip>:8000/v1/models
```

### 4. Test inference

```bash
# Via API's OpenAI proxy
curl -X POST http://127.0.0.1:8003/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <api-key>" \
  -d '{
    "model": "demo-model-<instance-id>",
    "messages": [
      {"role": "user", "content": "Hello, how are you?"}
    ]
  }'
```

**Expected response**: A real text generation from the model, not just an echo.

---

## Required Resources

### Standard Configuration (CPU-only, Multi-platform)

**Compatible with**: Windows, Linux, macOS (Intel and Apple Silicon)

- **RAM**: 6-8GB available (Docker)
- **CPU**: 4+ cores
- **Disk**: ~2GB for the model
- **Startup time**: 60-120 seconds
- **Latency**: 5-15 seconds per request

**Note**: vLLM runs in CPU-only mode on all local platforms (Windows, Linux, macOS). Performance is not the goal here — the goal is to test the complete provisioning cycle and real request processing. Performance tests are done with real GPU VMs in production (Scaleway, etc.).

### Configuration with NVIDIA GPU (Optional)

If you have a local NVIDIA GPU (Linux/Windows):

- **GPU**: NVIDIA with 2GB+ VRAM
- **RAM**: 4GB+ system
- **Startup time**: 10-30 seconds
- **Latency**: 0.1-1 second per request

**Note**: Uncomment the GPU section in `docker-compose.mock-runtime-real.yml` if you have an NVIDIA GPU available.

### Optimal Configuration

- **GPU**: NVIDIA with 4GB+ VRAM (RTX 3060, RTX 4060, etc.)
- **RAM**: 8GB+ system
- **Disk**: SSD recommended
- **Startup time**: 5-15 seconds
- **Latency**: <0.5 second per request

---

## Recommended Models

### For CPU-only

1. **Qwen2.5-0.5B-Instruct** ⭐ (recommended)
   - Size: ~1GB (quantized)
   - RAM: 2-4GB
   - Quality: Good

2. **TinyLlama-1.1B**
   - Size: ~600MB (quantized)
   - RAM: 2-3GB
   - Quality: Basic

### For GPU

1. **Qwen2.5-0.5B-Instruct** (AWQ/GPTQ quantized)
   - VRAM: ~1-2GB
   - Quality: Good

2. **Llama-3.2-1B-Instruct**
   - VRAM: ~2-3GB
   - Quality: Very good

3. **Phi-2**
   - VRAM: ~2GB
   - Quality: Excellent for its size

---

## Troubleshooting

### Model won't load

**Symptom**: The `mock-vllm-real` container stays in "starting" or crashes.

**Solutions**:
1. Check logs: `docker logs <container-id>`
2. Increase Docker memory: Settings → Resources → Memory → 8GB+
3. Verify `MOCK_VLLM_MAX_LEN=1024` (already configured by default)
4. Use a smaller model if necessary

### "Out of memory" error

**Symptom**: Container crashes with OOM.

**Solutions**:
1. Increase Docker memory to 8GB+ (Settings → Resources → Memory)
2. Verify `MOCK_VLLM_MAX_LEN=1024` (already configured)
3. Use a smaller model if necessary

### Very high latency (CPU-only)

**Symptom**: Requests take 5-15 seconds.

**Normal**: This is expected in CPU-only mode. The goal is to test the complete cycle, not performance.

**Note**: Performance tests are done with real GPU VMs in production.

### Model not found

**Symptom**: "Model not found" or "401 Unauthorized" error.

**Solutions**:
1. Verify the model exists on Hugging Face
2. If private model, set `WORKER_HF_TOKEN`
3. Check internet connection (model download)

---

## Mock vs Real Comparison

| Aspect | Mock-vllm | Real vLLM (CPU-only) |
|--------|-----------|----------------------|
| **Startup** | <1 second | 60-120 seconds |
| **Responses** | Echo of prompt | Real generation |
| **Latency** | <10ms | 5-15 seconds (CPU) |
| **Resources** | Minimal (<100MB) | 6-8GB RAM |
| **Metrics** | Simulated | Real |
| **Objective** | Fast functional tests | Complete cycle tests with real responses |
| **Performance** | N/A | ⚠️ Not optimized (CPU-only) |

**Note**: Real vLLM locally is used to **validate the complete cycle**, not to measure performance. Performance tests are done with real GPU VMs in production.

---

## Test Examples

### Basic functional test

```bash
# Enable real vLLM
export MOCK_USE_REAL_VLLM=1

# Start the stack
make up

# Create a Mock instance (via UI or API)
# Wait for model to load (check logs)

# Test a request
curl -X POST http://127.0.0.1:8003/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <api-key>" \
  -d '{
    "model": "demo-model-<instance-id>",
    "messages": [
      {"role": "user", "content": "What is the capital of France?"}
    ],
    "max_tokens": 50
  }'
```

### Test with E2E test

```bash
# Enable real vLLM
export MOCK_USE_REAL_VLLM=1

# Run E2E test (will be slower with real vLLM)
make test-worker-observability
```

**Note**: The E2E test will take longer because it needs to wait for the model to load.

---

## Important Notes

1. **First startup**: The model is downloaded from Hugging Face (may take several minutes depending on connection)

2. **Cache**: Models are cached in Docker, subsequent startups will be faster

3. **NVIDIA GPU**: If you have an NVIDIA GPU (Linux/Windows), uncomment the `deploy.resources` section in `docker-compose.mock-runtime-real.yml`

4. **macOS**: vLLM runs in CPU-only mode only (no CUDA/Metal support)

5. **Production**: Real vLLM locally is for functional tests. In production, use real GPU instances with vLLM.

6. **Metrics**: With real vLLM, GPU metrics can be real if NVIDIA GPU available, otherwise they remain simulated.

7. **Multi-platform**: The configuration works on Windows, Linux, and macOS without modification.

---

## Support

For more information:
- vLLM documentation: https://docs.vllm.ai/
- Hugging Face models: https://huggingface.co/models
- Inventiv Agents documentation: `docs/OBSERVABILITY_ANALYSIS.md`
