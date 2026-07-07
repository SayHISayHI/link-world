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

当前实现状态：Models 已提供正式 Settings route、多配置、默认项、连接测试和 credential 状态；Storage 已支持创建、列出、验证、两阶段恢复和自动 rollback，Sprint 2 readiness 自动化门禁已建立，真实 Windows 故障矩阵仍是发布门禁；Sprint 3 已建立 capture/job readiness 自动化和真实网络/进程故障矩阵，矩阵实测仍是完成门槛；Diagnostics 已提供本地健康快照、DB/object store/job/model 摘要、失败 job 打开对象与 capture retry、显式确认的脱敏 JSON 支持包及 size/SHA-256，并已接入 capture submit/fetch、AI enrichment、search maintenance、startup migration 与 restore 的有界结构化日志和持久化 correlation id；计划内关键流程的代码级覆盖已完成，Sprint 5 readiness 自动化和 W5-01 至 W5-14 发布候选矩阵已建立，但完整前端门禁与真实 Windows/轮转/支持交接矩阵仍未完成；Privacy、Capture、Plugins、About 仅提供明确的里程碑占位，不计为已交付。

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

当前实现边界：capture submit/fetch 与 AI enrichment submitted/started/succeeded/failed 已写入 `logs/link-world.jsonl`，单文件 2 MiB、保留一份轮转；写入与支持包读取均复验字段和敏感 marker。AI job payload、`analysis.requested/created/failed` domain event、IPC result 和日志共享同一 UUID，event payload 只含 analysis id 或稳定 `ai.*` code。search rebuild 使用持久化 job UUID、reindex 使用生成的 operation/job UUID 作为 correlation，完成/取消/失败日志不含 query、索引内容或 raw SQLite error；失败 job 只保存 `search.rebuild_failed` 恢复文案，query/rebuild/reindex/health 的底层数据库错误在 IPC 前统一映射为稳定 `search.*` 文案。startup migration 在 AppState 初始化 storage 前复用同一 logger，UUID 持久化到 guard/last-result 并关联 started/prepared/running/succeeded/failed；running guard 阻断、损坏 guard 和 plan 读取失败只暴露稳定 `migration.*` code；新 guard 的 backup ID、绝对路径或 raw error 不进入日志，legacy guard 的 UUID backup id 只允许作为 `correlationId` 复用。restore 复用 marker `transactionId` 作为 correlation，写入 prepare result 与 last-result，跨 prepare、重启安装、validation、success/rollback 延续；candidate 篡改、marker 损坏和 interrupted switch 只写稳定 `restore.*` code，target/safety backup ID、候选内容、路径与 raw error 不进入日志。Evaluation retry_requested/submitted/started/succeeded/failed/reused 与 startup recovered 也复用 request UUID 作为 correlation，日志只含 object/job identity 和稳定 `evaluation.*` code；独立 trace 仅保存 fingerprint/timing/identity，自动化以正文和 URL 诱饵验证不进入日志。计划内关键流程的确定性代码级覆盖已完成，Week 5 仍需真实矩阵验收。

日志不得包含：

- API key。
- token。
- cookie。
- session。
- 完整正文。
- secret / sensitive 内容。

### 3.3 CLI operations

Windows Local Edition 提供独立 `link-world-cli.exe`。桌面和 CLI 初始化同一 app data 前取得 OS-backed runtime lock；竞争立即以 exit 5 / `ERR_RUNTIME_BUSY` 失败，不等待或抢占。help/version/completion 不打开数据目录。CLI adapter 只调用共享 service，JSON schema v1 保证 stdout 单 document，提示/隐私警告写 stderr，错误只返回稳定 code/message/retryable/correlationId。

CLI 首期没有 daemon，写命令在既有 service timeout 内运行至 durable terminal state。capture、AI、Evaluation 可接受 UUID request ID；重复 identity 复用，跨对象/操作复用 fail closed。默认 object show 仅输出 metadata，正文必须显式 `--include-content`。release 通过独立 artifact 和 `install-link-world-cli.ps1` 分发，PATH 仅在用户显式 `-AddToPath` 时修改 User scope。

自动化入口为 `npm run readiness:cli`，真实 Windows/签名/网络门禁见 `docs/cli_windows_release_matrix.md`。

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

当前自动化已覆盖 0001/0002/0003 历史 schema 到 current 0007、1000 对象 v1 数据集、外键/FTS/隐私/AI/Evaluation/job/provider/tombstone 不变量、未知未来 migration 的 fail-closed、启动 recovery UI 的受限操作展示，以及便携导出默认排除 secret 与本机 storage URI。真实安装包原地升级和进程级强制终止仍未完成。

### 5.1 Evaluation Runtime

当前 Week 6 基线：

- `list_evaluator_capabilities` 返回 schema v1 capability；Prompt 为 `local_deterministic`，GitHub 为可降级的 `network_optional`；两者都不需要 model/sandbox。
- 客户端生成 UUID requestId；后端以相同 job/correlation identity 先持久化 planned/queued，再推进 running 和 passed/failed。
- 同 identity 重试返回原 run，不重复写 artifact/event；跨 object 或 requested evaluator 的 UUID 复用 fail closed。
- 对 failed run 的显式 retry 使用新的客户端 UUID，创建带 `retryOfRunId` 的新 run 并保留父 run；同一父 run/requestId 幂等，普通 trigger 或不同父 run 复用该 UUID fail closed。
- plan/input/output schema version 均持久化为 1；0004 前的 legacy run 保留 NULL identity 并按 version 1 读取；0005 不为 legacy run 伪造 trace。
- artifact/DB 失败会把已占位 run/job/trace 收敛为稳定 `evaluation.*` reason；不允许永久 running 或“失败但无记录”。
- Prompt 本地 evaluator 上限 2 秒；GitHub Evaluation 总上限 15 秒，其中 metadata 预算最多 12 秒；terminal timeout 持久化为 `evaluation.timeout`，GitHub 子请求 timeout 优先降级为 limitation。迟到的纯计算结果不具有外部副作用且不会反写。
- 启动恢复会处理当前 running job，也兼容旧版本“job 已 failed、run 仍 running”的不一致；run/job/trace 统一写 `evaluation.interrupted`，记录 terminal event、清理 artifact 目录并写同 correlation 的脱敏日志。
- `evaluation_traces` 只保存 request/correlation/evaluator identity、input/output hash、execution kind、timeout/latency、status、稳定 error code 与时间戳；不保存标题、URL、正文、plan/output 或 raw error。
- verdict 的 TSX 实现标记为 evaluator inference，evidence 显示 saved content/local/external/sandbox/user 来源；折叠 trace 详情显示 executor、status、latency/timeout、correlation 与截断 fingerprint；定向 TypeScript 编译通过，localhost rendered QA 尚未完成；无 evidence 的非 unknown 结论被 output validator 拒绝。

Week 6 代码契约已覆盖 capability、version、idempotency、timeout、terminal failure、trace、startup recovery、不可变历史 retry 和运行详情 UI。尚未达到发布完成线：相应 readiness 聚合门禁与 localhost rendered QA 仍待实现；真实进程强制终止仍需发布候选矩阵留证。
### 5.2 GitHub Repo Evaluator

当前 Week 7 自动化基线：

- 固定公开 fixture 在无 token 下串行返回 repository/README/latest release，产生 `external_check` evidence、六维评分和 15 秒 trace contract。
- README 只保留 byte length、SHA-256 与 install/usage/example/security 布尔 signals；正文诱饵不进入 output/log。
- `GITHUB_TOKEN` 只允许 `env:GITHUB_TOKEN` SecretStore 引用；adapter 不实现可泄露 token 的 Debug。
- secret object 不发网络请求，返回 `github.policy_denied` limitation；private/404 不复制响应 body。
- 403/429 rate limit 后停止后续可选请求；无自动重试。
- archived/disabled 强制 low-value 风险边界；stars/forks 有权重上限，不能单独决定 verdict。
- 当前未 clone、安装或执行仓库代码；sandbox dry-run 属于后续显式权限能力。

Week 7 代码级 fixture 已完成；真实 GitHub API、Windows 代理/DNS、真实 rate window 和 rendered UI 仍需发布候选留证。

### 5.3 Prompt Evaluator

当前 Week 8 自动化基线：

- pure evaluator 抽取 goal、variables、constraints、acceptance、output formats、dangerous actions、injection 和 credential-like signals。
- versioned 五维 rubric、原 prompt/hash、append-only improvement diff、四个 synthetic test specifications 进入本地 output/artifact。
- 相同 input/plan 重复计算得到相同 score、verdict、dimensions 和 report。
- injection fixture 不能强制 `high_value` 或 `score=1.0`；execution boundary 固定关闭 network/model/sandbox/external actions。
- credential 诱饵保留在本地 immutable original snapshot；generated tests 与 diff 不复制诱饵值。
- runtime integration 验证 versioned output、artifact、trace、lifecycle 和 2 秒上限。

Week 8 代码级验收已完成；真实模型 A/B、sandbox test execution、自动 diff 应用和 rendered UI 不在本周范围，未来启用前需独立权限、trace 与发布矩阵。

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
- Week 9/10 Alpha 聚合自动化门禁：`npm run readiness:alpha`，保存 JSON report；发布候选使用 `scripts/alpha-readiness.ps1 -IncludeSprintGates -IncludeTauriBuild -IncludeNetworkAudits`。2026-07-02 commit `98c9b0f` 的 17/17 steps、MSI/NSIS、npm/RustSec、manifest/checksum 已通过并记录在 `docs/release_evidence/alpha_0.1.0_98c9b0f.md`；工件仍为 unsigned Alpha，真实 Windows 安装/升级/卸载矩阵未完成。
- Week 9 Windows Alpha 发布矩阵：按 `docs/windows_alpha_release_matrix.md` 执行 W9-01 至 W9-14。
- Week 10 邀请制 Alpha 反馈闭环：按 `docs/alpha_feedback_playbook.md` 留存邀请、核心漏斗、P0/P1 和下一阶段决策证据。
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
