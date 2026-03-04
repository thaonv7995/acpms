---
name: init-import-analyze
description: Analyze an imported GitLab repository to understand structure, services, and current state for architecture mapping.
---

# Init Import Analyze

## Objective
After cloning a repository from GitLab, analyze the project to understand its structure, identify services/components, evaluate current state, and produce an architecture map for ACPMS.

## When This Applies
- GitLab import init task (existing repository cloned locally)
- Run after `git clone` completes, before bootstrap/PRD generation

## Workflow

1. **Explore Directory Structure**
   - List root directory and key subdirectories (src/, app/, packages/, services/, etc.)
   - Identify monorepo vs single-package layout
   - Note configuration files: package.json, Cargo.toml, go.mod, requirements.txt, docker-compose, etc.

2. **Identify Services and Components**
   - Frontend: React/Vue/Svelte/Angular apps, static sites, Next.js/Nuxt
   - Backend/API: Express/FastAPI/NestJS/Axum/Actix, serverless handlers
   - Database: Prisma, migrations, ORM configs
   - Workers/Queue: Bull/Celery/background jobs
   - Auth: NextAuth/Clerk/Auth0/JWT config
   - Storage: S3/MinIO/upload handlers
   - Microservices: Docker/K8s, gRPC, internal APIs

3. **Evaluate Current State**
   - Tech stack (languages, frameworks, databases)
   - Build system (Vite/Webpack/Cargo/npm scripts)
   - Test/lint setup
   - Deployment config (Dockerfile, CI, cloud configs)
   - Document any notable gaps or technical debt

4. **Produce Output**
   - Write `.acpms/import-analysis.json` with the required schema (see Output Contract)

## Output Contract

Create `.acpms/import-analysis.json` in the project root with this structure:

```json
{
  "architecture": {
    "nodes": [
      { "id": "browser", "label": "Browser Client", "type": "client", "status": "healthy" },
      { "id": "frontend", "label": "Web Frontend", "type": "frontend", "status": "healthy" },
      { "id": "api", "label": "Application API", "type": "api", "status": "healthy" },
      { "id": "database", "label": "Primary Database", "type": "database", "status": "healthy" }
    ],
    "edges": [
      { "source": "browser", "target": "frontend", "label": "HTTPS" },
      { "source": "frontend", "target": "api", "label": "REST/GraphQL" },
      { "source": "api", "target": "database", "label": "Read/Write" }
    ]
  },
  "assessment": {
    "project_type": "web",
    "summary": "Next.js full-stack app with PostgreSQL",
    "services": ["frontend", "api", "database"],
    "tech_stack": ["next", "react", "postgresql", "prisma"]
  }
}
```

### Node Types
- `client`, `frontend`, `api`, `database`, `cache`, `queue`, `storage`, `auth`, `gateway`, `service`, `mobile`, `worker`

### Edge Labels
- `HTTPS`, `REST/GraphQL`, `Read/Write`, `Cache`, `Jobs`, `OIDC/OAuth`, `Token Verify`, `IPC`, etc.

## Decision Rules
| Situation | Action |
|-----------|--------|
| Simple frontend-only app | Minimal nodes: browser, frontend (maybe database if direct) |
| Full-stack with API | Include api, database, auth if present |
| Monorepo with multiple packages | Map each deployable service as a node |
| Unclear structure | Infer from package.json scripts, Dockerfile, entry points |
| No .acpms/ directory | Create it before writing the file |

## Guardrails
- Do NOT modify any source code. Read-only analysis.
- Do NOT run install/build unless necessary to infer structure (prefer static analysis).
- Output must be valid JSON. Validate before writing.
- If analysis fails, still write a minimal valid JSON with empty nodes/edges and assessment.summary explaining the failure.
