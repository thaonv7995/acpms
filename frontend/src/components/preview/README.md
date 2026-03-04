# Preview Components - Phase 5.5

Live preview components for dev server integration (frontend-only with mock API).

## Components

### DevServerControls
Start/stop/restart controls with status indicators.
```tsx
<DevServerControls status="running" url="http://localhost:3000" onStart={...} />
```

### PreviewPanel
Iframe container with controls and fullscreen support.
```tsx
<PreviewPanel devServerUrl={url} status={status} onStart={...} />
```

## Hook

### useDevServer(taskId, attemptId?)
Manages dev server state with mock responses.
```tsx
const { status, url, startServer, stopServer } = useDevServer(taskId);
```

## Mock API
`src/api/dev-server-mock.ts` - Replace with real backend when ready.

## Backend Integration
Replace mocks with:
- `POST /api/v1/tasks/{id}/preview/start`
- `POST /api/v1/tasks/{id}/preview/stop`
- `GET /api/v1/tasks/{id}/preview/status`
