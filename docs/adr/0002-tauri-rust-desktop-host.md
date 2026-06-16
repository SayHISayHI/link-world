# 0002 - Tauri and Rust for the desktop host

Status: Accepted  
Date: 2026-06-16

## Context

The product needs local filesystem access, SQLite, background jobs, secure credential storage, plugin boundaries and a polished desktop UI. Electron would provide a large ecosystem but higher memory footprint. A pure web app would not satisfy Local-first requirements.

## Decision

Use Tauri v2 as the desktop host, Rust for backend/local runtime, and React + Vite + TypeScript for the UI.

## Consequences

- Backend logic lives in Rust and is exposed through typed Tauri commands.
- Frontend must not access SQLite, filesystem or secrets directly.
- Rust services own database access, object store, job runner, model provider adapters and plugin runtime.
- Tauri permission configuration must follow least privilege.
- Some browser/web patterns, such as SSR, are intentionally avoided.

## Alternatives Considered

- Electron: broad ecosystem, but heavier and easier to accidentally mix frontend/backend responsibilities.
- Native Windows app: strong platform integration, but weaker cross-platform path.
- Web-only PWA: easier deployment, but not enough local system access for this product.

## Revisit When

- Tauri v2 ecosystem blocks key requirements such as secure storage, updater or extension integration.
- Cross-platform packaging becomes substantially more expensive than expected.
- The product pivots away from desktop local-first usage.
