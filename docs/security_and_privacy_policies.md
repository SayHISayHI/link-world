# Link World 安全与隐私策略红线

状态: Draft  
关系: 本文档是 `security_privacy_compliance.md` 的执行策略版，偏向实现红线和检查项。

## 1. Non-Negotiable Rules

- API key、OAuth token、cookie、session 不得进入普通 SQLite 表。
- 日志、crash report、诊断包不得包含正文、secret、token、cookie、session、embedding。
- `secret` 内容禁止发送第三方 AI。
- `sensitive` 内容发送第三方 AI 前必须获得对象或 collection 级显式授权。
- 插件默认无权限，必须由 manifest 声明并由用户授权。
- 云端不托管非官方第三方平台账号密码、cookie 或 session。
- 删除对象必须清理派生索引、AI 输出、评估产物和对象存储。

## 2. Data Classification

| Class | Examples | Storage | AI policy | Sync policy |
| --- | --- | --- | --- | --- |
| `public` | public article/repo | local/cloud allowed | allowed | allowed |
| `personal` | saved links, notes | local default | allowed with trace | user controlled |
| `sensitive` | private repo, work data | local default | explicit authorization | metadata only by default |
| `secret` | key, cookie, session | secret store only | denied | denied |

## 3. Redaction Policy

Redact before logging:

- `Authorization` headers.
- `api_key`, `access_token`, `refresh_token`, `cookie`, `session`.
- full document text.
- raw prompt containing user content.
- embedding vectors.
- local absolute paths if diagnostics package is exported.

Allowed in logs:

- object id.
- job id.
- provider id.
- model name.
- content hash.
- error code.
- short redacted reason.

## 4. AI Policy Gates

Before `ModelProvider.chat` or `ModelProvider.embed`:

1. Resolve object privacy level.
2. Resolve collection/workspace policy.
3. Check provider destination.
4. Check explicit authorization if needed.
5. Create AI trace shell or policy denial audit.
6. Redact logs.

Decision table:

| Privacy | Local model | Third-party model |
| --- | --- | --- |
| public | allow | allow |
| personal | allow | allow with trace |
| sensitive | allow | explicit authorization |
| secret | deny unless special local-only future flow | deny |

## 5. Plugin Policy Gates

Plugin access must check:

- plugin enabled.
- permission declared.
- permission granted.
- object privacy level.
- requested capability.
- runtime location: local/cloud.

Denied access returns `ERR_PLUGIN_PERMISSION` and writes audit log.

## 6. Browser Capture Policy

Browser extension may submit:

- current URL.
- title.
- selected text.
- visible DOM fragment.
- screenshot if user explicitly chooses.

Browser extension must not:

- batch scrape background tabs.
- read third-party cookies.
- upload platform session.
- bypass paywalls or login walls.
- silently capture without user action.

## 7. Diagnostics Policy

Diagnostics package may contain:

- app version.
- OS version.
- schema version.
- feature flags.
- plugin manifests.
- failed job summaries.
- redacted logs.

Diagnostics package must not contain:

- source snapshots.
- parsed document content.
- raw AI prompts.
- API keys.
- tokens/cookies/sessions.
- embeddings.

## 8. Deletion Policy

Deletion must enqueue purge and remove:

- `knowledge_objects` visibility.
- `source_snapshots`.
- `parsed_documents`.
- `ai_analysis`.
- `ai_traces`.
- `evaluation_runs`.
- `evaluation_artifacts`.
- `knowledge_fts`.
- `vector_chunks_meta`.
- `vec_chunks`.
- object store files.
- caches.

Failures must be visible in diagnostics and retryable.

## 9. Security Review Checklist

Before shipping a feature:

- Does it read sensitive or secret data?
- Does it call third-party AI?
- Does it add plugin permissions?
- Does it write logs?
- Does it export data?
- Does it sync data?
- Does delete/purge cover its derived data?
- Does UI disclose provider/model/action to the user?
