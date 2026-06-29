# Link World 第三方调用韧性规范

状态: Draft  
适用范围: Model providers、GitHub API、URL fetch、browser capture bridge、future cloud APIs

## 1. Purpose

Link World 依赖外部网络和第三方 API，但产品必须在外部服务不可用时保持本地核心能力。本文档定义 timeout、retry、rate limit、fallback、错误分类和用户反馈规范。

## 2. External Dependency Classes

| Class | Examples | Criticality | Default behavior when down |
| --- | --- | --- | --- |
| Model provider | OpenAI, Anthropic, Gemini, OpenAI-compatible, Ollama HTTP | optional for core library | keep parsed state, mark AI job failed/blocked |
| GitHub API | repo metadata | optional enrichment | fallback to public HTML/README if available |
| URL fetch | user submitted URL | needed for URL capture | mark object failed, suggest browser capture |
| Browser bridge | extension/deep link | capture path | keep app usable, suggest manual Add URL |
| Cloud sync | future sync API | optional | queue local changes |

## 3. Timeout Policy

| Operation | Timeout |
| --- | --- |
| URL fetch connect | 5s |
| URL fetch total | 20s |
| Model chat | 60s |
| Model embedding batch | 60s |
| GitHub metadata | 15s |
| Browser capture endpoint | 5s |
| Cloud sync push/pull | 30s |

Timeout must produce typed error, not generic unknown failure.

## 4. Retry Policy

Retryable:

- network timeout.
- connection reset.
- HTTP 429 with retry-after.
- HTTP 503/504.
- provider temporary unavailable.

Not retryable automatically:

- HTTP 401/403 auth failure.
- policy denied.
- invalid API key.
- invalid model name.
- unsupported content type.
- parser unsupported.

Backoff:

- exponential backoff with jitter.
- max 3 attempts by default.
- user-triggered retry allowed after terminal UI feedback.
- 内置文本生成 adapter 统一执行该策略，service 不得按供应商各自重试。

## 5. Circuit Breaker

For providers:

- open circuit after repeated failures.
- stop scheduling new calls briefly.
- keep local save/search available.
- expose status in diagnostics.

Suggested states:

- `closed`: normal.
- `open`: temporarily blocked.
- `half_open`: allow one probe.

## 6. Rate Limit Handling

On rate limit:

- parse retry-after header if available.
- mark job retry time.
- show provider-specific but redacted message.
- do not hammer provider.
- aggregate repeated failures in diagnostics.

连接测试使用同一 adapter、timeout、错误映射和凭据解析路径，但不得写入 AI analysis/trace，也不得自动保存候选配置。

AI enrichment job failures persist stable `ai.*` reasons after provider adapter retry handling. The prefix is a product protocol value; user-facing components must format it into a title/action and must not expose raw provider messages. Current categories: `ai.timeout`, `ai.model_auth`, `ai.rate_limit`, `ai.model_not_found`, `ai.output_schema`, `ai.not_configured`, `ai.provider_config_invalid`, `ai.policy_denied`, `ai.provider_unavailable`, `ai.secret_storage`, `ai.input_unavailable`, `ai.local_failure`, `ai.failed`.

URL capture failures follow the same boundary with stable `capture.*` reasons. Classification may retain an HTTP status code, but must discard raw response bodies, cookies, tokens and unrecognized policy/internal error details before writing `knowledge_objects`, `background_jobs`, domain events or frontend completion events.

## 7. Fallback Strategy

Model provider:

- fallback to another configured provider if capability and policy allow.
- if no provider, keep object `parsed`.
- do not fabricate AI output.

GitHub:

- prefer official API if configured.
- fallback to public README URL.
- if both fail, keep URL object and mark evaluation blocked.

URL fetch:

- if HTML indicates JavaScript/login wall, mark failed with browser capture suggestion.
- do not bypass access controls.

Embedding:

- fallback to FTS search.
- do not block normal library.

## 8. Error Mapping

| Failure | IpcErrorCode |
| --- | --- |
| timeout | `ERR_NETWORK_TIMEOUT` |
| provider auth | `ERR_MODEL_AUTH` |
| rate limit | `ERR_MODEL_RATE_LIMIT` |
| model or provider endpoint not found | `ERR_MODEL_NOT_FOUND` |
| invalid model JSON | `ERR_MODEL_OUTPUT_SCHEMA` |
| policy denial | `ERR_POLICY_DENIED` |
| unsupported parser | `ERR_PARSE_FAILED` |
| plugin permission | `ERR_PLUGIN_PERMISSION` |

## 9. Idempotency

External calls triggered by jobs must be idempotent from the product perspective.

Rules:

- Re-running AI job should create new analysis version or detect duplicate input hash.
- Re-running parse should create new `parsed_documents` only when content hash differs.
- Re-running evaluation should create new `evaluation_runs`.
- Re-running manual URL capture should not duplicate the same normalized canonical URL unless user explicitly saves again; the default response returns the existing object with `deduplicated=true` and does not spawn a new fetch job.

## 10. Observability

Record:

- provider id.
- operation.
- latency.
- status code class.
- retry count.
- final error code.
- redacted failure reason.

Never record:

- API key.
- full request body containing user content.
- raw prompts with sensitive text.
- response bodies containing private content.

## 11. User Feedback

User-facing failure messages must include:

- what failed.
- whether data was saved locally.
- next action.
- whether retry is possible.

Examples:

- “网页已保存，但正文解析失败。这个网站可能需要 JavaScript 渲染，请尝试使用浏览器扩展保存当前页面。”
- “AI 分析未运行。模型鉴权失败，请检查设置中的 API Key。”
- “语义搜索暂不可用，已自动使用全文搜索。”

## 12. Testing Requirements

Tests must cover:

- timeout.
- 429 retry-after.
- 401 auth failure.
- invalid JSON model output.
- provider fallback.
- FTS fallback when embedding unavailable.
- URL blocked page detection.
- no secret in logs.
