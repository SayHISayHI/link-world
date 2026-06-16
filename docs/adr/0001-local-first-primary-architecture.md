# 0001 - Local-first as the primary architecture

Status: Accepted  
Date: 2026-06-16

## Context

Link World stores personal information assets: saved links, parsed content, AI summaries, evaluations, private notes, prompts and future sync metadata. Users need privacy, offline access and control over model providers. A cloud-only architecture would simplify product development, but it would weaken trust, increase compliance burden and make the product dependent on third-party service availability.

## Decision

Link World will be Local-first. The local application must be able to save, parse, browse, search and manage already processed content without a cloud account. Cloud and hybrid features are optional enhancements.

## Consequences

- Local database, object store, search index and job runner are first-class architecture elements.
- Cloud sync cannot become the only source of truth.
- Feature design must include offline and degraded states.
- Sensitive and secret data can remain local.
- Some capabilities, such as multi-device sync and hosted workers, require extra design later.

## Alternatives Considered

- Cloud-first SaaS: simpler collaboration and sync, weaker trust and offline story.
- Browser-extension-only product: easier capture, weaker long-term local knowledge system.
- Obsidian-style file-only system: portable, but weaker for jobs, AI trace, evaluation and structured query.

## Revisit When

- Enterprise customers require centralized administration as the default.
- Local storage and migration complexity become the main blocker for product adoption.
- A secure E2EE cloud architecture becomes mature enough to offer equivalent trust.
