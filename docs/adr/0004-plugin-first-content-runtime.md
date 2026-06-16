# 0004 - Plugin-first connectors, parsers and evaluators

Status: Accepted  
Date: 2026-06-16

## Context

The product depends on many changing input sources: web pages, GitHub repos, prompts, PDFs, social posts, browser extension captures and future APIs. The core differentiator is evaluation, and evaluator logic will vary by object type. Hard-coding platform-specific logic in core services would make the system brittle.

## Decision

Use plugin-first architecture for connectors, parsers, evaluators, model providers, sync providers and exporters. MVP plugins can be compiled into Rust as in-process trait implementations, but must follow the same contracts and permission model expected from future external plugins.

## Consequences

- Core services dispatch to registries instead of platform-specific code.
- Plugins must declare capabilities, version and permissions.
- Parser output includes parser id/version.
- Evaluation output includes evaluator type/version, evidence and artifacts.
- Security policy gates all plugin access to sensitive data and external capabilities.

## Alternatives Considered

- Hard-coded source handlers: faster initially, but high long-term maintenance cost.
- Full external plugin runtime from day one: flexible, but too much complexity for MVP.
- Prompt-only evaluators: simple, but fails the product goal of verifiable information value.

## Revisit When

- In-process plugins become insufficient for third-party ecosystem needs.
- Plugin failures threaten main process stability.
- WASM or process-isolated plugin runtime becomes necessary.
