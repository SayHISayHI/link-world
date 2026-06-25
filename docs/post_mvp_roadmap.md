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
| Sprint 2 数据安全 | 执行中 | 备份/两阶段恢复/rollback、0001–0003 fixture、启动迁移保护、启动恢复界面与便携导出已实现；真实 Windows 故障矩阵待实现 |
| Sprint 3 采集可靠性 | 执行中 | 启动时 running job 收敛、capture 超时/HTTP/受限页/空正文分类与扩展回退提示已实现；手动 URL 去重边界已实现；AI job failureReason 分类已实现 |
| Sprint 4 搜索质量 | 执行中 | FTS 字段权重与 secret snippet 抑制已实现；过滤组合、索引一致性检查和大库性能待实现 |
| Sprint 5 可观测性 | 未开始 | 诊断页、健康状态、脱敏支持包 |
| Sprint 6-8 Evaluation | 未开始 | 通用框架、GitHub evaluator、Prompt evaluator |
| Sprint 9-10 Alpha 发布 | 未开始 | 安装升级、安全审计、真实用户反馈闭环 |

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
- 恢复前校验、safety backup、私有候选迁移、重启切换、phase recovery 和失败回滚。（已实现；真实 Windows 回归待完成）
- JSON/Markdown 导出最小闭环；secret 默认不导出，且不导出 credential reference、内部 job 或本机对象存储路径。（已实现）
- 0001/0002/0003 历史 schema 到 latest 的生成式 migration fixture。（已实现）
- 启动迁移失败进入受限 recovery UI，隐藏 create backup，展示 verified backup ID，并只开放 verify/restore/restart。（已实现；真实安装升级待回归）
- 普通启动检测 pending migration 后先创建并验证 restore point，以 phase guard 阻止不确定 migration 自动重试。（已实现）

验收：

- 1000 对象 v1 fixture 与 v2/v3 定向 fixture 升级不丢行、不破坏外键和 FTS。（自动化已通过；真实用户规模仍需安装包回归）
- 备份中断不产生“看似成功”的半成品。
- 启动 migration fail closed 后不进入普通 Library，recovery UI 可列出并验证 restore point，显式准备恢复或重启重试。（组件与后端自动化已通过；真实安装包回归待完成）
- existing v1 启动升级前备份保持旧 schema；fresh DB 不备份；running guard 中断 fail closed；已提交 migration 的遗留 guard 可收敛。（自动化已通过）
- 恢复失败时原数据目录仍可启动。（自动化故障注入已通过；安装包与强制终止矩阵待完成）
- 本地 restore point 不含 API Key value，但为无损恢复包含用户内容和 opaque credential reference；便携导出不含 credential reference，默认排除 secret 对象。

### Week 3：采集与后台任务可靠性

交付：

- URL 获取、解析、AI job 的超时、取消和重试分类。（capture fetch timeout/HTTP/受限页/空正文/unsupported scheme 分类已实现；AI job auth/rate-limit/timeout/schema/policy/not-configured 分类已实现）
- 应用重启后 queued/running job 的恢复策略。（running job 启动收敛已实现：capture 可重试则回 queued，耗尽或未注册 runner 则 failed）
- 受限页面给出浏览器扩展回退动作。（HTTP 401/403、verification/CAPTCHA/challenge 页面已实现）
- 重复 URL 的幂等边界和用户可见策略。（手动 URL capture 按 normalized canonical URL 返回已有对象并显示 already-saved 状态；DOM/selection 不合并）
- capture/parse 错误码与状态机回归测试。（`capture.*` failureReason code 已覆盖核心路径）

验收：

- 离线/DNS、403、超时、无正文和损坏 HTML 都得到稳定状态。（capture 网络/HTTP/parse 分类自动化已覆盖主要分支，真实离线环境仍需手动矩阵）
- 单个失败任务不阻塞其他对象。
- 重启后无永久 running job。（repository 自动化已覆盖）
- 日志不包含正文、cookie、token 或第三方原始错误 body。

### Week 4：搜索质量与大库边界

交付：

- FTS 排序权重、snippet 安全和分类过滤组合。（title/content/AI summary 权重与 secret snippet 抑制已实现；过滤组合待实现）
- 索引一致性检查与按对象/全库重建。
- 5k、20k 对象基准数据集。
- 搜索空态、失败态、重建进度和取消边界。

验收：

- 标题、正文、AI summary 的权重符合定义。（当前权重：title 8、author 3、content 1、AI summary 4）
- 删除、重新解析和重新分析后索引一致。
- 20k 对象下常用查询达到文档化性能预算。
- sqlite-vec 仍是可选项，不能阻塞纯 FTS 发布。

### Week 5：诊断与可运维性

交付：

- Diagnostics 设置页：版本、数据路径、数据库健康、对象存储健康、job 失败摘要。
- 脱敏支持包与用户确认流程。
- 关键流程结构化日志和稳定 correlation id。
- operational readiness 清单自动化。

验收：

- 支持包不含正文、URL query、API Key、credential reference 或 embedding。
- 用户可从失败项进入对应对象或重试动作。
- 无模型配置被视为正常能力降级，不是系统故障。

### Week 6：Evaluation 通用框架

交付：

- Evaluator capability、plan、run、artifact 和 evidence 契约。
- 版本化 evaluator 输入输出。
- Evaluation job 的幂等、超时、失败和 trace。
- UI 明确区分原文、模型推断与验证证据。

验收：

- 重复触发不会覆盖历史运行。
- 无证据结论必须标注限制，不能伪装成已验证事实。
- evaluator 不能绕过 AI privacy policy。

### Week 7：GitHub Repo Evaluator

交付：

- README、license、最近活动和公开仓库元数据采集。
- 维护性、适用性、接入成本、风险与替代方案维度。
- 每个结论关联来源或 limitation。
- API 限流、私有仓库和元数据缺失边界。

验收：

- 对固定公开仓库 fixture 产生稳定结构。
- 无 token 时仍能使用公开数据或明确降级。
- stars 不作为单一价值结论。

### Week 8：Prompt Evaluator

交付：

- 目标、变量、约束、输出格式和危险动作抽取。
- 小型测试用例与评分 rubric。
- 改进建议保留原 prompt 与 diff。
- 不执行外部动作、不自动发送敏感样例。

验收：

- 结果包含 evidence、limitations 和 next actions。
- prompt injection 文本不能改变 evaluator 系统边界。
- 测试样例不包含真实用户 secret。

### Week 9：Windows Alpha 工程化

交付：

- 签名、安装、卸载、升级与数据目录保留策略。
- Windows 10/11 真实机矩阵。
- Credential Manager、代理、防火墙、长路径和非 ASCII 用户目录回归。
- 安全审计与第三方依赖清单。

验收：

- 从上一 Alpha 升级保持数据和 credential 可用。
- 卸载是否保留数据由用户明确选择。
- 安装包来源和版本可验证。

### Week 10：邀请制 Alpha 与反馈闭环

交付：

- 5-15 名目标用户的邀请、反馈模板和数据处理说明。
- P0/P1 缺陷响应和发布停止条件。
- 核心漏斗：成功启动、首次保存、首次搜索、首次 AI 分析、首次 Evaluation。
- 下一阶段决策记录。

验收：

- P0 为零，P1 有明确 owner 与 workaround。
- 至少完成一次安装到真实任务的完整观察。
- 根据证据决定优先扩展 Evaluation、采集覆盖或数据安全，而不是同时扩范围。

## 5. 质量门禁

每个 Sprint 完成前必须执行：

- Rust format、check、unit/integration tests。
- TypeScript typecheck、Vitest 和 production build。
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
