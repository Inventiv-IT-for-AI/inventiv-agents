# Testing Guide

This document consolidates all testing strategies, plans, and guidelines for the Inventiv Agents platform.

## Overview

The Inventiv Agents platform uses a comprehensive testing strategy covering:
- **Backend (Rust)**: Unit tests, integration tests, and API tests
- **Frontend (Next.js)**: Component tests and E2E tests
- **Infrastructure**: Deployment and integration tests
- **End-to-End**: Full workflow validation

## Test Structure

### Backend Tests (Rust)

Located in `inventiv-api/tests/` and `inventiv-orchestrator/tests/`:

- **Unit tests**: Test individual functions and modules
- **Integration tests**: Test API endpoints and database interactions
- **Mock provider tests**: Test with mock infrastructure providers

Run tests:
```bash
make test              # Run all Rust tests
cargo test            # Run tests in current workspace
cargo test --workspace # Run all workspace tests
```

### Frontend Tests (Next.js)

- **Linting**: ESLint checks (`npm run lint:ui`)
- **Build validation**: Production build verification (`npm run build:ui`)
- **Component tests**: (To be implemented)

Run checks:
```bash
npm run lint:ui       # Lint frontend code
npm run build:ui      # Build frontend for production
```

### CI/CD Tests

Automated tests run on every PR and push to `main`:
- Format check (`cargo fmt --check`)
- Clippy linting (`cargo clippy`)
- Rust tests (`cargo test`)
- Frontend lint (`npm run lint:ui`)
- Frontend build (`npm run build:ui`)

See [CI/CD documentation](CI_CD.md) for details.

## Test Plans

### Chat Sessions and Inference

See [TEST_PLAN_CHAT_SESSIONS.md](TEST_PLAN_CHAT_SESSIONS.md) for detailed test scenarios covering:
- Chat session creation and management
- Message handling and streaming
- Model selection and inference
- Error handling and edge cases

### Storage Management

See [TEST_PLAN_STORAGE_MANAGEMENT.md](TEST_PLAN_STORAGE_MANAGEMENT.md) for detailed test scenarios covering:
- Volume discovery and attachment
- Volume lifecycle management
- Cleanup on instance termination
- Data persistence and recovery

## Local Testing

### Quick CI Check

Run the same checks as CI locally:
```bash
make ci-fast    # Fast checks (fmt, clippy, test, lint, build)
make ci         # Full CI including security checks
```

### Development Testing

```bash
# Backend
make test                    # Run Rust tests
make fmt-check              # Check formatting
make clippy                 # Run clippy

# Frontend
npm run lint:ui            # Lint frontend
npm run build:ui           # Build frontend
```

## Test Coverage

Current test coverage focuses on:
- ✅ Core API endpoints
- ✅ Authentication and authorization
- ✅ Instance lifecycle management
- ✅ Storage operations
- ⏳ Frontend components (to be expanded)
- ⏳ E2E workflows (to be expanded)

## Best Practices

1. **Write tests before fixing bugs**: Reproduce the bug in a test, then fix it
2. **Test edge cases**: Empty inputs, null values, boundary conditions
3. **Mock external dependencies**: Use mock providers for infrastructure tests
4. **Keep tests fast**: Unit tests should run quickly, integration tests can be slower
5. **Document test scenarios**: Use descriptive test names and comments

## Related Documentation

- [Engineering Guidelines](engineering_guidelines.md) - Code quality standards
- [CI/CD](CI_CD.md) - Continuous integration setup
- [Development Setup](DEVELOPMENT_SETUP.md) - Local development environment

