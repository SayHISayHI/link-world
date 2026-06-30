# Link World 运营就绪与发布标准

状态: Draft
目标: 定义从 MVP 到商业化版本必须满足的工程运营标准。

## 1. Release Readiness

发布前必须检查：

- 数据库 migration 能从空库启动。
- 数据库 migration 能从上一版本升级。
- 应用可以在无网络环境启动并浏览已有内容。
- 未配置模型时，保存、解析、搜索仍可用。
- 配置模型后，AI enrich 可以成功并写入 trace。
- 删除对象后，FTS、vector、object store 和 AI 派生产物被清理。
- Windows 安装包可以安装、启动、卸载。

## 2. Environment Matrix

MVP 主支持：

- Windows 11（当前 Local Alpha 主目标；API Key 使用 Credential Manager）。
- Tauri v2。
- SQLite + FTS5。
- OpenAI Chat Completions/Responses、Anthropic Messages、Google Generative AI、OpenAI-compatible 和 Ollama Chat provider。

兼容目标：

- Windows 10。
- macOS。
- Linux。
- Ollama / LM Studio。
- sqlite-vec semantic search。

## 3. Observability

### 3.1 Local diagnostics

设置页必须能展示：

- 应用版本。
- 数据库路径。
- 对象存储路径。
- 数据库大小。
- 对象存储大小。
- 后台任务队列深度。
- 最近失败任务。
- 模型 provider 连接测试结果。
- 当前 provider、API protocol、base URL、model 与凭据是否已配置；不得显示 secret 内容。
- 插件列表和权限状态。

当前实现状态：Models 已提供正式 Settings route、多配置、默认项、连接测试和 credential 状态；Storage 已支持创建、列出、验证、两阶段恢复和自动 rollback，Sprint 2 readiness 自动化门禁已建立，真实 Windows 故障矩阵仍是发布门禁；Sprint 3 已建立 capture/job readiness 自动化和真实网络/进程故障矩阵，矩阵实测仍是完成门槛；Diagnostics 已提供本地健康快照、DB/object store/job/model 摘要、失败 job 打开对象与 capture retry、显式确认的脱敏 JSON 支持包及 size/SHA-256，并已接入 capture submit/fetch 与 AI enrichment 的有界结构化日志和持久化 correlation id；Sprint 5 readiness 自动化和 W5-01 至 W5-14 发布候选矩阵已建立，但其他关键流程日志/correlation 与真实矩阵仍未完成；Privacy、Capture、Plugins、About 仅提供明确的里程碑占位，不计为已交付。

### 3.2 Logs

日志必须结构化：

- timestamp
- level
- module
- event
- object_id
- job_id
- error_code
- message

当前实现边界：capture submit/fetch 与 AI enrichment submitted/started/succeeded/failed 已写入 `logs/link-world.jsonl`，单文件 2 MiB、保留一份轮转；写入与支持包读取均复验字段和敏感 marker。AI job payload、`analysis.requested/created/failed` domain event、IPC result 和日志共享同一 UUID，event payload 只含 analysis id 或稳定 `ai.*` code。restore、migration、search maintenance 等关键流程尚未接入，因此 Week 5 日志验收仍未完成。

日志不得包含：

- API key。
- token。
- cookie。
- session。
- 完整正文。
- secret / sensitive 内容。

## 4. Backup and Recovery

Local Edition 必须支持：

- 手动导出全库非 secret 对象为 Markdown/JSON 目录。（已实现；真实安装包回归待完成）
- 创建、列出和校验本地 restore point。（已实现）
- 普通应用启动检测 pending migration 后先创建并验证 restore point，以 phase guard 阻止不确定 migration 自动重试。（已实现；真实安装升级待回归）
- 恢复前完整校验、safety backup、私有候选迁移和重启应用。（已实现）
- 启动迁移失败时进入受限 recovery UI，展示 verified backup ID 并提供显式 restore/restart 操作。（已实现；真实安装升级待回归）
- 恢复失败或 phase 中断后自动 rollback，旧数据库重新可启动。（自动化已实现；真实 Windows 回归待完成）
- 对象存储路径可定位。
- 检测数据库损坏并给出用户可理解提示。

当前恢复策略：

- 在线备份使用 `VACUUM INTO`，候选迁移后执行 `quick_check`、`foreign_key_check` 和 WAL checkpoint。
- 在线进程只 prepare；重启后、pool 初始化前通过 phase marker 切换数据库与 objects。
- 候选初始化失败先关闭新 pool，再恢复 rollback payload 和旧数据库。
- JSON/Markdown 便携导出是独立能力，不复用含完整用户内容的 restore point；默认排除 secret、credential reference、内部 job 和本机对象存储路径。（已实现）
- 普通启动 migration 在 SQLx 写 schema 前创建完整 restore point；`guard.running.json` 遗留且仍有 pending migration 时 fail closed，并保留 verified backup ID。
- 启动 migration 失败时不会挂载普通 Library 或后台服务；应用进入 `StartupState::Recovery`，只开放备份列表、校验、显式 restore 准备和重启。

## 5. Migration Policy

迁移规则：

- 每个 migration 必须有版本号和描述。
- 破坏性 migration 必须先创建备份。
- restore candidate 迁移失败不得触碰在线数据；启动期迁移失败必须自动 rollback 或停止启动，禁止在不一致数据上继续运行。
- 不允许静默删除用户数据。

- 旧 AI analysis 和 evaluation result 不因 schema 升级被覆盖。

当前自动化已覆盖 0001/0002/0003 历史 schema、1000 对象 v1 数据集、外键/FTS/隐私/AI/Evaluation/job/provider/tombstone 不变量、未知未来 migration 的 fail-closed、启动 recovery UI 的受限操作展示，以及便携导出默认排除 secret 与本机 storage URI。真实安装包原地升级和进程级强制终止仍未完成。

## 6. Incident Playbooks

### 6.1 AI provider failure

处理：

- 将 job 标记为 `failed` 或 `blocked`。
- 保留对象在 `parsed` 状态。
- 提示用户检查 key、base URL、余额或模型名称。
- 不重试鉴权失败。

### 6.2 Parser regression

处理：

- 标记 parser version。
- 新 parser 发布后允许用户手动 reparse。
- 旧 `parsed_documents` 保留，不覆盖。

### 6.3 Database migration failure

处理：

- 停止后续写入。
- 展示错误和备份路径。
- 提供恢复上一版本数据库的说明。

### 6.4 Plugin misbehavior

处理：

- 自动禁用连续失败插件。
- 记录 audit log。
- 展示插件权限和最近错误。
- 提供撤销权限和卸载入口。

## 7. Performance Regression Gates

发布前采样验证：

- Sprint 2 数据安全自动化门禁：`npm run readiness:sprint2`，保存 JSON report。
- Sprint 2 Windows 安装包故障矩阵：按 `docs/sprint2_windows_fault_matrix.md` 执行 W2-01 至 W2-13。
- Sprint 3 采集可靠性自动化门禁：`npm run readiness:sprint3`，保存 JSON report。
- Sprint 3 真实网络/进程矩阵：按 `docs/sprint3_capture_fault_matrix.md` 执行 W3-01 至 W3-16。
- Sprint 5 本地可观测性自动化门禁：`npm run readiness:sprint5`，保存 JSON report。
- Sprint 5 真实 Windows/支持矩阵：按 `docs/sprint5_observability_readiness.md` 执行 W5-01 至 W5-14。
- 1000 个对象列表和搜索。
- 250 个对象搜索基准 smoke：`cargo test repositories::search::tests::search_benchmark_fixture_supports_repeatable_corpus`。
- 5000 个对象搜索基准：`cargo test repositories::search::tests::search_benchmark_5k_objects_reports_budget -- --ignored --nocapture`，最大单次查询预算 <= 250ms。
- 20000 个对象搜索基准：`cargo test repositories::search::tests::search_benchmark_20k_objects_reports_budget -- --ignored --nocapture`，最大单次查询预算 <= 500ms。
- 2026-06-26 手动验收结果：5k benchmark 最大单次查询 82ms，20k benchmark 最大单次查询 64ms，均低于预算。
- 5000 个对象列表滚动。
- 100 个 failed jobs 的诊断页。
- 50MB 对象存储下启动。
- 10 个并发 URL capture jobs。

任何明显卡顿必须进入发布阻断项。

## 8. Supportability

用户反馈问题时，应用应能生成本地诊断包。

诊断包包含：

- 应用版本。
- OS 版本。
- schema version。
- 最近错误码或 sanitized failed job summary。
- 任务状态摘要。
- 插件列表。
- 已脱敏日志。

诊断包不得包含：

- API key。
- token。
- cookie。
- session。
- 正文。
- source snapshots。
- embeddings。
- URL query/fragment。
- credential reference。
