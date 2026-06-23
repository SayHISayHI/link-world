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

Before `TextGenerationProvider.generate` or any future embedding/rerank/vision provider call:

1. Resolve object privacy level.
2. Resolve collection/workspace policy.
3. Check provider destination.
4. Check explicit authorization if needed.
5. Create AI trace shell or policy denial audit.
6. Redact logs.

Provider connection tests do not include object content and do not create AI analyses, but they still use `SecretStore`, redacted errors and the configured destination. Windows Alpha 使用 Credential Manager（service `com.linkworld.app.model-provider`）；SQLite 只保存 `keyring:model-provider:<config-id>` 或显式环境变量引用。Read APIs return only `hasApiKey`, never the key or `secret_ref`. 删除 provider 配置时先删除 credential，再删除数据库引用；任一步失败都不得伪装为成功。
Provider base URL 不允许携带 userinfo、query 或 fragment，避免把凭据混入 URL、错误或诊断日志。默认 provider 的 config id 存在 `local_settings`；删除默认项只清除选择，不自动把内容路由到其他第三方 provider。

本地 restore point 是完整用户数据副本，不是脱敏导出：它包含对象正文和 sensitive / secret 内容，依赖 OS 用户目录权限，当前不宣称加密。Credential value 不进入备份；便携导出必须另行脱敏并默认排除 secret。备份命令不得接受调用方提供的任意路径。

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

扩展提交的 DOM 必须先做采集侧清洗，再由桌面端 Rust parser 统一解析。扩展不得生成 Markdown、注入站点专用展示规则或把第三方页面脚本带入阅读器。

## 7. Document Rendering and AI Display Policy

- Markdown 阅读器必须跳过原始 HTML，禁止启用 `rehype-raw` 或等效的任意 HTML 执行路径。
- 链接只允许 `http`、`https`、`mailto`；图片只允许 `http`、`https`，并强制 lazy loading、async decoding 和 `no-referrer`。
- Callout 等项目扩展只允许编译期固定插件，禁止运行时安装或执行不受信任的渲染插件。
- AI display hints 只建议文档级展示模式，不得修改 Markdown、AST、URL、图片属性、组件映射或清洗策略。
- 仅应用绑定当前 parsed document 且置信度至少为 `0.75` 的合法提示；缺失、未知 mode、越界 confidence 和过期提示必须忽略。
- display hints 解析失败不得让主体 AI analysis 失败，必须回退到确定性的 AST 展示模式。

## 8. Diagnostics Policy

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

## 9. Deletion Policy

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

## 10. Security Review Checklist

Before shipping a feature:

- Does it read sensitive or secret data?
- Does it call third-party AI?
- Does it add plugin permissions?
- Does it write logs?
- Does it export data?
- Does it sync data?
- Does delete/purge cover its derived data?
- Does UI disclose provider/model/action to the user?
