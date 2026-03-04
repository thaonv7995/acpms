---
name: init-api-scaffold
description: Type-specific scaffolding requirements for API Service projects.
---

# Init API Service Scaffold

## Objective
Define scaffolding requirements for a new API service project. Use the project name and description from the Project Details section in the instruction. Follow Required Tech Stack or Required Stack By Layer if specified.

## Your Tasks

1. **Analyze the repository structure** (if existing) or create a new project scaffold
2. **Set up the API development environment**:
   - Initialize project (Cargo.toml for Rust, package.json for Node, requirements.txt for Python)
   - Configure the web framework (Axum, Express, FastAPI, etc.)
   - Set up database connectivity if needed
3. **Create essential project files**:
   - `README.md` with API overview, setup, and usage instructions
   - `.gitignore` appropriate for the language/framework
   - Environment configuration (`.env.example`)
   - Docker configuration for local development
4. **Set up API structure**:
   - Route/endpoint organization
   - Middleware setup (auth, logging, CORS, rate limiting)
   - Error handling patterns
   - Request/response validation
5. **Configure database (if applicable)**:
   - Migration system setup
   - Connection pooling
   - Initial schema design
6. **Set up code quality tools**:
   - Linting configuration
   - Formatting configuration
   - Type checking
7. **Create initial API structure**:
   - Health check endpoint (`/health`)
   - API versioning (`/api/v1/`)
   - Basic CRUD endpoint template

## Tech Stack Recommendations

For new projects, consider:
- **Rust**: Axum + SQLx + tokio
- **Node.js**: Express/Fastify + TypeScript + Prisma
- **Python**: FastAPI + SQLAlchemy + Pydantic
- **Go**: Gin/Echo + GORM

## API Best Practices

- Use RESTful conventions or GraphQL schema design
- Implement proper HTTP status codes
- Add request validation and sanitization
- Include OpenAPI/Swagger documentation
- Plan for authentication (JWT, OAuth2)
- Consider rate limiting and caching strategies

## Output

After completing initialization:
1. List all created/modified files
2. Provide API endpoint documentation
3. Include example requests/responses
4. Document database setup if applicable
5. Highlight decisions made and rationale
