---
name: init-mobile-scaffold
description: Type-specific scaffolding requirements for Mobile Application projects.
---

# Init Mobile Application Scaffold

## Objective
Define scaffolding requirements for a new mobile application project. Use the project name and description from the Project Details section in the instruction. Follow Required Tech Stack or Required Stack By Layer if specified.

## Your Tasks

1. **Analyze the repository structure** (if existing) or create a new project scaffold
2. **Set up the mobile development environment**:
   - Initialize React Native / Expo / Flutter project
   - Configure platform-specific settings (iOS/Android)
   - Set up TypeScript/Dart configuration
3. **Create essential project files**:
   - `README.md` with project overview, setup, and run instructions
   - `.gitignore` for mobile projects (node_modules, build artifacts, platform-specific)
   - Environment configuration for different environments (dev, staging, prod)
4. **Configure native platforms**:
   - iOS: Update `Info.plist`, configure Xcode project settings
   - Android: Update `AndroidManifest.xml`, configure Gradle settings
5. **Set up code quality tools**:
   - ESLint/Dart analyzer configuration
   - Prettier configuration
   - Type checking configuration
6. **Create initial project structure**:
   - `src/` or `lib/` directory with entry point
   - Navigation setup (React Navigation / GoRouter)
   - Basic screen templates

## Tech Stack Recommendations

For new projects, consider:
- **Framework**: React Native with Expo (managed workflow) or Flutter
- **Language**: TypeScript (RN) or Dart (Flutter)
- **Navigation**: React Navigation 6+ or GoRouter
- **State Management**: Zustand/Redux Toolkit (RN) or Riverpod/Bloc (Flutter)
- **Testing**: Jest + React Native Testing Library or Flutter Test

## Platform Considerations

- Handle both iOS and Android platform differences
- Consider safe area insets and notch handling
- Plan for offline-first architecture
- Consider app signing and deployment pipelines

## Output

After completing initialization:
1. List all created/modified files
2. Provide platform-specific setup instructions (iOS Simulator, Android Emulator)
3. Document any native module requirements
4. Highlight decisions made and rationale
