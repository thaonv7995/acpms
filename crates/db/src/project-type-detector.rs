//! Project Type Detector Service
//!
//! Automatically detects project type based on file structure and configuration files.
//! Used during GitLab import to classify projects.

use crate::models::ProjectType;
use serde_json::Value;

/// Service for detecting project type from repository files
pub struct ProjectTypeDetector;

impl ProjectTypeDetector {
    /// Detect project type from a list of file paths in the repository
    ///
    /// Priority-based detection:
    /// 1. Browser extension (manifest.json + background.js/service_worker)
    /// 2. Mobile (React Native, Flutter, Expo)
    /// 3. Desktop (Electron, Tauri)
    /// 4. Microservice (Dockerfile + go.mod/Cargo.toml)
    /// 5. API (backend-specific configs without frontend)
    /// 6. Web (default for frontend projects)
    pub fn detect_from_files(files: &[String]) -> ProjectType {
        let file_set: std::collections::HashSet<&str> = files.iter().map(|s| s.as_str()).collect();

        // Check for browser extension (highest priority for specific type)
        if Self::is_browser_extension(&file_set) {
            return ProjectType::Extension;
        }

        // Check for mobile app
        if Self::is_mobile_app(&file_set) {
            return ProjectType::Mobile;
        }

        // Check for desktop app
        if Self::is_desktop_app(&file_set) {
            return ProjectType::Desktop;
        }

        // Check for microservice (containerized backend)
        if Self::is_microservice(&file_set) {
            return ProjectType::Microservice;
        }

        // Check for API (backend without frontend)
        if Self::is_api(&file_set) {
            return ProjectType::Api;
        }

        // Default to web application
        ProjectType::Web
    }

    /// Detect project type from package.json content
    pub fn detect_from_package_json(pkg: &Value) -> ProjectType {
        let dependencies = pkg.get("dependencies").and_then(|d| d.as_object());
        let dev_dependencies = pkg.get("devDependencies").and_then(|d| d.as_object());

        // Helper to check if a dependency exists
        let has_dep = |name: &str| -> bool {
            dependencies.is_some_and(|deps| deps.contains_key(name))
                || dev_dependencies.is_some_and(|deps| deps.contains_key(name))
        };

        // Check for React Native / Expo (mobile)
        if has_dep("react-native") || has_dep("expo") {
            return ProjectType::Mobile;
        }

        // Check for Electron / Tauri (desktop)
        if has_dep("electron") || has_dep("electron-builder") {
            return ProjectType::Desktop;
        }

        // Check for browser extension frameworks
        if has_dep("webextension-polyfill") || has_dep("@anthropic-ai/claude-code") {
            // Also check scripts for extension-specific commands
            if let Some(scripts) = pkg.get("scripts").and_then(|s| s.as_object()) {
                let scripts_str = serde_json::to_string(scripts).unwrap_or_default();
                if scripts_str.contains("extension") || scripts_str.contains("manifest") {
                    return ProjectType::Extension;
                }
            }
        }

        // Check for backend frameworks (API)
        if has_dep("express")
            || has_dep("fastify")
            || has_dep("@nestjs/core")
            || has_dep("koa")
            || has_dep("hapi")
        {
            // If no frontend framework, it's an API
            if !has_dep("react")
                && !has_dep("vue")
                && !has_dep("svelte")
                && !has_dep("@angular/core")
            {
                return ProjectType::Api;
            }
        }

        // Default to web for anything with frontend frameworks or no specific indicators
        ProjectType::Web
    }

    /// Detect project type from Cargo.toml content
    pub fn detect_from_cargo_toml(cargo: &Value) -> ProjectType {
        // Check for web framework dependencies
        let dependencies = cargo.get("dependencies").and_then(|d| d.as_object());

        let has_dep =
            |name: &str| -> bool { dependencies.is_some_and(|deps| deps.contains_key(name)) };

        // Check for web frameworks (Actix, Axum, Rocket, Warp)
        if has_dep("actix-web")
            || has_dep("axum")
            || has_dep("rocket")
            || has_dep("warp")
            || has_dep("tide")
        {
            // Check for Tauri (desktop)
            if has_dep("tauri") {
                return ProjectType::Desktop;
            }
            return ProjectType::Api;
        }

        // Check for gRPC (microservice indicator)
        if has_dep("tonic") || has_dep("prost") {
            return ProjectType::Microservice;
        }

        // Default to API for Rust projects
        ProjectType::Api
    }

    /// Detect project type from go.mod content (Go projects)
    pub fn detect_from_go_mod(go_mod_content: &str) -> ProjectType {
        // Check for web frameworks
        if go_mod_content.contains("github.com/gin-gonic/gin")
            || go_mod_content.contains("github.com/labstack/echo")
            || go_mod_content.contains("github.com/gorilla/mux")
            || go_mod_content.contains("github.com/gofiber/fiber")
        {
            return ProjectType::Api;
        }

        // Check for gRPC
        if go_mod_content.contains("google.golang.org/grpc") {
            return ProjectType::Microservice;
        }

        // Default to microservice for Go (commonly used for microservices)
        ProjectType::Microservice
    }

    // ===== Private Helper Methods =====

    fn is_browser_extension(files: &std::collections::HashSet<&str>) -> bool {
        // Must have manifest.json
        let has_manifest = files.iter().any(|f| f.ends_with("manifest.json"));
        if !has_manifest {
            return false;
        }

        // Should have background script or service worker
        let has_background = files.iter().any(|f| {
            f.contains("background")
                || f.contains("service_worker")
                || f.contains("content_script")
                || f.ends_with("popup.html")
                || f.ends_with("popup.js")
                || f.ends_with("popup.tsx")
        });

        has_background
    }

    fn is_mobile_app(files: &std::collections::HashSet<&str>) -> bool {
        // React Native indicators
        let has_react_native = files.contains("metro.config.js")
            || files.contains("react-native.config.js")
            || files.iter().any(|f| f.contains("android/app"))
            || files
                .iter()
                .any(|f| f.contains("ios/") && f.ends_with(".xcodeproj"));

        // Flutter indicators
        let has_flutter =
            files.contains("pubspec.yaml") && files.iter().any(|f| f.contains("lib/main.dart"));

        // Expo indicators
        let has_expo = files.contains("app.json")
            && files
                .iter()
                .any(|f| f.contains("expo") || f.contains("App.tsx"));

        has_react_native || has_flutter || has_expo
    }

    fn is_desktop_app(files: &std::collections::HashSet<&str>) -> bool {
        // Electron indicators
        let has_electron = files.contains("electron.js")
            || files.contains("electron/main.js")
            || files.contains("main.electron.js")
            || files.iter().any(|f| f.contains("electron-builder"));

        // Tauri indicators
        let has_tauri =
            files.contains("tauri.conf.json") || files.iter().any(|f| f.contains("src-tauri/"));

        has_electron || has_tauri
    }

    fn is_microservice(files: &std::collections::HashSet<&str>) -> bool {
        let has_dockerfile = files.contains("Dockerfile") || files.contains("dockerfile");
        let has_docker_compose =
            files.contains("docker-compose.yml") || files.contains("docker-compose.yaml");

        // Must have Dockerfile
        if !has_dockerfile {
            return false;
        }

        // Should have Go or Rust (common microservice languages)
        let has_go = files.contains("go.mod");
        let has_rust = files.contains("Cargo.toml");

        // Or should have docker-compose (indicating multi-service setup)
        (has_go || has_rust) || has_docker_compose
    }

    fn is_api(files: &std::collections::HashSet<&str>) -> bool {
        // Python API indicators
        let has_python_api = files.contains("requirements.txt")
            && (files.iter().any(|f| f.contains("fastapi"))
                || files.iter().any(|f| f.contains("flask"))
                || files.iter().any(|f| f.contains("django")));

        // Node.js API indicators (without frontend)
        let has_node_api = files.contains("package.json")
            && !files.iter().any(|f| {
                f.contains("src/components")
                    || f.contains("src/pages")
                    || f.contains("src/app")
                    || f.ends_with(".tsx")
                    || f.ends_with(".vue")
                    || f.ends_with(".svelte")
            });

        // Rust API indicators
        let has_rust_api = files.contains("Cargo.toml") && !files.contains("tauri.conf.json");

        // Go API indicators
        let has_go_api = files.contains("go.mod") && !files.contains("Dockerfile");

        has_python_api || has_node_api || has_rust_api || has_go_api
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_web_project() {
        let files = vec![
            "package.json".to_string(),
            "src/App.tsx".to_string(),
            "src/components/Header.tsx".to_string(),
            "vite.config.ts".to_string(),
        ];
        assert_eq!(
            ProjectTypeDetector::detect_from_files(&files),
            ProjectType::Web
        );
    }

    #[test]
    fn test_detect_extension() {
        let files = vec![
            "manifest.json".to_string(),
            "background.js".to_string(),
            "popup.html".to_string(),
            "content_script.js".to_string(),
        ];
        assert_eq!(
            ProjectTypeDetector::detect_from_files(&files),
            ProjectType::Extension
        );
    }

    #[test]
    fn test_detect_mobile_react_native() {
        let files = vec![
            "package.json".to_string(),
            "metro.config.js".to_string(),
            "android/app/build.gradle".to_string(),
            "ios/MyApp.xcodeproj".to_string(),
        ];
        assert_eq!(
            ProjectTypeDetector::detect_from_files(&files),
            ProjectType::Mobile
        );
    }

    #[test]
    fn test_detect_desktop_electron() {
        let files = vec![
            "package.json".to_string(),
            "electron.js".to_string(),
            "src/main.ts".to_string(),
        ];
        assert_eq!(
            ProjectTypeDetector::detect_from_files(&files),
            ProjectType::Desktop
        );
    }

    #[test]
    fn test_detect_microservice() {
        let files = vec![
            "Dockerfile".to_string(),
            "go.mod".to_string(),
            "main.go".to_string(),
            "docker-compose.yml".to_string(),
        ];
        assert_eq!(
            ProjectTypeDetector::detect_from_files(&files),
            ProjectType::Microservice
        );
    }

    #[test]
    fn test_detect_from_package_json_react_native() {
        let pkg = serde_json::json!({
            "dependencies": {
                "react": "^18.0.0",
                "react-native": "^0.72.0"
            }
        });
        assert_eq!(
            ProjectTypeDetector::detect_from_package_json(&pkg),
            ProjectType::Mobile
        );
    }

    #[test]
    fn test_detect_from_package_json_electron() {
        let pkg = serde_json::json!({
            "dependencies": {
                "react": "^18.0.0"
            },
            "devDependencies": {
                "electron": "^25.0.0",
                "electron-builder": "^24.0.0"
            }
        });
        assert_eq!(
            ProjectTypeDetector::detect_from_package_json(&pkg),
            ProjectType::Desktop
        );
    }

    #[test]
    fn test_detect_from_package_json_express_api() {
        let pkg = serde_json::json!({
            "dependencies": {
                "express": "^4.18.0",
                "cors": "^2.8.0"
            }
        });
        assert_eq!(
            ProjectTypeDetector::detect_from_package_json(&pkg),
            ProjectType::Api
        );
    }
}
