# Implementation: Real vLLM with Qwen2.5-0.5B-Instruct

## Summary

Complete implementation of support for using a **real LLM model** (vLLM) with the Mock provider, enabling end-to-end inference chain testing.

---

## Created/Modified Files

### ✅ New files

1. **`docker-compose.mock-runtime-real.yml`**
   - Docker Compose configuration for real vLLM
   - Uses `vllm/vllm-openai:latest` image
   - Optional GPU support (commented by default)
   - Healthcheck to wait for model loading

2. **`docs/MOCK_REAL_VLLM_PROPOSAL.md`**
   - Feasibility analysis
   - Options comparison (vLLM, llama.cpp, Transformers)
   - Model recommendations

3. **`docs/MOCK_REAL_VLLM_USAGE.md`**
   - Complete usage guide
   - Configuration and examples
   - Troubleshooting

4. **`docs/MOCK_REAL_VLLM_IMPLEMENTATION.md`** (this file)
   - Technical documentation of the implementation

### ✅ Modified files

1. **`inventiv-providers/src/mock.rs`**
   - Added support to choose between mock-vllm and real vLLM
   - Environment variable `MOCK_USE_REAL_VLLM`
   - Passing vLLM configuration variables
   - Improved `stop_runtime()` to clean up both runtime types

2. **`README.md`**
   - Added section on real vLLM with Mock
   - Link to documentation

---

## Architecture

### Flow with Mock-vllm (default)

```
Mock Instance → docker-compose.mock-runtime.yml
  ├─ mock-vllm (simulation)
  └─ worker-agent
```

### Flow with Real vLLM (MOCK_USE_REAL_VLLM=1)

```
Mock Instance → docker-compose.mock-runtime-real.yml
  ├─ mock-vllm-real (vLLM with Qwen2.5-0.5B)
  └─ worker-agent
```

---

## Environment Variables

### Activation

```bash
export MOCK_USE_REAL_VLLM=1  # 0 = mock, 1 = real vLLM
```

### vLLM Configuration

```bash
export MOCK_VLLM_MODEL="Qwen/Qwen2.5-0.5B-Instruct"
export MOCK_VLLM_QUANTIZATION=""  # awq, gptq, or "" for FP16
export MOCK_VLLM_MAX_LEN="2048"
export WORKER_HF_TOKEN=""  # If private model
export MOCK_VLLM_TRUST_REMOTE_CODE="true"
```

---

## Technical Details

### Compose file selection

**File**: `inventiv-providers/src/mock.rs`

```rust
let use_real_vllm = std::env::var("MOCK_USE_REAL_VLLM")
    .unwrap_or_else(|_| "0".to_string())
    .parse::<i32>()
    .unwrap_or(0) > 0;

let compose_file = if use_real_vllm {
    format!("{}/docker-compose.mock-runtime-real.yml", project_root)
} else {
    format!("{}/docker-compose.mock-runtime.yml", project_root)
};
```

### Variable passing

vLLM variables are passed to the Docker Compose container:

```rust
if use_real_vllm {
    if let Ok(model) = std::env::var("MOCK_VLLM_MODEL") {
        cmd.env("MOCK_VLLM_MODEL", &model);
    }
    // ... other variables
}
```

### Cleanup

The `stop_runtime()` function now tries both compose files to ensure cleanup:

```rust
let compose_files = vec![
    format!("{}/docker-compose.mock-runtime.yml", project_root),
    format!("{}/docker-compose.mock-runtime-real.yml", project_root),
];

for compose_file in compose_files {
    // Try to stop with each file
}
```

---

## Docker Compose Configuration

### Healthcheck

The healthcheck waits for vLLM to be ready before starting the worker-agent:

```yaml
healthcheck:
  test: ["CMD-SHELL", "python3 -c \"import urllib.request; urllib.request.urlopen('http://localhost:8000/v1/models').read()\" || exit 1"]
  interval: 10s
  timeout: 5s
  retries: 10
  start_period: 90s  # Give 90s to load the model
```

### GPU (optional)

To enable GPU support, uncomment in `docker-compose.mock-runtime-real.yml`:

```yaml
deploy:
  resources:
    reservations:
      devices:
        - driver: nvidia
          count: 1
          capabilities: [gpu]
```

---

## Tests

### Manual test

```bash
# 1. Enable real vLLM
export MOCK_USE_REAL_VLLM=1

# 2. Start the stack
make up

# 3. Create a Mock instance (via UI or API)
# Wait 30-90 seconds for model loading

# 4. Verify vLLM is ready
docker ps | grep mockrt-
docker logs <container-id> | grep -i "ready\|model"

# 5. Test a request
curl -X POST http://127.0.0.1:8003/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <api-key>" \
  -d '{
    "model": "demo-model-<instance-id>",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

### E2E test

```bash
export MOCK_USE_REAL_VLLM=1
make test-worker-observability
```

**Note**: The test will take longer (90s+ to load the model).

---

## Known Limitations

1. **Startup time**: 30-90 seconds to load the model (vs <1s for mock)

2. **Resources**: Requires 4-8GB RAM (CPU) or GPU with 2GB+ VRAM

3. **First download**: Model is downloaded from Hugging Face (may take several minutes)

4. **CPU latency**: On CPU, latency of 1-5 seconds per request (vs <10ms for mock)

5. **Docker cache**: Models are cached, but require disk space

---

## Possible Next Steps

1. **Automatic GPU support**: Automatically detect if GPU available
2. **Pre-downloaded models**: Docker volume for model cache
3. **Automatic quantization**: Automatically choose best quantization based on GPU
4. **Automated tests**: Add CI/CD tests with real vLLM
5. **Monitoring**: Specific metrics for real vLLM (loading time, latency, etc.)

---

## Conclusion

✅ **Complete and functional implementation**

The system now allows easy switching between mock-vllm (simulation) and real vLLM for tests. Configuration is flexible and documented.

**Recommended usage**:
- **Mock-vllm**: For fast tests and development
- **Real vLLM**: For complete integration tests and performance validation
