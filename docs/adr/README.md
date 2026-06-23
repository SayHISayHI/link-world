# Architecture Decision Records

ADR 用于记录 Link World 的关键架构决策。任何会影响长期架构、数据模型、安全边界、技术栈或部署形态的决定，都应新增或更新 ADR。

## Format

每条 ADR 使用：

- Status: Proposed / Accepted / Deprecated / Superseded
- Context
- Decision
- Consequences
- Alternatives Considered
- Revisit When

## Index

- [0001 - Local-first as the primary architecture](./0001-local-first-primary-architecture.md)
- [0002 - Tauri and Rust for the desktop host](./0002-tauri-rust-desktop-host.md)
- [0003 - SQLite, FTS5 and optional sqlite-vec for local storage](./0003-sqlite-fts-sqlite-vec.md)
- [0004 - Plugin-first connectors, parsers and evaluators](./0004-plugin-first-content-runtime.md)
- [0005 - Traceable AI and privacy policy gates](./0005-traceable-ai-privacy-policy-gates.md)
- [0006 - Markdown AST rendering and advisory AI display hints](./0006-markdown-ast-rendering-and-ai-display-hints.md)
- [0007 - Model provider runtime and protocol adapters](./0007-model-provider-runtime-and-protocol-adapters.md)
