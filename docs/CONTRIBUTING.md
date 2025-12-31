# Contributing Guide

## Overview

This project is **open source** and welcomes contributions from all developers, regardless of their local platform (Windows, Linux, macOS).

---

## Prerequisites

### Common to all platforms

- **Docker** & **Docker Compose** v2.0+
- **Git**
- **Make** (optional, but recommended)

See [docs/DEVELOPMENT_SETUP.md](docs/DEVELOPMENT_SETUP.md) for platform-specific details.

---

## Contribution Workflow

### 1. Fork and Clone

```bash
# Fork the project on GitHub
# Then clone your fork
git clone https://github.com/YOUR-USERNAME/inventiv-agents.git
cd inventiv-agents
```

### 2. Create a branch

```bash
git checkout -b feature/my-feature
# or
git checkout -b fix/my-bug
```

### 3. Develop locally

```bash
# Start the stack
make up

# Start the UI
make ui

# Test your changes
make test-worker-observability
```

### 4. Commit and Push

```bash
# Use Conventional Commits
git commit -m "feat: add my feature"
git commit -m "fix: fix my bug"
git commit -m "docs: update documentation"

# Push to your fork
git push origin feature/my-feature
```

### 5. Create a Pull Request

- Go to GitHub
- Create a PR from your fork to the main repo
- Clearly describe the changes

---

## Code Standards

### Rust

```bash
# Formatting
cargo fmt --all

# Linting
cargo clippy --workspace

# Tests
cargo test --workspace
```

### TypeScript/JavaScript

- Automatic formatting via Prettier
- ESLint for linting

### Conventional Commits

Format: `<type>: <description>`

Types:
- `feat:`: New feature
- `fix:`: Bug fix
- `docs:`: Documentation
- `chore:`: Maintenance
- `refactor:`: Refactoring
- `test:`: Tests

---

## Testing

### E2E Tests

```bash
# Test with mock-vllm (fast)
make test-worker-observability

# Test with real vLLM (slower, tests complete cycle)
export MOCK_USE_REAL_VLLM=1
make test-worker-observability
```

### Unit Tests

```bash
# Rust
cargo test --workspace

# Frontend (if applicable)
npm -w inventiv-frontend test
```

---

## Multi-platform Compatibility

### ✅ Checks to perform

Before submitting a PR, verify that your code works on:

- ✅ **Windows** (WSL2 recommended)
- ✅ **Linux**
- ✅ **macOS** (Intel and Apple Silicon)

### ⚠️ Things to avoid

- ❌ Hardcoded paths (`/Users/...`, `C:\...`, etc.)
- ❌ Platform-specific commands (`ls`, `dir`, etc.)
- ❌ Dependencies on platform-specific tools

### ✅ Best practices

- ✅ Use Docker for isolation
- ✅ Use relative paths
- ✅ Use cross-platform commands (`docker compose`, `make`, etc.)
- ✅ Test on multiple platforms if possible

---

## Documentation

### Updating documentation

If you add/modify features:

1. Update `README.md` if necessary
2. Add/modify documentation in `docs/` if applicable
3. Document new environment variables

### Format

- Standard Markdown
- Code blocks with syntax highlighting
- Working command examples

---

## Questions?

- Create a GitHub issue for bugs
- Create a discussion for questions
- See `docs/DEVELOPMENT_SETUP.md` for local setup

---

## Thank you for contributing! 🎉
