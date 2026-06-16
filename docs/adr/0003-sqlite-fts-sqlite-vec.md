# 0003 - SQLite, FTS5 and optional sqlite-vec for local storage

Status: Accepted  
Date: 2026-06-16

## Context

Local Edition needs structured metadata, full-text search, durable background jobs, AI traces and future sync metadata. The database must be embedded, reliable and easy to back up. Vector search is important but should not block the MVP.

## Decision

Use SQLite as the local relational store and FTS5 for required full-text search. Use sqlite-vec as an optional feature for semantic search when packaging and runtime stability are acceptable. Treat FTS and vector indexes as derived data. `parsed_documents` is the source of truth for parsed content.

## Consequences

- MVP can ship with reliable FTS even if sqlite-vec is delayed.
- Database schema must support reindexing and rebuildable derived indexes.
- Vector chunks reference metadata through rowid-linked metadata tables.
- Cloud Edition can later map relational data to Postgres while preserving domain contracts.

## Alternatives Considered

- Postgres local service: powerful, but operationally heavy for desktop users.
- File-based Markdown only: portable, but insufficient for job state, traces, evaluations and permissions.
- Dedicated vector DB: stronger vector features, but too heavy for local-first MVP.

## Revisit When

- sqlite-vec packaging becomes unreliable across target platforms.
- Object count or semantic search requirements exceed SQLite-based performance.
- Cloud Edition becomes dominant and Postgres-specific capabilities are required.
