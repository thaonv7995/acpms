---
name: init-microservice-scaffold
description: Type-specific scaffolding requirements for Microservice projects.
---

# Init Microservice Scaffold

## Objective
Define scaffolding requirements for a new microservice project. Use the project name and description from the Project Details section in the instruction. Follow Required Tech Stack or Required Stack By Layer if specified.

## Your Tasks

1. **Analyze the repository structure** (if existing) or create a new project scaffold
2. **Set up the microservice development environment**:
   - Initialize project (go.mod for Go, Cargo.toml for Rust)
   - Configure the service framework
   - Set up dependency injection if applicable
3. **Create essential project files**:
   - `README.md` with service overview, setup, and deployment instructions
   - `.gitignore` appropriate for the language
   - Environment configuration (`.env.example`)
4. **Configure containerization**:
   - `Dockerfile` (multi-stage build, minimal base image)
   - `docker-compose.yml` for local development
   - `.dockerignore` for efficient builds
5. **Set up service structure**:
   - Entry point with graceful shutdown
   - Configuration management
   - Logging setup (structured JSON logs)
   - Health check endpoints (`/health`, `/ready`, `/live`)
   - Metrics endpoint (`/metrics`)
6. **Configure communication**:
   - REST API endpoints (if applicable)
   - gRPC service definitions (if applicable)
   - Message queue integration (if applicable)
7. **Set up observability**:
   - Structured logging
   - Prometheus metrics
   - Distributed tracing setup (OpenTelemetry)
8. **Set up code quality tools**:
   - Linting (golangci-lint, clippy)
   - Formatting (gofmt, rustfmt)
   - Testing setup
9. **Create initial service structure**:
   - `cmd/` or `src/main.rs` - Entry point
   - `internal/` or `src/` - Business logic
   - `api/` - API definitions (proto files, OpenAPI)
   - `configs/` - Configuration files

## Tech Stack Recommendations

For new projects, consider:
- **Go**: Standard library + Chi/Gin, gRPC, sqlx
- **Rust**: Axum/Actix, tonic for gRPC, sqlx
- **Observability**: Prometheus, Jaeger/Zipkin, structured logging
- **Container**: Alpine-based or distroless images

## Microservice Best Practices

- Follow 12-factor app principles
- Implement proper graceful shutdown
- Use environment variables for configuration
- Design for horizontal scaling
- Implement circuit breakers for external calls
- Use connection pooling for databases
- Plan for service discovery and load balancing
- Implement idempotency for critical operations

## Container Optimization

- Use multi-stage builds
- Minimize image size (Alpine, distroless, scratch)
- Run as non-root user
- Properly handle signals (SIGTERM, SIGINT)
- Set resource limits in container spec

## Output

After completing initialization:
1. List all created/modified files
2. Provide Docker build and run instructions
3. Document API endpoints and contracts
4. Include observability setup instructions
5. Highlight decisions made and rationale
