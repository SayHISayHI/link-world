# 0007 - Model provider runtime and protocol adapters

Status: Accepted  
Date: 2026-06-23

## Context

AI enrichment originally called an OpenAI-compatible `chat/completions` endpoint directly. Adding Anthropic, Gemini, OpenAI Responses or local Ollama would therefore duplicate authentication, payload, response, timeout and error handling inside business services. Adopting a hosted gateway as the only abstraction would also weaken Local-first and BYO API requirements.

OpenClaw demonstrates the useful architectural boundary: provider selection, model metadata and runtime execution are separate from agent/business logic. Node Tide needs the same boundary, but its stable contract must remain owned by the project so a third-party SDK does not become a domain API.

## Decision

- Node Tide owns capability-specific provider contracts and `ModelProviderRegistry`.
- `provider` identifies the configured supplier; `api_family` identifies the wire protocol. They are stored and versioned separately.
- The first `TextGenerationProvider` implementation wraps Rust `genai` for OpenAI Chat Completions, OpenAI Responses, Anthropic Messages, Google Generative AI and Ollama.
- Known OpenAI-compatible suppliers may select specialized `genai` adapters, while unknown compatible suppliers fall back to the OpenAI adapter with an explicit base URL.
- AI enrichment depends only on the registry contract. It does not construct provider URLs or vendor payloads.
- Credentials remain behind `SecretStore`; Windows uses Credential Manager and macOS uses Keychain. API keys are never returned by read commands or persisted in the normal configuration table.
- Multiple provider configs use stable ids. The selected default Chat config id is stored in `local_settings`; automatic cross-provider failover is explicitly out of scope.
- Provider configuration belongs to the formal Settings route. Object detail can trigger a run and show trace metadata, but cannot edit credentials.
- Embedding, rerank and vision will use separate capability contracts and registries when implemented.

## Consequences

- Adding a provider that uses an existing protocol is configuration work rather than a new business-service branch.
- Protocol-specific fixes, retries and error mapping are centralized and testable.
- The `genai` dependency is replaceable because its types do not cross the Node Tide runtime contract.
- New native protocols still require an adapter implementation and capability tests.
- `api_family` requires an additive database migration; old configurations default to OpenAI Chat Completions.
- Legacy single-provider rows remain readable. On first save they can be updated in place, while new configurations receive UUID ids and an explicit default selection.

## Alternatives Considered

### Keep a hand-written OpenAI-compatible client

Rejected because each native protocol would repeat transport and response normalization, and the existing service boundary was already vendor-specific.

### Expose `genai` directly to services

Rejected because SDK request/response types would become domain contracts and make replacement or capability-specific policy harder.

### Require a hosted model gateway

Rejected as the default because Local Edition must work with direct BYO credentials and local Ollama without a cloud dependency. A gateway can later be configured as another provider.

### Copy OpenClaw's provider implementation

Rejected as a direct port because its TypeScript/runtime constraints differ from Tauri/Rust. Its separation of model registry and execution informed this design.

## Revisit When

- Dynamic third-party model provider plugins are loaded out of process.
- Provider fallback, health scoring or circuit breakers require multi-provider routing.
- Embedding/rerank/vision capabilities expose material shared transport abstractions.
- `genai` no longer supports required protocols or security maintenance.
