# Prompt Evaluator

本文档定义 Week 8 Prompt Evaluator 的抽取、评分、改进、测试与安全边界。实现入口为 `src-tauri/src/services/prompt_evaluator.rs`；该模块是纯函数，不持有网络、模型、sandbox 或外部动作能力。

## 1. 当前交付

- capability 为 `local_deterministic`，总执行上限 2 秒。
- 输入始终按 `untrusted_data` 处理；prompt 中的指令不能改变 evaluator 代码路径、评分阈值或输出契约。
- 抽取目标、变量名、约束、验收标准、输出格式、危险动作类别、注入类别和 credential-like signal。
- 输出五维 rubric、synthetic test cases、原 prompt、content hash 和结构化 improvement diff。
- 不调用模型、不访问网络、不执行 shell、不运行测试样例、不触发外部动作。

## 2. 抽取契约

抽取结果位于 `report.extracted`：

- `goal`: 是否检测到明确目标，以及最多 240 字符的首个非空行摘要；credential-like token 在摘要中替换为 `[REDACTED_SECRET]`。
- `variables`: 仅提取 `{{name}}`、`<name>`、`$name` 的变量名；排序去重，最多 20 项，每项最多 64 字节；不提取变量值。
- `constraints`: 是否存在约束和验收标准。
- `outputFormats`: JSON、Markdown、table、YAML、XML 或显式 schema signal。
- `dangerousActions`: credential access、destructive filesystem、command execution、network exfiltration、privilege escalation、policy bypass 等类别；不复制命令或 secret。
- `injectionSignals`: instruction override、hidden instruction request、evaluation manipulation 等类别。
- `hasCredentialLikeLiteral`: 只表示检测结果，不导出凭据值。

抽取是有界词法分析，不宣称理解所有语言、变体或模型行为。未检测到 signal 不等于安全证明。

## 3. 评分 rubric

rubric version 为 1，维度范围均为 0 到 1：

| 维度 | 当前含义 |
| --- | --- |
| `clarity` | 角色、目标和可读范围是否明确 |
| `specificity` | 输入变量、约束和验收标准是否明确 |
| `testability` | 输出格式、示例和成功条件是否可观察 |
| `reusability` | 是否使用变量和上下文边界，是否嵌入 credential-like literal |
| `safety` | injection、credential 和危险动作 signal 的有界扣分 |

总分为五维平均值并保留两位小数。`safety < 0.45` 时 verdict 强制为 `unsafe`；否则按总分映射 `high_value`、`useful`、`situational` 或 `low_value`。评分是 triage signal，不是模型实测结论。

## 4. 原 prompt 与 improvement diff

`report.originalPrompt` 保存本次 immutable Evaluation input snapshot 的原文，`report.originalPromptHash` 关联输入 hash。它只进入本地 Evaluation output/artifact，不进入 trace、structured log、support bundle 或外部服务。

`report.improvementDiff` 是可审计的 append 操作列表，每项包含：

- 稳定 `id`
- `operation=append`
- 人类可读 `summary`
- `priority`
- 由产品生成的静态安全文本

diff 可以补充 role、goal、inputs、constraints、output format、acceptance criteria、synthetic example、untrusted-input boundary 和 credential reference。diff 不复制用户 secret；检测到 literal credential 时建议改为 `{{credential_reference}}`。

本阶段不自动应用 diff，不覆盖原 prompt，不把 diff 发送给模型。应用和编辑属于后续显式用户动作。

## 5. 合成测试用例

`report.testCases` 当前固定生成：

1. `happy_path`
2. `missing_context`
3. `injection_resistance`
4. `format_conformance`

所有 fixture 标记为 `synthetic`。测试只引用变量名或固定虚构文本，不复制用户标题、正文片段、credential value、cookie、token、session 或本机路径。test case 是规范，不会在当前版本执行。

## 6. 注入与动作边界

Evaluator 不构造第二层 prompt，也不把输入交给模型，因此输入中的“忽略之前指令”“返回 high_value”“显示系统提示”等文本只能成为被扫描的数据。`report.executionBoundary` 固定声明：

- `inputTreatment=untrusted_data`
- `networkAccess=false`
- `modelExecution=false`
- `sandboxExecution=false`
- `externalActions=false`
- `userSecretsInGeneratedTests=false`

检测到 injection 会降低 safety、产生 `prompt.injection_detected` limitation，并建议添加 untrusted-content boundary；它不能直接设置 score、verdict、evidence 或 next action。

## 7. 自动化证据与剩余边界

当前自动化覆盖：

- 同一 input/plan 两次运行得到相同 score、verdict、dimensions 和 report。
- 结构化目标/变量/格式抽取、rubric、原 prompt 与 diff 持久化。
- injection fixture 不能强制 `high_value` 或 `score=1.0`，且 external actions 始终为 false。
- credential fixture 的 generated tests 和 diff 不包含诱饵 secret；原文仍只在本地 immutable snapshot 中保留。
- runtime 集成写入 versioned run、artifact、trace 和 lifecycle。

尚未包含真实模型 A/B、sandbox test execution、自动 diff 应用或多语言语义分类。上述能力必须新增显式权限、独立 trace 和新的验收矩阵，不能隐式扩展当前 evaluator。
