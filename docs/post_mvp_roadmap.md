# Link World Post-MVP 10 周实施路线图

状态：执行中
基线日期：2026-06-23
目标版本：可邀请测试的 Windows Local Alpha

## 1. 产品目标与成功标准

本阶段把“功能可用的 MVP”推进为“可以安全交给少量外部用户长期使用的本地产品”，同时开始验证 Link World 的核心差异：对内容价值进行有证据、有边界的评估，而不只是生成摘要。

发布门槛：

- 首次启动、模型配置、采集、解析、搜索、AI 分析、删除和恢复路径可完成。
- 旧数据库自动升级且不丢对象、快照、解析正文、分析或评估记录。
- API Key 不进入 SQLite、日志、事件、前端持久化状态或支持包。
- 无模型、模型失效、网络离线时，保存、解析和全文检索仍可用。
- 关键失败有可读原因、重试或明确下一步，不出现无限等待。
- Windows 安装包、升级包和回滚说明经过真实机器验证。
- 每个完成项同步更新 /docs；文档与代码不一致视为未完成。

## 2. 明确边界

本阶段包含：

- Windows Local Edition。
- 少量邀请制 Alpha。
- BYO API 与本地 Ollama。
- 本地 SQLite、对象存储、FTS、AI trace 和 Evaluation。
- GitHub Repo 与 Prompt 的第一批结构化评估。

本阶段不包含：

- 账号、云同步、团队空间、权限管理和计费。
- 移动端与 Web 云端版。
- 托管模型网关。
- 自动跨供应商 failover、成本路由或健康评分。
- 动态第三方插件安装与不受信代码执行。
- 默认把 public 或 personal 内容发送到第三方 AI；自动处理默认关闭。

## 3. 当前执行状态

| 工作流 | 状态 | 交付 |
| --- | --- | --- |
| Sprint 1A 设置与模型配置 | 已实现，待最终回归 | 正式 Settings route；多 provider；默认 Chat 配置；连接测试；删除 |
| Sprint 1B 凭据持久化 | 已实现，待真实机回归 | Windows Credential Manager；数据库只保存引用 |
| Sprint 1C 导航与列表边界 | 已实现，待 UI 回归 | All/Inbox/Articles/GitHub/Prompts/Failed 后端过滤；30 条分页 |
| Sprint 1D 文档契约 | 已同步 | API、架构、安全、PRD、UI、数据库与本路线图 |
| Sprint 2 数据安全 | 执行中 | 备份/两阶段恢复/rollback、0001–0003 historical fixture → 0006、启动迁移保护、启动恢复界面与便携导出已实现；readiness 自动化门禁已建立；真实 Windows 安装包故障矩阵待执行 |
| Sprint 3 采集可靠性 | 执行中 | 启动时 running job 收敛、capture 超时/HTTP/受限页/空正文分类、任务隔离、失败原因脱敏、扩展回退提示、手动 URL 去重和 AI job failureReason 已实现；readiness 自动化已建立，真实网络/进程矩阵待执行 |
| Sprint 4 搜索质量 | 已完成 | FTS 字段权重、secret snippet 抑制、Library filter 组合搜索、索引一致性检查、可重复大库搜索基准、搜索空态/失败态、重建进度和取消边界已实现；5k/20k benchmark 已实测通过 |
| Sprint 5 可观测性 | 执行中 | Diagnostics、失败 job 操作、脱敏支持包、插件指纹/audit/domain correlation 摘要、size/SHA-256 已实现；capture submit/fetch、AI enrichment、search rebuild/reindex、startup migration 与 restore 已接入 2 MiB 有界 JSONL 和持久化 correlation UUID，migration/restore UUID 另写入跨启动 control/result；计划内关键流程的代码级日志覆盖已完成，readiness 自动化与 W5-01 至 W5-14 发布候选矩阵已建立，完整前端门禁和真实 Windows/轮转/支持交接证据仍待完成 |
| Sprint 6-8 Evaluation | 执行中 | Week 6 已建立 capability/plan/input/output/trace v1、UUID request idempotency、planned/running/passed/failed run+job+trace 事务、2 秒执行 timeout、跨版本启动中断 recovery、artifact cleanup、稳定失败码、结构化 correlation 日志，以及 UI inference/evidence/trace 详情（定向 TypeScript 编译通过，rendered QA 待完成）；Prompt evaluator 保持无模型/无 sandbox 的本地确定性执行，GitHub evaluator 已升级为可降级的 `network_optional` 执行；不可变历史 retry 已通过 0006 lineage 实现；Week 7 已加入无 token 公共 GitHub metadata/README/release adapter、隐私与限流降级、六维评分和 stars 非决定性测试，真实 GitHub API/Windows 矩阵尚未完成；Week 8 Prompt 的纯评分抽取、rubric、diff、synthetic tests、注入和 secret 边界已完成 |
| Sprint 9-10 Alpha 发布 | 执行中 | commit `98c9b0f` 的 17 项完整 Alpha 门禁、Sprint 2/3/5 聚合、MSI/NSIS、manifest/checksum、npm/RustSec 审计和 RSA waiver 检查已通过并留证；Week 10 playbook 已建立；真实 Windows 安装升级卸载、签名和 5-15 名用户观察仍待执行 |

## 4. 周度实施计划

### Week 1：设置、凭据、导航和分页

交付：

- 模型配置从对象详情迁入正式 Settings route。
- 支持配置列表、新建、编辑、启用、删除、连接测试和唯一默认 Chat 配置。
- Windows API Key 写入 Credential Manager；读取接口只返回 hasApiKey。
- Sidebar 分类驱动后端过滤，不做纯视觉导航。
- 列表按固定页大小加载并提供 Load more。
- 保留旧 get/update provider command 作为兼容层，新 UI 只用 list/save/delete/set-default。

验收：

- 创建两条同供应商配置不会互相覆盖。
- 修改配置时留空 API Key 会保留原 credential。
- 禁用当前默认项会被拒绝；删除默认项后 AI 显式未配置。
- 对象详情中不存在 API Key、Base URL 或 provider 编辑框。
- Rust 编译、测试编译、TypeScript、前端测试与文档检查通过。

### Week 2：数据安全、迁移与恢复

交付：

- 数据目录清单与版本信息。
- 原子备份：SQLite 一致性快照、对象存储清单、manifest hash。（已实现）
- 恢复前校验、safety backup、私有候选迁移、重启切换、phase recovery 和失败回滚。（已实现；readiness 自动化门禁已建立；真实 Windows 回归待完成）
- JSON/Markdown 导出最小闭环；secret 默认不导出，且不导出 credential reference、内部 job 或本机对象存储路径。（已实现）
- 0001/0002/0003 历史 schema 到 latest（0006）的生成式 migration fixture。（已实现）
- 启动迁移失败进入受限 recovery UI，隐藏 create backup，展示 verified backup ID，并只开放 verify/restore/restart。（已实现；真实安装升级待回归）
- 普通启动检测 pending migration 后先创建并验证 restore point，以 phase guard 阻止不确定 migration 自动重试。（已实现）

验收：

- 1000 对象 v1 fixture 与 v2/v3 定向 fixture 升级不丢行、不破坏外键和 FTS。（自动化已通过；真实用户规模仍需安装包回归）
- 备份中断不产生“看似成功”的半成品。
- 启动 migration fail closed 后不进入普通 Library，recovery UI 可列出并验证 restore point，显式准备恢复或重启重试。（组件与后端自动化已通过；真实安装包回归待完成）
- existing v1 启动升级前备份保持旧 schema；fresh DB 不备份；running guard 中断 fail closed；已提交 migration 的遗留 guard 可收敛。（自动化已通过）
- 恢复失败时原数据目录仍可启动。（自动化故障注入已通过；`npm run readiness:sprint2` 已作为聚合门禁；安装包与强制终止矩阵待完成）
- 本地 restore point 不含 API Key value，但为无损恢复包含用户内容和 opaque credential reference；便携导出不含 credential reference，默认排除 secret 对象。

### Week 3：采集与后台任务可靠性

交付：

- URL 获取、解析、AI job 的超时、取消和重试分类。（capture fetch timeout/HTTP/受限页/空正文/unsupported scheme 分类已实现；AI job auth/rate-limit/timeout/schema/policy/not-configured 分类已实现）
- 应用重启后 queued/running job 的恢复策略。（running job 启动收敛已实现：capture 可重试则回 queued，耗尽或未注册 runner 则 failed）
- 受限页面给出浏览器扩展回退动作。（HTTP 401/403、verification/CAPTCHA/challenge 页面已实现）
- 重复 URL 的幂等边界和用户可见策略。（手动 URL capture 按 normalized canonical URL 返回已有对象并显示 already-saved 状态；DOM/selection 不合并）
- capture/parse 错误码与状态机回归测试。（`capture.*` failureReason code 已覆盖核心路径）

验收：

- 离线/DNS、403、超时、无正文和损坏 HTML 都得到稳定状态。（`npm run readiness:sprint3` 覆盖确定性分类；真实离线/DNS/外部 HTTP 仍按 `sprint3_capture_fault_matrix.md` 执行）
- 单个失败任务不阻塞其他对象。（capture fetch job 自动化已覆盖：403 失败后后续 URL job 仍可成功解析）
- 重启后无永久 running job。（repository 自动化已覆盖）
- 日志不包含正文、cookie、token 或第三方原始错误 body。（capture failure mapper 的诱饵秘密测试已覆盖 HTTP、网络、policy 和 generic fallback；失败原因在写入数据库/事件前完成稳定分类）

### Week 4：搜索质量与大库边界

交付：

- FTS 排序权重、snippet 安全和分类过滤组合。（title/content/AI summary 权重、secret snippet 抑制和 Library filter 组合搜索已实现）
- 索引一致性检查与按对象/全库重建。（missing/stale/orphaned/duplicate 只读检查已实现；rebuild 使用 staging FTS 与 atomic swap；reindex 已有入口）
- 5k、20k 对象基准数据集。（deterministic Rust corpus、250 对象 smoke、5k/20k ignored benchmark 已实现）
- 搜索空态、失败态、重建进度和取消边界。（ObjectList 状态与组件测试已实现；finalizing 阶段不可取消）

验收：

- 标题、正文、AI summary 的权重符合定义。（当前权重：title 8、author 3、content 1、AI summary 4）
- 删除、重新解析和重新分析后索引一致。（health check 可检测 missing/stale/orphaned/duplicate FTS rows）
- 20k 对象下常用查询达到文档化性能预算。（2026-06-26 手动运行 `search_benchmark_5k_objects_reports_budget` 与 `search_benchmark_20k_objects_reports_budget` 均通过；5k 最大单次查询 82ms <= 250ms，20k 最大单次查询 64ms <= 500ms）
- Rebuild 进度可见、取消不发布半成品索引，finalizing 的不可取消边界对用户可见。
- sqlite-vec 仍是可选项，不能阻塞纯 FTS 发布。

### Week 5：诊断与可运维性

交付：

- Diagnostics 设置页：版本、数据路径、数据库健康、对象存储健康、job 失败摘要。（本地快照与 Settings UI 已实现）
- 脱敏支持包与用户确认流程。（schema v1 已实现：固定目录原子 JSON、command/UI 双重确认、稳定 failure code、插件指纹、audit/domain correlation 摘要、最近 100 条重验证 runtime logs、size/SHA-256）
- 关键流程结构化日志和稳定 correlation id。（代码级覆盖已完成：capture submit/fetch 与 AI enrichment 由 job payload、domain event、IPC result 和日志复用 UUID；AI event payload 只含 analysis id 或稳定 `ai.*` code；search rebuild/reindex 以 job UUID 关联完成/取消/失败并只保存稳定 `search.*` code；startup migration 的 UUID 写入 guard/result；restore 的 transaction UUID 写入 prepare result、pending marker 和 last-result，并覆盖成功、中断回滚、candidate 篡改与 marker 损坏，只记录稳定 `restore.*` code）
- operational readiness 清单自动化。（`npm run readiness:sprint5` 与 W5-01 至 W5-14 发布候选矩阵已建立；真实 Windows/支持交接证据待执行）

验收：

- 支持包不含正文、URL query、API Key、credential reference 或 embedding。（后端诱饵秘密测试已覆盖 object title/store body、job payload/error、plugin manifest、audit metadata、credential ref 和本机绝对路径）
- 用户可从失败项进入对应对象或重试动作。（Diagnostics failed job summary 已支持打开对象；capture.fetch_url 支持 retry）
- 无模型配置被视为正常能力降级，不是系统故障。（Diagnostics model status 使用 not_configured_normal_degradation）
- `npm run readiness:sprint5` 聚合门禁通过，真实发布候选按 `sprint5_observability_readiness.md` 留存确认、轮转、隐私和支持交接证据。（自动化已建立；真实矩阵待执行）

### Week 6：Evaluation 通用框架

交付：

- Evaluator capability、plan、run、artifact 和 evidence 契约。（capability/plan/input/output schema v1 与 capability list command 已实现；artifact/evidence contract 已校验）
- 版本化 evaluator 输入输出。（0004 migration 和 API 已持久化/返回 plan/input/output schema version；legacy run 默认 v1）
- Evaluation job 的幂等、超时、失败和 trace。（UUID request 幂等、planned→running→passed/failed、2 秒 timeout、privacy-bounded trace、稳定失败码、artifact cleanup、重启中断 recovery，以及创建新子 run 且保留失败历史的幂等 retry 已实现）
- UI 明确区分原文、模型推断与验证证据。（verdict 标为 Evaluator inference，evidence 按 saved content/local/external/sandbox/user 来源标记；折叠 trace 详情展示 executor、status、latency/timeout、correlation 和截断 fingerprint；定向 TypeScript 编译已通过，rendered QA 待完成）

验收：

- 重复触发不会覆盖历史运行。（同 request identity 返回原 run；新 request 创建新历史；跨 object/evaluator 冲突 fail closed，自动化已覆盖）
- 无证据结论必须标注限制，不能伪装成已验证事实。（output validator 要求无 evidence 时只能为 unknown 且必须有 limitation；UI 分离 inference/evidence）
- evaluator 不能绕过 AI privacy policy。（当前 capability 明确 local deterministic、requiresNetwork/Model/Sandbox=false；后续 model-assisted evaluator 仍需独立 policy gate）

### Week 7：GitHub Repo Evaluator

交付：

- README、license、最近活动和公开仓库元数据采集。（已实现：repo/README/latest release 串行有界 adapter；无 token public fixture 通过）
- 维护性、适用性、接入成本、风险与替代方案维度。（已实现：documentation/licensing/maintenance/adoption/actionability/risk + alternative action）
- 每个结论关联来源或 limitation。（已实现：external_check refs；saved fallback 使用稳定 github.* limitation）
- API 限流、私有仓库和元数据缺失边界。（已实现：rate limit 停止后续请求；private/404 fail closed；README/release 独立降级）

验收：

- 对固定公开仓库 fixture 产生稳定结构。（自动化已通过）
- 无 token 时仍能使用公开数据或明确降级。（自动化已通过；真实 API 待矩阵）
- stars 不作为单一价值结论。（百万 stars archived fixture 仍为 low_value）

### Week 8：Prompt Evaluator

交付：

- 目标、变量、约束、输出格式和危险动作抽取。（已实现：有界变量名、goal/acceptance、format、dangerous action/injection/credential 类别）
- 小型测试用例与评分 rubric。（已实现：rubric v1 五维评分；四个固定 synthetic specifications）
- 改进建议保留原 prompt 与 diff。（已实现：本地 immutable original/hash + append-only improvement diff；不自动应用）
- 不执行外部动作、不自动发送敏感样例。（已实现：network/model/sandbox/external actions 固定关闭）

验收：

- 结果包含 evidence、limitations 和 next actions。（自动化已通过）
- prompt injection 文本不能改变 evaluator 系统边界。（自动化已通过：无法强制 `high_value`/`score=1.0`）
- 测试样例不包含真实用户 secret。（自动化已通过：credential 诱饵不进入 generated tests/diff）

### Week 9：Windows Alpha 工程化

交付：

- 签名、安装、卸载、升级与数据目录保留策略。（矩阵与证据模板已建立：`docs/windows_alpha_release_matrix.md`）
- Windows 10/11 真实机矩阵。（W9-01 至 W9-14 已定义；真实执行待完成）
- Credential Manager、代理、防火墙、长路径和非 ASCII 用户目录回归。（真实执行待完成）
- 安全审计与第三方依赖清单。（2026-07-02 npm production audit 为 0；`quinn-proto` 与 `anyhow` 已升级到修复版本；inactive RSA waiver 由 fail-closed 活跃依赖检查保护，详见 `docs/dependency_security_audit.md`）
- Alpha 聚合门禁：commit `98c9b0f` 在干净工作区完成 17/17 steps，Sprint 2/3/5、Tauri build、npm/RustSec audit 全部通过，见 `docs/release_evidence/alpha_0.1.0_98c9b0f.md`。
- 发布候选组装：已生成 commit-bound MSI/NSIS、`unsigned_alpha` manifest 与 SHA-256 清单并独立复算通过；真实安装和来源分发验证仍待完成。

验收：

- 从上一 Alpha 升级保持数据和 credential 可用。（真实安装包证据待完成）
- 卸载是否保留数据由用户明确选择。（真实安装包证据待完成）
- 安装包来源和版本可验证。（commit/version/manifest/checksum 的本机构建证据已通过；带外发布来源和第二台机器复算仍待完成）

### Week 10：邀请制 Alpha 与反馈闭环

交付：

- 5-15 名目标用户的邀请、反馈模板和数据处理说明。（`docs/alpha_feedback_playbook.md` 已建立；真实邀请待执行）
- P0/P1 缺陷响应和发布停止条件。（已定义）
- 核心漏斗：成功启动、首次保存、首次搜索、首次 AI 分析、首次 Evaluation。（观察模板已定义；真实观察待完成）
- 下一阶段决策记录。（模板已定义；需真实证据后填写）

验收：

- P0 为零，P1 有明确 owner 与 workaround。（待真实 Alpha 反馈证明）
- 至少完成一次安装到真实任务的完整观察。（待完成）
- 根据证据决定优先扩展 Evaluation、采集覆盖或数据安全，而不是同时扩范围。（待真实漏斗证据）

## 5. 质量门禁

每个 Sprint 完成前必须执行：

- Rust format、check、unit/integration tests。
- TypeScript typecheck、Vitest 和 production build。
- Week 9/10 发布候选需运行 `npm run readiness:alpha`，并在发布候选时附加 Sprint 2/3/5 readiness JSON、安装包 manifest/checksum、安全审计处置和 Alpha 反馈记录。
- Settings、Library、无模型、错误模型、离线和删除的浏览器回归。
- migration fixture 与数据完整性检查。
- secret/log 扫描。
- /docs 契约差异检查。

任何门禁失败时，功能状态只能标记为“已实现，待验证”，不能标记为“完成”。

## 6. 变更控制

需要 ADR 的变更：

- 新的持久化语义或不可逆迁移。
- 自动 provider failover、托管网关或云依赖。
- secret backend 或加密模型变化。
- 第三方插件执行模型。
- Evaluation 结论影响自动外部动作。

需要用户明确授权的扩展：

- 账号、云同步或团队协作。
- 将现有本地数据上传到云端。
- 自动发送 public/personal 内容给第三方 AI。
- 收集可关联个人内容的遥测。
