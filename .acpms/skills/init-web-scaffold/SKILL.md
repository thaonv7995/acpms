---
name: init-web-scaffold
description: Type-specific scaffolding requirements for Web Application projects.
---

# Init Web Application Scaffold

## Objective
Define scaffolding requirements for a new web application project. Use the project name and description from the Project Details section in the instruction. Follow Required Tech Stack or Required Stack By Layer if specified.

## Your Tasks

1. **Analyze the repository structure** (if existing) or create a new project scaffold
2. **Set up the development environment**:
   - Create/update `package.json` with appropriate dependencies
   - Configure build tools (Vite, Next.js, or similar)
   - Set up TypeScript configuration if applicable
3. **Create essential project files**:
   - `README.md` with project overview and setup instructions
   - `.gitignore` for web projects
   - Environment configuration (`.env.example`)
4. **Set up code quality tools**:
   - ESLint configuration
   - Prettier configuration
   - Pre-commit hooks (optional)
5. **Create initial project structure**:
   - `src/` directory with entry point
   - `public/` directory for static assets
   - Basic routing setup if framework supports it

## Tech Stack Recommendations

For new projects, consider:
- **Framework**: Next.js 14+ (App Router), Vite + React, or SvelteKit
- **Language**: TypeScript
- **Styling**: Tailwind CSS or CSS Modules
- **State Management**: React Query for server state, Zustand for client state
- **Testing**: Vitest + Testing Library

## Output

After completing initialization:
1. List all created/modified files
2. Provide setup instructions
3. Highlight any decisions made and rationale
