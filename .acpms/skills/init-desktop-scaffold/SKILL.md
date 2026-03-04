---
name: init-desktop-scaffold
description: Type-specific scaffolding requirements for Desktop Application projects.
---

# Init Desktop Application Scaffold

## Objective
Define scaffolding requirements for a new desktop application project. Use the project name and description from the Project Details section in the instruction. Follow Required Tech Stack or Required Stack By Layer if specified.

## Your Tasks

1. **Analyze the repository structure** (if existing) or create a new project scaffold
2. **Set up the desktop development environment**:
   - Initialize Electron or Tauri project
   - Configure build system for multi-platform support
   - Set up TypeScript/Rust configuration
3. **Create essential project files**:
   - `README.md` with project overview and build instructions
   - `.gitignore` for desktop projects (build outputs, platform-specific)
   - Environment configuration
4. **Configure desktop framework**:
   - **Electron**: Main process, renderer process, preload scripts
   - **Tauri**: Rust backend, frontend configuration, security settings
5. **Set up native integrations**:
   - System tray support
   - Native menus
   - File system access
   - IPC (Inter-Process Communication)
6. **Set up code quality tools**:
   - ESLint/Clippy configuration
   - Prettier/rustfmt configuration
   - Security auditing
7. **Create initial project structure**:
   - Main process entry point
   - Renderer/frontend application
   - Shared types and utilities
8. **Configure packaging and distribution**:
   - Code signing setup (placeholder)
   - Auto-update configuration
   - Platform-specific installers

## Tech Stack Recommendations

For new projects, consider:
- **Tauri** (Rust + Web frontend): Smaller bundle, better security, native performance
- **Electron** (Node.js): Larger ecosystem, easier web developer transition
- **Frontend**: React, Vue, or Svelte with TypeScript
- **Build Tools**: electron-builder, tauri-cli

## Desktop-Specific Considerations

- Handle multiple windows and window management
- Implement proper security (context isolation, CSP)
- Consider offline functionality
- Handle native OS integrations (notifications, file associations)
- Plan for auto-updates and version management
- Test on all target platforms (Windows, macOS, Linux)

## Output

After completing initialization:
1. List all created/modified files
2. Provide build instructions for each platform
3. Document IPC patterns and security considerations
4. Highlight decisions made and rationale
