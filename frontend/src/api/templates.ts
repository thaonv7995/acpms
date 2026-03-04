/**
 * Templates API Client
 * Manual API layer for template endpoints (until Orval regeneration)
 */

import { apiGet, API_PREFIX } from './client';

// ProjectType enum matching backend
export type ProjectType = 'web' | 'mobile' | 'desktop' | 'extension' | 'api' | 'microservice';

// Tech stack options per project type
export interface TechStack {
  name: string;
  value: string;
}

// Project template model
export interface ProjectTemplate {
  id: string;
  name: string;
  description: string | null;
  project_type: ProjectType;
  repository_url: string;
  tech_stack: TechStack[];
  default_settings: Record<string, unknown>;
  is_official: boolean;
  created_by: string | null;
  created_at: string;
}

// Template list response
export interface TemplateListResponse {
  templates: ProjectTemplate[];
  total: number;
}

/**
 * List all project templates, optionally filtered by project type
 */
export async function listTemplates(projectType?: ProjectType): Promise<ProjectTemplate[]> {
  const params = projectType ? `?project_type=${projectType}` : '';
  return apiGet<ProjectTemplate[]>(`${API_PREFIX}/templates${params}`);
}

/**
 * Get a specific template by ID
 */
export async function getTemplate(id: string): Promise<ProjectTemplate> {
  return apiGet<ProjectTemplate>(`${API_PREFIX}/templates/${id}`);
}

// Project type metadata for UI display
export interface ProjectTypeInfo {
  type: ProjectType;
  label: string;
  description: string;
  icon: string;
  color: string;
  defaultTechStacks: TechStack[];
  defaultBuildCommand: string;
  supportsPreview: boolean;
}

// Project type configurations
export const PROJECT_TYPE_INFO: Record<ProjectType, ProjectTypeInfo> = {
  web: {
    type: 'web',
    label: 'Web Application',
    description: 'Full-stack web apps with React, Vue, Next.js, etc.',
    icon: 'language',
    color: 'blue',
    defaultTechStacks: [
      { name: 'Next.js', value: 'nextjs' },
      { name: 'React + Vite', value: 'react-vite' },
      { name: 'Remix', value: 'remix' },
      { name: 'Nuxt 3', value: 'nuxt3' },
      { name: 'Vue.js', value: 'vuejs' },
      { name: 'SvelteKit', value: 'sveltekit' },
      { name: 'Angular', value: 'angular' },
      { name: 'React Router', value: 'react-router' },
      { name: 'Preact + Vite', value: 'preact-vite' },
      { name: 'SolidJS + Vite', value: 'solidjs-vite' },
      { name: 'Astro', value: 'astro' },
      { name: 'Qwik', value: 'qwik' },
      { name: 'SolidStart', value: 'solidstart' },
      { name: 'Laravel + Inertia', value: 'laravel-inertia' },
      { name: 'Django', value: 'django' },
      { name: 'Ruby on Rails', value: 'rails' },
      { name: 'Blazor', value: 'blazor' },
      { name: 'Gatsby', value: 'gatsby' },
      { name: 'Phoenix LiveView', value: 'phoenix-liveview' },
      { name: 'ASP.NET MVC', value: 'aspnet-mvc' },
      { name: 'Headless WordPress', value: 'wordpress-headless' },
    ],
    defaultBuildCommand: 'npm run build',
    supportsPreview: true,
  },
  mobile: {
    type: 'mobile',
    label: 'Mobile App',
    description: 'iOS/Android apps with React Native, Flutter, etc.',
    icon: 'smartphone',
    color: 'purple',
    defaultTechStacks: [
      { name: 'React Native', value: 'react-native' },
      { name: 'Expo', value: 'expo' },
      { name: 'Flutter', value: 'flutter' },
      { name: 'Ionic + Capacitor', value: 'ionic-capacitor' },
      { name: 'Swift/SwiftUI', value: 'swift' },
      { name: 'Kotlin', value: 'kotlin' },
      { name: 'Kotlin Multiplatform', value: 'kotlin-multiplatform' },
      { name: '.NET MAUI', value: 'dotnet-maui' },
      { name: 'NativeScript', value: 'nativescript' },
      { name: 'Cordova', value: 'cordova' },
      { name: 'Jetpack Compose', value: 'jetpack-compose' },
      { name: 'Compose Multiplatform', value: 'compose-multiplatform' },
      { name: 'Swift + UIKit', value: 'swift-uikit' },
      { name: 'Objective-C (iOS)', value: 'objective-c' },
      { name: 'Xamarin', value: 'xamarin' },
      { name: 'Unity Mobile', value: 'unity-mobile' },
    ],
    defaultBuildCommand: 'npm run build:ios && npm run build:android',
    supportsPreview: true,
  },
  desktop: {
    type: 'desktop',
    label: 'Desktop App',
    description: 'Cross-platform desktop apps with Electron, Tauri, etc.',
    icon: 'desktop_windows',
    color: 'emerald',
    defaultTechStacks: [
      { name: 'Tauri', value: 'tauri' },
      { name: 'Electron', value: 'electron' },
      { name: 'Wails', value: 'wails' },
      { name: 'Neutralino', value: 'neutralino' },
      { name: 'Flutter Desktop', value: 'flutter-desktop' },
      { name: '.NET MAUI', value: 'dotnet-maui' },
      { name: 'WPF (.NET)', value: 'wpf' },
      { name: 'WinUI 3', value: 'winui3' },
      { name: 'Avalonia', value: 'avalonia' },
      { name: 'NW.js', value: 'nwjs' },
      { name: 'Qt', value: 'qt' },
      { name: 'JavaFX', value: 'javafx' },
      { name: 'GTK', value: 'gtk' },
      { name: 'Tkinter (Python)', value: 'tkinter' },
      { name: 'PyQt / PySide', value: 'pyqt-pyside' },
      { name: 'Chromium Embedded (CEF)', value: 'cef' },
    ],
    defaultBuildCommand: 'npm run package',
    supportsPreview: true,
  },
  extension: {
    type: 'extension',
    label: 'Browser Extension',
    description: 'Chrome, Firefox, Edge browser extensions',
    icon: 'extension',
    color: 'orange',
    defaultTechStacks: [
      { name: 'Chrome MV3', value: 'chrome-mv3' },
      { name: 'Firefox WebExtension', value: 'firefox' },
      { name: 'Edge MV3', value: 'edge-mv3' },
      { name: 'Safari Web Extension', value: 'safari-web-extension' },
      { name: 'Plasmo', value: 'plasmo' },
      { name: 'WXT', value: 'wxt' },
      { name: 'Vanilla WebExtension', value: 'webextension-vanilla' },
      { name: 'TypeScript WebExtension', value: 'webextension-typescript' },
      { name: 'React Extension', value: 'react-extension' },
      { name: 'Vue Extension', value: 'vue-extension' },
      { name: 'Svelte Extension', value: 'svelte-extension' },
      { name: 'Angular Extension', value: 'angular-extension' },
      { name: 'Cross-browser MV3', value: 'cross-browser-mv3' },
      { name: 'Brave MV3 Extension', value: 'brave-mv3' },
    ],
    defaultBuildCommand: 'npm run build:ext',
    supportsPreview: true,
  },
  api: {
    type: 'api',
    label: 'API Service',
    description: 'REST/GraphQL APIs with FastAPI, Express, NestJS, etc.',
    icon: 'api',
    color: 'cyan',
    defaultTechStacks: [
      { name: 'FastAPI (Python)', value: 'fastapi' },
      { name: 'Express.js', value: 'express' },
      { name: 'NestJS', value: 'nestjs' },
      { name: 'tRPC', value: 'trpc' },
      { name: 'Koa.js', value: 'koa' },
      { name: 'AdonisJS', value: 'adonisjs' },
      { name: 'Hono', value: 'hono' },
      { name: 'Spring Boot', value: 'spring-boot' },
      { name: 'Micronaut', value: 'micronaut' },
      { name: 'Quarkus', value: 'quarkus' },
      { name: 'Django REST', value: 'django-rest' },
      { name: 'Flask', value: 'flask' },
      { name: 'Ruby on Rails API', value: 'rails-api' },
      { name: 'ASP.NET Core', value: 'aspnet-core' },
      { name: 'Axum (Rust)', value: 'axum' },
      { name: 'Actix Web (Rust)', value: 'actix' },
      { name: 'Gin (Go)', value: 'gin' },
      { name: 'Fiber (Go)', value: 'fiber' },
      { name: 'Echo (Go)', value: 'echo-go' },
      { name: 'Chi (Go)', value: 'chi-go' },
      { name: 'Laravel API', value: 'laravel-api' },
      { name: 'Apollo GraphQL', value: 'apollo-graphql' },
      { name: 'gRPC + gRPC Gateway', value: 'grpc-gateway' },
      { name: 'Phoenix API (Elixir)', value: 'phoenix-api' },
    ],
    defaultBuildCommand: 'cargo build --release',
    supportsPreview: true,
  },
  microservice: {
    type: 'microservice',
    label: 'Microservice',
    description: 'Containerized microservices with Docker, gRPC, etc.',
    icon: 'hub',
    color: 'rose',
    defaultTechStacks: [
      { name: 'Go + gRPC', value: 'go-grpc' },
      { name: 'Rust + Tonic', value: 'rust-tonic' },
      { name: 'Node.js + gRPC', value: 'node-grpc' },
      { name: 'Python + gRPC', value: 'python-grpc' },
      { name: 'Spring Cloud', value: 'spring-cloud' },
      { name: '.NET Microservices', value: 'dotnet-microservices' },
      { name: 'Kafka Event-Driven', value: 'kafka-event-driven' },
      { name: 'NATS JetStream', value: 'nats-jetstream' },
      { name: 'Dapr', value: 'dapr' },
      { name: 'Serverless Microservices', value: 'serverless-microservices' },
      { name: 'NestJS Microservices', value: 'nestjs-microservices' },
      { name: 'Go Kit', value: 'go-kit' },
      { name: 'Go Kratos', value: 'go-kratos' },
      { name: 'Temporal Workflows', value: 'temporal' },
      { name: 'RabbitMQ Workers', value: 'rabbitmq-workers' },
      { name: 'Redis Streams', value: 'redis-streams' },
      { name: 'CQRS + Event Sourcing', value: 'cqrs-event-sourcing' },
      { name: 'Istio Service Mesh', value: 'istio-service-mesh' },
      { name: 'Linkerd Service Mesh', value: 'linkerd' },
    ],
    defaultBuildCommand: 'docker build -t service .',
    supportsPreview: true,
  },
};

/**
 * Get project type info by type
 */
export function getProjectTypeInfo(type: ProjectType): ProjectTypeInfo {
  return PROJECT_TYPE_INFO[type];
}

/**
 * Get all project types as array
 */
export function getAllProjectTypes(): ProjectTypeInfo[] {
  return Object.values(PROJECT_TYPE_INFO);
}
