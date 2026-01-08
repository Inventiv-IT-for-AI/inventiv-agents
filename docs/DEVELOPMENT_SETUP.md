# Local Development Guide - Multi-platform

## Overview

This guide explains how to set up the local development environment on different platforms: **Windows**, **Linux**, and **macOS**.

---

## Multi-platform Prerequisites

### Common to all platforms

- **Docker** & **Docker Compose** (v2.0+)
- **Git**
- **Make** (optional, but recommended for using `make` commands)

### Platform-specific

#### Windows

- **WSL2** (Windows Subsystem for Linux) recommended
- **Docker Desktop for Windows** with WSL2 backend
- **Make**: Install via `choco install make` or use Git Bash

#### Linux

- **Docker** and **Docker Compose** (plugin or standalone)
- **Make**: Usually pre-installed (`sudo apt install make` on Debian/Ubuntu)

#### macOS

- **Docker Desktop for Mac**
- **Make**: Pre-installed with Xcode Command Line Tools (`xcode-select --install`)

---

## Initial Setup

### 1. Clone the project

```bash
git clone https://github.com/Inventiv-IT-for-AI/inventiv-agents.git
cd inventiv-agents
```

### 2. Create configuration files

```bash
# Copy example files
cp env/dev.env.example env/dev.env

# Create admin secret (not committed)
mkdir -p deploy/secrets
echo "<your-admin-password>" > deploy/secrets/default_admin_password
```

### 3. Verify Docker

```bash
# Verify Docker is working
docker --version
docker compose version

# Verify Docker can access necessary resources
docker info | grep -i "memory\|cpus"
```

**Recommendation**: Allocate at least **8GB of RAM** to Docker (Settings → Resources → Memory).

---

## Quick Start

### Full stack (recommended)

```bash
# Start all services
make up

# Start the UI
make ui
```

**URLs**:
- Frontend: `http://localhost:3000`
- API: Not exposed by default (use `make api-expose` if needed)

### Individual services

```bash
# Start only infrastructure (db, redis, api, orchestrator)
docker compose up -d db redis api orchestrator

# Start UI separately
make ui
```

---

## Platform-specific Configuration

### Windows (WSL2)

**Recommended**: Use WSL2 with Docker Desktop.

```bash
# In WSL2
cd /mnt/c/path/to/inventiv-agents  # or your path

# Verify Docker is accessible from WSL2
docker ps

# Continue with standard commands
make up
```

**Notes**:
- File paths are automatically handled by Docker Desktop
- Docker volumes work correctly with WSL2
- Use Unix paths in WSL2 (`/mnt/c/...` to access C:\)

### Linux

**Standard configuration**:

```bash
# Verify Docker permissions
sudo usermod -aG docker $USER
# Logout/login required

# Start the stack
make up
```

**Notes**:
- Make sure Docker Compose is installed (plugin or standalone)
- File paths are standard Unix

### macOS

**Standard configuration**:

```bash
# Verify Docker Desktop
docker ps

# Start the stack
make up
```

**macOS-specific notes**:
- **File watching**: Automatically configured via `CHOKIDAR_USEPOLLING=1` in `docker-compose.yml`
- **Apple Silicon (M1/M2/M3)**: Compatible, Docker Desktop handles emulation automatically
- **Intel Mac**: Compatible without restrictions

---

## Testing with Real vLLM (Optional)

### Objective

Test the **complete cycle** of provisioning with real inference responses, **not to measure performance**.

**Note**: Performance tests are done with real GPU VMs in production (Scaleway, etc.).

### Activation

```bash
# Enable real vLLM (CPU-only, works on all platforms)
export MOCK_USE_REAL_VLLM=1
export MOCK_VLLM_MODEL="Qwen/Qwen2.5-0.5B-Instruct"

# Start the stack
make up

# Create a Mock instance → will use real vLLM
```

### Expected performance (CPU-only)

- **Loading time**: 60-120 seconds
- **Latency per request**: 5-15 seconds
- **RAM used**: 6-8GB

**Compatible with**: Windows, Linux, macOS (Intel and Apple Silicon)

---

## Multi-platform Compatibility

### ✅ Compatible features

| Feature | Windows | Linux | macOS |
|---------|---------|-------|-------|
| **Full stack** | ✅ | ✅ | ✅ |
| **Mock provider** | ✅ | ✅ | ✅ |
| **vLLM CPU-only** | ✅ | ✅ | ✅ |
| **E2E tests** | ✅ | ✅ | ✅ |
| **Frontend UI** | ✅ | ✅ | ✅ |

### ⚠️ Known limitations

| Limitation | Windows | Linux | macOS |
|------------|---------|-------|-------|
| **NVIDIA GPU** | ✅ (if NVIDIA GPU) | ✅ (if NVIDIA GPU) | ❌ (no CUDA) |
| **vLLM GPU** | ✅ (if NVIDIA GPU) | ✅ (if NVIDIA GPU) | ❌ (CPU-only) |
| **File watching** | ⚠️ (WSL2 recommended) | ✅ | ✅ (polling enabled) |

---

## Multi-platform Troubleshooting

### Common issues

#### Docker won't start

**Windows**:
```bash
# Verify Docker Desktop is running
# Verify WSL2 is enabled (Settings → General → Use WSL 2)
```

**Linux**:
```bash
# Verify Docker service is running
sudo systemctl status docker
sudo systemctl start docker
```

**macOS**:
```bash
# Verify Docker Desktop is running
# Restart Docker Desktop if necessary
```

#### "No space left on device" error

```bash
# Clean up unused Docker resources
make docker-prune-old

# Or manually
docker system prune -a --volumes
```

#### Port already in use

```bash
# Use a different PORT_OFFSET
PORT_OFFSET=10000 make up
```

#### File watching not working (macOS/Windows)

**Already configured**: `CHOKIDAR_USEPOLLING=1` in `docker-compose.yml` for macOS.

**Windows (WSL2)**: Should work automatically.

---

## Useful Commands

### Development

```bash
# Start the stack
make up

# Stop the stack (keeps volumes)
make down

# Stop and remove volumes
make nuke

# View logs
make logs

# Rebuild Rust services
docker compose build orchestrator api

# Tests
make test-worker-observability
```

### Debugging

```bash
# Logs for a specific service
docker compose logs -f api
docker compose logs -f orchestrator

# Shell into a container
docker compose exec api sh
docker compose exec db psql -U postgres -d llminfra
```

---

## Advanced Configuration

### Important environment variables

**File**: `env/dev.env`

| Variable | Description | Default |
|----------|-------------|---------|
| `PORT_OFFSET` | Port offset (multi-worktree) | `0` |
| `MOCK_USE_REAL_VLLM` | Use real vLLM (1) or mock (0) | `0` |
| `MOCK_VLLM_MODEL` | Hugging Face model | `Qwen/Qwen2.5-0.5B-Instruct` |
| `MOCK_VLLM_MAX_LEN` | Max context length | `1024` |

### Platform-specific customization

Configurations are **generic** and work on all platforms. No specific configuration is needed.

**Exception**: If you have an NVIDIA GPU (Linux/Windows), you can uncomment the GPU section in `docker-compose.mock-runtime-real.yml`.

---

## E2E Tests

### Full test (mock-vllm)

```bash
# Test with mock-vllm (simulation, fast)
make test-worker-observability
```

### Test with real vLLM

```bash
# Test with real vLLM (CPU-only, slower but tests complete cycle)
export MOCK_USE_REAL_VLLM=1
make test-worker-observability
```

**Note**: The test with real vLLM will take longer (60-120s to load the model).

---

## Contributing

### Recommended workflow

1. **Fork** the project
2. **Clone** your fork
3. **Create a branch**: `git checkout -b feature/my-feature`
4. **Develop** locally (Windows/Linux/macOS)
5. **Test**: `make test-worker-observability`
6. **Commit**: `git commit -m "feat: my feature"`
7. **Push**: `git push origin feature/my-feature`
8. **Create a PR**

### Code standards

- **Rust**: `cargo fmt` and `cargo clippy`
- **TypeScript**: Automatic formatting via Prettier
- **Commits**: Conventional commits (`feat:`, `fix:`, `chore:`, etc.)

---

## Support

### Documentation

- **Architecture**: `docs/architecture.md`
- **Domain Design**: `docs/domain_design.md`
- **Specifications**: `docs/specification_generale.md`
- **Real vLLM**: `docs/MOCK_REAL_VLLM_USAGE.md`

### Issues

If you encounter platform-specific issues:

1. Check logs: `make logs`
2. Check Docker configuration: `docker info`
3. Create a GitHub issue with:
   - Platform (Windows/Linux/macOS)
   - Docker version
   - Error logs

---

## Conclusion

The project is **fully compatible** with Windows, Linux, and macOS. No specific configuration is needed — everything works "out of the box" with Docker.

**Goal**: Enable all developers to contribute easily, regardless of their local platform.
