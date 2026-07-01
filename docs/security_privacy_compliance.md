# Link World 安全、隐私与合规设计

状态: Draft  
适用范围: Local Edition、Cloud Edition、Hybrid Edition

## 1. Security Goals

Link World 处理的是用户长期积累的信息资产，其中可能包含私密网页、内部链接、prompt、代码片段、聊天内容、API key 和账号相关页面。安全目标是：

- 默认本地优先，用户不需要把数据交给云端。
- 用户清楚知道哪些内容被保存、分析、同步和导出。
- 第三方 AI 调用必须可授权、可追踪、可撤销。
- 插件默认无权限，权限必须声明、展示和审计。
- 删除必须覆盖原文、快照、索引、向量、AI 派生产物和评估产物。

## 2. Threat Model

### 2.1 Assets

- 用户保存的原始内容和快照。
- 解析后的正文。
- AI summary、evaluation 和 trace。
- 向量 embeddings。
- API key、OAuth token、本地插件 secret。
- 插件包和插件配置。
- 本地数据库和对象存储。
- 云同步数据包。

### 2.2 Actors

- 正常用户。
- 恶意或失控插件。
- 第三方平台。
- 第三方模型供应商。
- 本机恶意软件。
- 云端服务管理员。
- 网络攻击者。

### 2.3 Key risks

| Risk | Impact | Required mitigation |
| --- | --- | --- |
| API key 明文泄漏 | 资金损失、账号风险 | OS keychain / encrypted secret store |
| 插件越权读取 sensitive 内容 | 隐私泄漏 | deny-by-default permission model |
| 第三方 AI 收到 secret 内容 | 高危泄漏 | policy gate + explicit authorization |
| 删除不彻底 | 合规和信任风险 | tombstone + purge job + audit |
| 云端代登录第三方平台 | 账号封禁和合规风险 | 默认禁止 |
| 向量泄漏语义信息 | 内容反推风险 | 跟随 privacy/sync policy |
| 日志记录正文或 token | 本地或云端泄漏 | structured redaction |
| Search maintenance 记录 query/index content/raw SQLite error | 搜索内容或本机结构泄漏 | job-id correlation + stable `search.*` code |
| Markdown 原始 HTML 或危险 URL 被执行 | 脚本执行、跟踪或数据泄漏 | skip raw HTML + protocol allowlist + sanitize |
| AI 展示提示越权修改内容或安全策略 | 内容失真或绕过安全边界 | advisory-only schema + parsed document binding + deterministic fallback |

## 3. Privacy Levels

| Level | 内容示例 | 默认 AI 策略 | 默认同步策略 |
| --- | --- | --- | --- |
| `public` | 公开文章、公开 repo | 可调用用户配置模型 | 可同步 |
| `personal` | 收藏、批注、阅读记录 | 可调用，但需显示 trace | 可同步 |
| `sensitive` | 私有 repo、工作资料、聊天内容 | 第三方 AI 需显式授权 | 默认不同步正文 |
| `secret` | API key、cookie、session、密码 | 禁止发送第三方 AI | 禁止同步 |

## 4. Credential Handling

### 4.1 Storage rules

- API key 不进入普通 SQLite 表。
- OAuth token 只允许官方 connector 使用。
- 非官方 cookie/session 只能存在本地高级插件环境，默认禁用。
- `.log`、crash report、export 文件和前端 store 禁止出现凭据。
- `model_provider_configs.secret_ref` 只保存 secret 引用，不保存 secret 值。

### 4.2 Rotation and revocation

- 用户必须能删除或替换模型 API key。
- 删除 key 后，等待中的 AI job 必须失败为 `blocked` 或要求重新授权。
- 插件禁用后，其 secret 访问立即失效。

## 5. Plugin Permission Model

插件权限采用最小权限原则。

权限类型：

- `network`
- `filesystem`
- `browser_automation`
- `read_public_objects`
- `read_personal_objects`
- `read_sensitive_objects`
- `call_third_party_ai`

权限要求：

- 插件 manifest 必须声明权限。
- 用户启用插件时必须看到权限说明。
- sensitive 权限必须单独确认。
- 插件权限变更写入 `audit_logs`。
- 插件运行时必须携带 `PluginContext`，不能直接访问数据库连接或 secret store。

## 6. AI Safety and Traceability

每次 AI 调用必须记录：

- provider
- model
- capability
- prompt template id/version
- input hash
- output hash
- token usage
- cost estimate
- latency
- privacy decision

AI 输出展示规则：

- 必须区分原文事实、模型推断和 evaluator 结论。
- 低置信度结论必须标注。
- 高风险建议必须要求用户确认。
- 模型输出 JSON 解析失败时，不得静默降级为“成功”。
- evaluator verdict/score 属于推断，不得与 evidence 混排为原文事实；evidence 必须携带 `original_content`、`internal_library`、`external_check`、`sandbox_run` 或 `user_feedback` 来源标签。
- 无 evidence 时只允许 `unknown` verdict 且必须给出 limitation；当前 `local_deterministic` evaluator 不访问第三方，未来 `model_assisted` evaluator 必须经过与 AI enrichment 相同或更严格的 privacy policy gate。
- 可选 `displayHints` 只允许选择版本化的文档级展示模式；无效提示不影响主体分析，并回退到 Markdown AST 推断。
- 只有绑定当前 parsed document 且置信度至少为 `0.75` 的提示可以应用；提示不得改变正文、Markdown、AST 或渲染安全策略。

文档渲染规则：

- Markdown 是稳定的展示存储格式，阅读 AST 只在前端临时派生，不持久化。
- 原始 HTML 必须跳过，链接和图片 URL 使用协议 allowlist。
- 远程图片必须启用 lazy loading、async decoding 和 `no-referrer`。
- Callout 等渲染扩展使用项目内固定插件管线，不开放运行时第三方插件执行。

## 7. Deletion and Data Portability

删除流程：

1. 创建 `deletion_tombstones`。
2. 隐藏对象。
3. 清理 FTS。
4. 清理 vector chunks。
5. 清理 source snapshots 和 object store。
6. 清理 AI analysis、AI trace、evaluation artifacts。
7. 写入 audit log。
8. 标记 purge completed。

导出要求：

- 用户可以导出 Markdown + JSON metadata。
- 导出必须保留来源、采集时间、AI trace 摘要和 evaluation verdict。
- secret 内容默认不导出，除非用户明确选择。

## 8. Compliance Boundaries

平台采集策略：

- 默认支持用户主动提交、浏览器当前页保存、官方 API、文件导入。
- 不做云端代登录。
- 不托管第三方平台账号密码、cookie 和非官方 session。
- 本地高级自动化 connector 必须明确风险、默认关闭、可审计。

AI 数据策略：

- 不默认使用用户内容训练模型。
- 不把用户内容作为公开数据集。
- 云端遥测默认不包含正文。
- 用户应能关闭产品分析遥测。

## 9. Security Acceptance Criteria

- sensitive 对象调用第三方 AI 前必须触发 policy gate。
- secret 对象无法被第三方 AI job 读取。
- 禁用插件后，该插件不能继续访问对象或 secret。
- 删除对象后，搜索和向量检索无法命中该对象。
- crash log 中不包含 API key、token、cookie、正文。
- 支持包必须显式确认、固定写入本机应用目录且不自动上传；导出 JSON 不含正文、raw job/audit/plugin/domain-event payload、URL query/fragment、credential reference、embedding 或本机绝对路径；只允许 payload-free correlation summary。AI enrichment event payload 在数据库内也只允许 internal analysis id 或稳定 `ai.*` code，不得复制 prompt、output、provider 配置或 raw error。runtime logs 只允许导出通过 schema/redaction 复验的当前日志条目，读取失败不得回退为 raw text。`npm run readiness:sprint5` 提供确定性隐私门禁，发布候选仍须完成 `sprint5_observability_readiness.md` 的合成诱饵、真实轮转和支持交接矩阵。
- 模型调用都能在 UI 中追踪到 provider、model 和时间。
- 恶意 Markdown、危险 URL 和无效 AI display hints 的安全回归测试通过。
