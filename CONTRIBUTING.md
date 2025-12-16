# Contributing to Inventiv Agents

Thank you for your interest in contributing to the project.

## Getting Started

1.  **Fork** the repository.
2.  **Clone** your fork locally.
3.  **Install dependencies**:
    *   Docker & Docker Compose
    *   Rust (Cargo)
    *   Node.js (for the frontend)
4.  **Run the checks**: `make check` and `make test`.

## Development Flow

1.  Create a feature branch: `git checkout -b feat/my-awesome-feature`.
2.  Make your changes.
3.  Ensure everything works:
    - `make up` to run the local stack (API/DB/Redis/Orchestrator/FinOps)
    - `make ui` to run the Next.js UI
4.  Commit your changes using [Conventional Commits](https://www.conventionalcommits.org/).
5.  Push and open a **Pull Request**.

## Architecture & Standards

*   **Language**: Rust (2021 Edition).
*   **Style**: We use `rustfmt` standard. Run `cargo fmt`.
*   **Linting**: We use `clippy`. Run `cargo clippy`.

## License

By contributing, you agree that your contributions will be licensed under its **AGPL v3** License.
