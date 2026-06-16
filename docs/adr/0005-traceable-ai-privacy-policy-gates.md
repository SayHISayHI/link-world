# 0005 - Traceable AI and privacy policy gates

Status: Accepted  
Date: 2026-06-16

## Context

Link World uses AI to summarize, classify, evaluate and retrieve personal information assets. AI output can be wrong, costly, privacy-sensitive or vendor-dependent. Users must be able to trust what happened to their data and understand how conclusions were generated.

## Decision

Every AI call that affects stored product state must create trace metadata. Sensitive and secret content must pass policy gates before third-party AI calls. AI outputs must distinguish original facts, model inference and evaluation conclusions.

## Consequences

- `ai_traces` records provider, model, capability, prompt template, hashes, token usage, cost and latency.
- `AIAnalysis` and `EvaluationRun` are versioned and append-only by default.
- Secret content is never sent to third-party AI.
- Sensitive content requires explicit authorization.
- UI must surface model/provider/time information where relevant.
- Deletion must purge AI-derived artifacts and indexes.

## Alternatives Considered

- Store only AI output: simpler, but not auditable.
- Allow provider-specific direct calls in services: faster initially, but prevents consistent policy and cost tracking.
- Treat AI conclusions as plain notes: weakens trust and evaluation quality.

## Revisit When

- Local-only model usage becomes dominant and trace overhead is too high.
- Enterprise customers require stricter audit exports.
- Regulations or platform policies impose new AI data handling requirements.
