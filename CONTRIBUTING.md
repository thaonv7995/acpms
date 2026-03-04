# Contributing to ACPMS

Thank you for your interest in contributing to **ACPMS** (Agentic Coding Project Management System). All contributions—bug reports, feature suggestions, and pull requests—are welcome.

## Getting Started

1. **Clone and set up your environment** — Follow [README – Development](README.md#development).
2. **Run locally** — Ensure `make setup && make infra-up && make migrate && make dev` runs successfully.
3. **Run tests** — `make test` (Rust + frontend).

## Reporting Bugs & Feature Requests

- **Bugs:** Describe how to reproduce, your environment (OS, versions), and any logs or error messages.
- **Features:** Describe the use case, why it’s needed, and (if any) suggested implementation.

Use [GitHub Issues](https://github.com/thaonv7995/acpms/issues) for both.

## Pull Request Process

1. **Fork** the repo and create a **branch** from `main` (e.g. `fix/xyz`, `feat/abc`).
2. Make your **changes** in that branch; keep the scope small and easy to review.
3. **Run** `make test` and ensure nothing is broken.
4. **Commit** with a clear message (e.g. `fix(server): handle empty body in POST /api/...`).
5. **Push** and open a **Pull Request** against `main`:
   - Describe what changed and why.
   - Reference any related issue (e.g. `Fixes #123`).
6. Address review feedback if requested.

## Code Conventions

- **Rust (crates/):** Use `cargo fmt` and `cargo clippy`; add tests for non-trivial logic.
- **Frontend (frontend/):** TypeScript/React; keep `npm run lint` clean; keep components small and reusable.
- **API/Database:** See [docs/api-integration-documentation.md](docs/api-integration-documentation.md) when changing API or schema.

## Repository Structure

- `crates/` — Rust workspace (db, server, services, executors, gitlab, deployment, preview, utils).
- `frontend/` — React + TypeScript + Vite.
- `docs/` — API docs, UI flows, SRS.
- `install.sh`, `Makefile` — Setup and build.

Details: [README – Project Structure](README.md#project-structure).

## License

By submitting a PR, you agree that your contribution is licensed under **Apache License 2.0**, same as the project (see [LICENSE](LICENSE)).
