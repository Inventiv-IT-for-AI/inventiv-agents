# Multi-platform Compatibility

## Overview

The project is **fully compatible** with Windows, Linux, and macOS. No specific configuration is needed — everything works "out of the box" with Docker.

---

## Platform Compatibility

### ✅ Windows

- **CPU-only**: ✅ Works
- **NVIDIA GPU**: ✅ Supported (if NVIDIA GPU available)
- **WSL2**: Recommended for better performance
- **File watching**: Works with WSL2

### ✅ Linux

- **CPU-only**: ✅ Works
- **NVIDIA GPU**: ✅ Supported (if NVIDIA GPU available)
- **File watching**: ✅ Works natively

### ✅ macOS

- **CPU-only**: ✅ Works (Intel and Apple Silicon)
- **NVIDIA GPU**: ❌ Not supported (no CUDA)
- **Apple GPU (Metal)**: ❌ Not supported by vLLM
- **File watching**: ✅ Works (polling automatically enabled)

---

## vLLM Locally: CPU-only (Multi-platform)

### Objective

Test the **complete cycle** of provisioning (creation → routing → request processing → destruction) with real inference responses, **not to measure performance**.

**Important note**: Performance tests are done with real GPU VMs in production (Scaleway, etc.).

### Standard Configuration

**Works on**: Windows, Linux, macOS (Intel and Apple Silicon)

```bash
export MOCK_USE_REAL_VLLM=1
export MOCK_VLLM_MODEL="Qwen/Qwen2.5-0.5B-Instruct"
export MOCK_VLLM_MAX_LEN="1024"  # Optimized for CPU-only
```

### Expected Performance (CPU-only)

- **Loading time**: 60-120 seconds
- **Latency per request**: 5-15 seconds
- **RAM used**: 6-8GB

**Acceptable**: These performance characteristics are expected and acceptable because the goal is to test the complete cycle, not performance.

---

## Configuration with NVIDIA GPU (Optional)

If you have a local NVIDIA GPU (Linux/Windows only):

**Uncomment** in `docker-compose.mock-runtime-real.yml`:

```yaml
deploy:
  resources:
    reservations:
      devices:
        - driver: nvidia
          count: 1
          capabilities: [gpu]
```

**Expected performance**:
- Loading time: 10-30 seconds
- Latency per request: 0.1-1 second
- VRAM used: 2-4GB

**Note**: Not available on macOS (no CUDA).

---

## Options Comparison

| Option | Latency | RAM | GPU Used | Compatibility |
|--------|---------|-----|----------|---------------|
| **mock-vllm** | <10ms | <100MB | ❌ No | ✅ All platforms |
| **vLLM CPU-only** | 5-15s | 6-8GB | ❌ No | ✅ Windows/Linux/macOS |
| **vLLM NVIDIA GPU** | 0.1-1s | 4GB | ✅ CUDA | ✅ Linux/Windows (if GPU) |

---

## Recommendations by Use Case

### Fast Functional Tests

**Use `mock-vllm`** (default):
- ✅ Fast (<10ms)
- ✅ No resources needed
- ✅ Sufficient for functional tests

### Complete Cycle Tests

**Use `vLLM CPU-only`**:
- ✅ Works on all platforms
- ✅ Same stack as production
- ⚠️ Slow (5-15s) but sufficient to validate the complete cycle
- ✅ No GPU needed

### Performance Tests

**Use real GPU VMs in production**:
- Scaleway (NVIDIA GPU)
- Other providers to come
- Real latency: 0.1-1 second per request
- Real throughput: N concurrent requests depending on GPU

---

## Docker Configuration

### Docker Memory

**Recommendation**: Allocate at least **8GB of RAM** to Docker

**Windows**: Docker Desktop → Settings → Resources → Memory → 8GB+
**Linux**: Check with `docker info | grep -i memory`
**macOS**: Docker Desktop → Settings → Resources → Memory → 8GB+

### File Watching

**Already configured**: `CHOKIDAR_USEPOLLING=1` in `docker-compose.yml` to improve reliability on macOS/Windows.

---

## Multi-platform Troubleshooting

### Common issues

#### Docker won't start

**Windows**:
- Verify Docker Desktop is running
- Verify WSL2 is enabled (Settings → General → Use WSL 2)

**Linux**:
```bash
sudo systemctl status docker
sudo systemctl start docker
```

**macOS**:
- Verify Docker Desktop is running
- Restart Docker Desktop if necessary

#### Port already in use

```bash
# Use a different PORT_OFFSET
PORT_OFFSET=10000 make up
```

#### "No space left on device" error

```bash
# Clean up Docker resources
make docker-prune-old
```

---

## Conclusion

**Recommended configuration for all developers**:

1. **Fast functional tests**: Use `mock-vllm` (default)
2. **Complete cycle tests**: Use `vLLM CPU-only` with `MOCK_USE_REAL_VLLM=1`
3. **Performance tests**: Use real GPU VMs in production

**No specific configuration needed** — everything works "out of the box" on Windows, Linux, and macOS.

See [docs/DEVELOPMENT_SETUP.md](docs/DEVELOPMENT_SETUP.md) for the detailed configuration guide by platform.
