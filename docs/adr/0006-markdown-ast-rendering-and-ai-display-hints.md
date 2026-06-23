# 0006 - Markdown AST rendering and advisory AI display hints

Status: Accepted  
Date: 2026-06-23

## Context

Link World must render articles captured either by pasted URL or by the browser extension with consistent structure and safe behavior. Plain-text rendering loses headings, lists, tables and code semantics. Persisting a renderer-specific AST would couple storage to frontend libraries, while allowing the extension or AI to rewrite presentation would create inconsistent capture paths and weaken security guarantees.

## Decision

- Rust parsers persist both `text_content` and `markdown_content`. Text remains the input for search and AI; Markdown is the stable reading format.
- URL HTML and sanitized browser DOM use the same Rust document parser. The browser extension does not generate Markdown or implement site-specific layout rules.
- The frontend derives an ephemeral MDAST from Markdown with the unified/remark ecosystem. AST data is memoized for rendering but is not stored in SQLite or global application state.
- Rendering uses a fixed compile-time pipeline: GFM, sanitization, stable heading slugs and the trusted Link World Callout transform. Raw HTML and runtime-installed rendering plugins are not supported.
- A deterministic AST summary selects one of `article`, `tutorial`, `reference` or `code-heavy`, so readable presentation never depends on AI availability.
- AI analysis schema version 2 may store a nullable, versioned `display_hints_json` sidecar. A hint is applied only when it is valid, belongs to the current parsed document and has confidence of at least `0.75`.
- AI display hints are advisory. They cannot edit Markdown or AST, choose arbitrary components, change URL handling, relax sanitization or alter image privacy attributes. Missing, malformed, low-confidence and stale hints are ignored without failing the main AI analysis.
- Existing analysis rows are not backfilled. They continue to render using deterministic AST inference until AI analysis is run again.

## Consequences

- Captures have one server-side parsing behavior regardless of whether they originate from a pasted URL or the extension.
- Markdown remains portable and independent from a specific React renderer; AST library upgrades do not require data migrations.
- The Markdown reader and its dependencies stay in a lazy chunk, while AST analysis and component policies require dedicated frontend tests.
- Migration `0002_ai_display_hints.sql` adds one nullable column without rewriting existing rows.
- Security tests must cover raw HTML, dangerous protocols, remote image attributes and attempts to use AI hints to bypass rendering policy.
- AI prompt/schema changes must preserve tolerant parsing because display hints are optional enrichment, not the primary analysis result.

## Alternatives Considered

- Persist renderer AST: rejected because it couples storage and migrations to frontend implementation details.
- Render only plain text: rejected because it loses document semantics and produces poor reading quality.
- Generate Markdown in the browser extension: rejected because capture paths would diverge and extension complexity would grow.
- Let AI rewrite Markdown or annotate individual blocks: rejected because output becomes nondeterministic and can cross content and security boundaries.
- Provide a general runtime rendering plugin market: rejected for the first version because the execution and compatibility surface is disproportionate to the required features.

## Revisit When

- Block-level annotations or collaborative comments require stable persisted block identities.
- Server and client must exchange a shared AST for non-React renderers.
- Syntax highlighting or additional document formats justify a larger rendering runtime and bundle budget.
- A runtime plugin model has a concrete use case and an enforceable sandbox and compatibility contract.
