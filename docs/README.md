# Link World 文档索引

本目录是 Link World 的产品、架构和工程规范源。后续实现代码前，必须先确认对应文档没有冲突。

## Reading Order

1. [architecture.md](./architecture.md)
   - 总体架构、核心原则、部署形态、插件系统、事件模型、任务模型、SLO、数据治理。
2. [product_requirements.md](./product_requirements.md)
   - MVP 用户故事、界面约束、目标用户、功能优先级、产品指标、端到端验收。
3. [backend_architecture.md](./backend_architecture.md)
   - Rust/Tauri 后端分层、AppState、错误处理、事务、异步隔离、后台任务和测试要求。
4. [frontend_architecture.md](./frontend_architecture.md)
   - React 前端状态分层、组件边界、Tauri command hooks、三栏布局、错误状态和测试要求。
5. [api_contracts.ts](./api_contracts.ts)
   - Tauri IPC、核心类型、后台任务、插件权限、错误码和维护命令的契约。
6. [database_schema.sql](./database_schema.sql)
   - SQLite schema 蓝图，包括 knowledge objects、parsed documents、AI trace、evaluation、jobs、events、plugins。
7. [state_machines.md](./state_machines.md)
   - Knowledge Object、Job、AI、Evaluation、Deletion、Sync 状态流转和禁止状态。
8. [database_migrations.md](./database_migrations.md)
   - SQLite migration 命名、事务、0001–0003 historical fixture → current 0006、启动 restore-point guard、数据不变量和回滚策略。
9. [backup_and_restore.md](./backup_and_restore.md)
   - 本地 restore point、manifest、原子创建、两阶段恢复、phase rollback、启动 recovery UI、便携 Markdown/JSON 导出、凭据边界和发布前故障矩阵。
10. [tech_stack.md](./tech_stack.md)
   - 技术栈、目录结构、测试策略、质量门槛、发布和依赖治理。
11. [security_privacy_compliance.md](./security_privacy_compliance.md)
   - 威胁模型、隐私分级、凭据、插件权限、AI trace、删除和平台合规边界。
12. [security_and_privacy_policies.md](./security_and_privacy_policies.md)
   - 安全与脱敏红线、AI policy gate、插件 policy gate、诊断和删除策略。
13. [ui_components_inventory.md](./ui_components_inventory.md)
   - UI 组件清单、组件职责、状态要求、设置页、搜索、AI/Evaluation 面板规范。
14. [external_api_resilience.md](./external_api_resilience.md)
   - 第三方调用 timeout、retry、fallback、rate limit、错误映射和韧性测试。
15. [sync_architecture.md](./sync_architecture.md)
   - 多端同步、可同步字段、冲突解决、tombstone 同步、隐私同步边界。
16. [operational_readiness.md](./operational_readiness.md)
   - 发布就绪、环境矩阵、诊断、备份恢复、迁移策略和事故处理。
17. [devops_and_ci.md](./devops_and_ci.md)
   - CI、构建矩阵、测试门槛、打包、签名、发布、回滚和诊断包。
18. [sprint2_windows_fault_matrix.md](./sprint2_windows_fault_matrix.md)
   - Sprint 2 数据安全、迁移、恢复在 Windows 安装包和真实进程故障下的发布验收矩阵。
19. [sprint3_capture_fault_matrix.md](./sprint3_capture_fault_matrix.md)
   - Sprint 3 URL capture、后台任务、真实网络与进程中断的发布验收矩阵。
20. [sprint5_observability_readiness.md](./sprint5_observability_readiness.md)
   - Sprint 5 Diagnostics、结构化日志、支持包隐私和真实 Windows 支持交接验收矩阵。
21. [windows_alpha_release_matrix.md](./windows_alpha_release_matrix.md)
   - Week 9 Windows Alpha 打包、安装、升级、卸载、签名、依赖审计和真实机发布候选矩阵。
22. [dependency_security_audit.md](./dependency_security_audit.md)
   - Windows Alpha 的 npm/RustSec 审计、漏洞处置、RSA waiver 失效条件和依赖面缩减记录。
23. [release_evidence/alpha_0.1.0_98c9b0f.md](./release_evidence/alpha_0.1.0_98c9b0f.md)
   - Alpha 0.1.0 RC 的干净 commit 门禁、MSI/NSIS、hash、签名状态和未执行真实机边界。
24. [alpha_feedback_playbook.md](./alpha_feedback_playbook.md)
   - Week 10 邀请制 Alpha 用户邀请、反馈、P0/P1 响应、核心漏斗和下一阶段决策模板。
25. [plugins/plugin_development_guide.md](./plugins/plugin_development_guide.md)
   - Connector、Parser、Evaluator、Prompt 资源和 Registry 规范。
26. [github_repo_evaluator.md](./github_repo_evaluator.md)
   - GitHub 公共 metadata adapter、限流/私有边界、隐私策略、评分上限和 Week 7 验收证据。
27. [prompt_evaluator.md](./prompt_evaluator.md)
   - Prompt 抽取、rubric、结构化 diff、synthetic tests、注入与 secret 边界和 Week 8 验收证据。
28. [post_mvp_roadmap.md](./post_mvp_roadmap.md)
   - MVP 后 10 周执行范围、周度交付、验收门槛、当前状态和明确排除项。
29. [cli_development_plan.md](./cli_development_plan.md)
   - Alpha 后 CLI 的共享核心、命令范围、机器输出、并发安全、分阶段交付和发布门禁。
30. [../tests/docs/testing_strategy.md](../tests/docs/testing_strategy.md)
   - 测试分层、fixtures、AI evals、回归测试和发布测试清单。
31. [adr/README.md](./adr/README.md)
   - 架构决策记录，解释关键技术和产品架构取舍，包括 Markdown/AST 渲染与 AI 展示提示边界。

## Source of Truth

- 架构原则以 `architecture.md` 为准。
- 用户体验和 MVP 范围以 `product_requirements.md` 为准。
- 前后端通信以 `api_contracts.ts` 为准。
- 数据存储以 `database_schema.sql` 为准。
- 状态流转以 `state_machines.md` 为准。
- 数据库升级以 `database_migrations.md` 为准。
- 本地备份、验证和恢复安全边界以 `backup_and_restore.md` 为准。
- 后端实现模式以 `backend_architecture.md` 为准。
- 前端实现模式以 `frontend_architecture.md` 为准。
- 安全和隐私争议以 `security_privacy_compliance.md` 和 `security_and_privacy_policies.md` 为准。
- UI 组件职责以 `ui_components_inventory.md` 为准。
- 第三方调用韧性以 `external_api_resilience.md` 为准。
- 多端同步争议以 `sync_architecture.md` 为准。
- 测试策略以 `tests/docs/testing_strategy.md` 为准。
- 发布和运维争议以 `operational_readiness.md` 为准。
- Windows 数据安全发布故障矩阵以 `sprint2_windows_fault_matrix.md` 为准。
- URL capture 与后台任务发布故障矩阵以 `sprint3_capture_fault_matrix.md` 为准。
- 本地可观测性、支持包隐私与支持交接发布矩阵以 `sprint5_observability_readiness.md` 为准。
- Windows Alpha 打包、安装、升级、卸载、签名和安全审计矩阵以 `windows_alpha_release_matrix.md` 为准。
- 邀请制 Alpha 反馈、数据处理、缺陷响应和下一阶段决策以 `alpha_feedback_playbook.md` 为准。
- GitHub Repo Evaluator 的外部数据、限流、隐私和评分边界以 `github_repo_evaluator.md` 为准。
- Prompt Evaluator 的抽取、rubric、diff、测试与注入边界以 `prompt_evaluator.md` 为准。
- 当前里程碑、周度范围与完成状态以 `post_mvp_roadmap.md` 为准。
- CLI 的目标范围、命令契约、并发策略和阶段门禁以 `cli_development_plan.md` 为准；在该计划进入实施前，CLI 仍是 Proposed 能力。
- 架构取舍的历史原因以 `adr/` 为准。

## Documentation Versioning Policy

- 主文档始终保持最新状态。
- 普通迭代直接修改对应 source-of-truth 文档。
- 不创建零散的 `v2`、`v3`、`new`、`final`、`updated` 等副本文档。
- 重大架构决策必须更新主文档，并新增或更新 `docs/adr/` 下的 ADR。
- ADR 记录为什么做出决策；主文档记录当前应该如何实现。
- 如果代码和文档不一致，必须先解决不一致，再继续实现。

## Change Policy

- 修改 API contract 时，必须同步检查 PRD、schema、实现计划和安全文档。
- 修改 schema 时，必须同步检查 API contract、migration plan 和删除策略。
- 修改状态流转时，必须更新 `state_machines.md`、API contract 和相关测试。
- 修改插件能力时，必须同步检查权限模型、安全文档和 audit 需求。
- 修改模型调用方式时，必须同步检查 AI trace、privacy policy 和成本记录。
- 新增高风险功能时，必须增加 feature flag、用户授权、日志脱敏和回滚策略。
- 修改后端分层、状态机、任务或错误模型时，必须更新 `backend_architecture.md` 和相关 ADR。
- 修改前端状态、路由、command 调用方式时，必须更新 `frontend_architecture.md`。
- 修改 UI 组件职责、状态或交互模式时，必须更新 `ui_components_inventory.md`。
- 修改同步字段、删除语义或冲突策略时，必须更新 `sync_architecture.md`。
- 修改第三方 API 调用、模型供应商或重试策略时，必须更新 `external_api_resilience.md`。
- 修改发布、打包、签名或 CI 门槛时，必须更新 `devops_and_ci.md`。
- 修改测试约束、fixtures 或 eval 基准时，必须更新 `tests/docs/testing_strategy.md`。
- 修改 Markdown 持久化、AST 派生、渲染插件管线或 AI display hints 时，必须同步检查 API、schema、前端架构、安全策略、测试规范和相关 ADR。
- 任何代码变更如果影响架构、数据模型、API 契约、状态机、安全策略、同步语义、持久化、AI 行为、插件权限、发布流程或测试策略，必须同步修改相关文档。
