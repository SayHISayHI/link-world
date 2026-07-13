# Week 10 邀请制 Alpha 与反馈闭环

状态: Operationally ready，邀请、反馈、缺陷响应、脱敏证据契约和自动门禁已建立；真实 5-15 名目标用户观察与下一阶段决策证据仍需执行。
适用范围: Node Tide Windows Local Alpha 的邀请制测试、反馈采集、数据处理、P0/P1 响应、核心漏斗观察和下一阶段决策。

## 1. 目标与边界

Week 10 的目标是用少量真实任务验证 Node Tide 是否能让目标用户完成“保存、理解、检索和评估”的核心闭环，并据此决定下一阶段优先扩展 Evaluation、采集覆盖或数据安全。

本阶段不扩大产品范围:

- 不开放 public download。
- 不新增云账号、同步、团队空间或计费。
- 不默认收集遥测。
- 不要求用户分享正文、API key、cookie、session、prompt 原文或私有数据。
- 支持包只在用户显式确认后本地生成，由用户主动选择是否分享。

## 2. Alpha 用户画像与邀请范围

邀请 5-15 名用户，优先覆盖:

- AI/开发者重度信息消费者。
- 独立开发者或技术创业者。
- 研究型知识工作者。

准入条件:

- 使用 Windows 10/11。
- 愿意安装未公开 Alpha build。
- 愿意使用自己的公开网页、GitHub repo 或非敏感 prompt 完成测试任务。
- 理解产品本地优先，默认不上传内容。
- 能接受手动反馈或 30 分钟观察访谈。

排除条件:

- 必须处理公司机密、客户隐私或不可分享 secret 的任务。
- 需要移动端、云同步、团队协作或托管模型网关才能完成测试。
- 不能接受 unsigned alpha 或手动安装来源验证。

## 3. 邀请文案模板

```text
Subject: Node Tide Windows Alpha invitation

Hi <name>,

I am running a small Windows Alpha for Node Tide, a local-first desktop app for saving, searching and evaluating articles, GitHub repos and prompts.

This Alpha is intentionally small:
- It runs locally on Windows.
- It does not require a cloud account.
- Model use is BYO API or local Ollama.
- Please do not test with secrets, customer data, private credentials or content you cannot share in feedback.
- Support bundles are local-only and require explicit confirmation before you choose whether to share them.

The test should take about 30-45 minutes. The core flow is:
1. Install and start the app.
2. Save one useful article or repo.
3. Search for it.
4. Optionally configure a model and run AI analysis.
5. Run one GitHub or Prompt Evaluation.
6. Tell us where the app was useful, confusing or broken.

Known Alpha constraints:
- Windows is the only supported platform for this test.
- Some installers may be unsigned; verify the SHA-256 from the release note.
- Browser extension and external network paths may still have rough edges.

If you are interested, reply with your Windows version and whether you plan to use a cloud model, local model or no model.
```

## 4. Consent 与数据处理说明

每位用户测试前必须看到并确认:

- Alpha 反馈用于产品改进和缺陷排查。
- 用户不应提交真实 API key、cookie、session、password、private customer data 或无法分享的正文。
- 默认不采集云端遥测。
- 用户可以只提交文字反馈，不分享支持包。
- 如果用户选择分享支持包，应先查看文件；支持包不应包含正文、URL query/fragment、credential reference、raw job payload、embedding 或本机绝对路径。
- 反馈记录中可以保存 app version、Windows version、稳定 error code、object/job/evaluation/correlation id 和复现步骤。
- 反馈记录不得保存 API key、token、cookie、session、完整正文、prompt 原文、source snapshot 或 embedding。

## 5. 核心漏斗观察

每位 Alpha 用户至少观察或自报以下漏斗:

| 步骤 | 成功标准 | 证据 |
| --- | --- | --- |
| 启动 | 安装后 3 秒级进入可交互或明确 recovery 状态 | version、Windows version、启动结果 |
| 首次保存 | 用户能保存一个 URL 或浏览器捕获内容 | object id、capture status、失败 code 或成功 parsed |
| 首次阅读 | 详情页显示安全 Markdown 或清楚失败原因 | parsed/failure state、截图或描述 |
| 首次搜索 | 用户能用关键词找到已保存内容 | query 类别、结果打开情况，不保存完整敏感 query |
| 首次 AI 分析 | 有模型时能生成 analysis；无模型时显示正常降级 | provider family、success/failure stable code |
| 首次 Evaluation | GitHub Repo 或 Prompt 产生 verdict、score、evidence、limitations | evaluator type、run id、status、用户理解度 |
| 删除或恢复信任 | 用户理解删除、备份、支持包或 diagnostics 的边界 | 操作是否成功、困惑点 |

## 6. 反馈表模板

```text
Alpha feedback record

Participant code:
Date:
Observer:
App version:
Commit SHA:
Windows version:
Install type:
Model mode: none / cloud BYO / local Ollama / other

Task context:
- Article / GitHub repo / Prompt / mixed:
- User goal:
- Content privacy used for test: public / personal synthetic / sensitive synthetic

Funnel:
- Started successfully: yes/no
- First save status:
- First search status:
- AI analysis status:
- Evaluation status:
- Diagnostics/support bundle used: yes/no

Top moments:
- Most useful:
- Most confusing:
- Slowest or most brittle:
- Missing next action:

Bugs:
- Severity:
- Module:
- Stable error code:
- Object/job/run/correlation id:
- Repro steps:
- Workaround:
- Owner:

Privacy notes:
- Any accidental sensitive content in feedback? yes/no
- Support bundle shared? yes/no
- Support bundle reviewed by user first? yes/no

Decision signal:
- Expand Evaluation next?
- Expand capture coverage next?
- Invest in data safety/recovery next?
- Evidence:
```

## 7. 缺陷严重级别

| Severity | 定义 | 发布动作 |
| --- | --- | --- |
| P0 | 数据丢失、secret 泄漏、无法启动且无 recovery、错误删除用户数据、安装包来源不可验证 | 立即停止发布；必须修复并重新跑 Week 9/10 相关证据 |
| P1 | 核心漏斗阻断但有 workaround，例如无法保存、搜索、AI/Evaluation 全失败、升级后 credential 不可用 | 必须有 owner、workaround 和修复计划；外部扩大发布前解决或明确 waiver |
| P2 | 明显体验问题、性能差、错误文案不清、单一环境失败 | 进入下一迭代队列；若集中出现可升级为 P1 |
| P3 | polish、文案、低频边界 | 排期但不阻断 Alpha |

发布停止条件:

- 任意开放 P0。
- 超过 2 个无 workaround 的开放 P1。
- 支持包或日志发现 secret/content 泄漏。
- 同一核心漏斗步骤在 3 名以上用户中失败且无明确 workaround。
- 安装/升级/卸载路径出现数据保留语义不清或误删风险。

## 8. 支持响应 SLA

邀请制 Alpha 的最低响应:

- P0: 当天确认，停止新邀请，修复后重新发布候选。
- P1: 1 个工作日内确认 owner 和 workaround。
- P2: 3 个工作日内归类并决定是否进入下一 patch。
- P3: 汇总到下一阶段 polish。

支持沟通只使用脱敏证据。需要支持包时，先让用户在 Diagnostics 中显式确认导出并自行决定是否分享。

## 9. 下一阶段决策记录模板

```text
Alpha decision record

Date:
Release candidate:
Participants completed:
P0 count:
Open P1 count:
Core funnel completion:
- start:
- first save:
- first search:
- AI analysis:
- Evaluation:

Observed value:

Primary blockers:

Decision:
- Prioritize Evaluation depth / capture coverage / data safety / other:

Evidence:

Explicitly not doing next:

Owners and next milestones:
```

## 10. 完成边界

Week 10 可以标记为完成的最低条件:

- 至少邀请 5 名目标用户，且记录邀请结果。
- 至少 1 名用户完成从安装到真实任务的完整观察。
- 所有反馈记录按本 playbook 脱敏保存。
- P0 为零。
- 每个开放 P1 都有 owner、workaround 和到期日。
- 核心漏斗数据能支持下一阶段选择，不能同时把 Evaluation、采集覆盖和数据安全都列为最高优先级。
- `docs/post_mvp_roadmap.md` 和 `docs/operational_readiness.md` 记录观察日期、参与人数、P0/P1 状态和下一阶段决策。

只准备邀请文案、只内部自测、或只收集非结构化聊天反馈，都不足以证明 Week 10 完成。

## 11. 脱敏证据目录与自动门禁

真实证据只保存在本机根目录的 `alpha-evidence/`，该目录已被 Git 忽略，不应直接提交。目录必须包含同一 `releaseCommit` 的三个 schema v1 JSON：

- `participants.json`：`evidenceKind=alpha_observation`、5-15 个匿名 `participantCode`、consent、完成观察的 `observedOn`、Windows 大版本和枚举化 funnel 结果。
- `issues.json`：只保存 issue id、P0-P3、open/closed、owner、workaround 和 dueDate，不保存正文或原始报错。
- `decision.json`：`decisionDate`、完成观察数、唯一 `primaryPriority`、20-500 字脱敏摘要、明确不做项和 owner role/code。

运行真实门禁：

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/alpha-feedback-readiness.ps1 -EvidenceDirectory alpha-evidence -OutputPath alpha-feedback-readiness.json
```

门禁强制要求邀请 5-15 人、至少一次完整观察、完整观察通过 start/save/search/Evaluation、AI 为 `passed` 或正常的 `not_configured`、零开放 P0、每个开放 P1 都有 owner/workaround/dueDate，并且决策只选择 Evaluation 深度、采集覆盖、数据安全或 other 中的一个最高优先级。

校验器会递归拒绝 API key/token/cookie/session/password、正文、Prompt、source snapshot、embedding、URL/query、本机绝对路径和直接身份字段，也会拒绝 secret-like 值。输出报告只含聚合计数，不含参与者逐条记录。

仓库中的 `tests/fixtures/alpha-feedback-ready` 是明确标注的 synthetic contract fixture，只用于 CI。`npm run test:alpha-feedback-contract` 同时证明：合法 synthetic fixture 可通过测试开关、synthetic fixture 不能冒充真实证据、含敏感字段的证据必定失败。它不能证明任何真实用户已经被邀请或观察。
