---
name: init-extension-scaffold
description: Type-specific scaffolding requirements for Browser Extension projects.
---

# Init Browser Extension Scaffold

## Objective
Define scaffolding requirements for a new browser extension project. Use the project name and description from the Project Details section in the instruction. Follow Required Tech Stack or Required Stack By Layer if specified.

## Your Tasks

1. **Analyze the repository structure** (if existing) or create a new project scaffold
2. **Set up the extension development environment**:
   - Create `manifest.json` (Manifest V3 preferred)
   - Configure build tools (Webpack, Vite, or Rollup)
   - Set up TypeScript configuration
3. **Create essential project files**:
   - `README.md` with extension overview and installation instructions
   - `.gitignore` for extension projects
   - Environment configuration for different browsers
4. **Configure manifest.json**:
   - Define permissions (minimal required)
   - Set up content scripts
   - Configure background service worker
   - Define popup and options pages
5. **Set up extension components**:
   - Background service worker
   - Content scripts (if needed)
   - Popup UI
   - Options page (if needed)
   - Side panel (if applicable)
6. **Set up code quality tools**:
   - ESLint with browser extension rules
   - Prettier configuration
   - Type checking for WebExtension APIs
7. **Create initial project structure**:
   - `src/background/` - Service worker
   - `src/content/` - Content scripts
   - `src/popup/` - Popup UI
   - `src/options/` - Options page
   - `public/` - Static assets (icons, etc.)
8. **Configure multi-browser support**:
   - Chrome extension manifest
   - Firefox compatibility (if needed)
   - Safari compatibility (if needed)

## Tech Stack Recommendations

For new projects, consider:
- **Manifest**: V3 (Chrome) with V2 fallback for Firefox if needed
- **Framework**: React, Vue, or vanilla TypeScript
- **Build Tool**: Vite with CRXJS or Webpack
- **Storage**: chrome.storage.sync/local
- **Messaging**: chrome.runtime.sendMessage

## Extension-Specific Considerations

- Follow principle of least privilege for permissions
- Handle different browser contexts (background, content, popup)
- Implement proper message passing between contexts
- Consider performance impact on web pages (content scripts)
- Plan for extension updates and migration
- Test in multiple browsers
- Prepare for web store submission requirements

## Security Best Practices

- Avoid using `eval()` or inline scripts
- Sanitize any user input
- Use Content Security Policy
- Minimize host permissions
- Handle cross-origin requests properly

## Output

After completing initialization:
1. List all created/modified files
2. Provide loading instructions for each browser (unpacked extension)
3. Document permission requirements and justifications
4. Highlight decisions made and rationale
