# Link World 插件开发规范 (MVP 阶段)

终态架构希望 Connector、Parser、Evaluator、ModelProvider 和 SyncProvider 都可以通过外部动态加载机制扩展。MVP 阶段为了避免过早引入跨进程通信复杂度，插件先实现为编译进 Rust 的内部 Trait 实现，但接口边界必须保持终态兼容。

本指南用于指导 AI 如何为 Link World 增加新的内容采集、解析或评估能力。

## 1. 概念与生命周期

一个知识对象 (`KnowledgeObject`) 的生成链路为：

1. **Connector**: 把用户主动提交的 URL、DOM、选中文本、文件或官方 API 数据变成 `RawCaptureItem`。
2. **Parser**: 把已有的 HTML、Markdown、JSON 或文本提取成标准化 `ParsedDocument`。
3. **Evaluator**: 基于 `KnowledgeObject` 和 `ParsedDocument` 生成 `EvaluationPlan`，执行验证，并产出 `EvaluationResult`。

MVP 可以把这些插件编译进 Rust，但禁止把平台特殊逻辑散落在业务层。新增一个网站解析器时，只应实现新的 Parser 并注册到 Registry。

## 2. ConnectorPlugin 规范

Connector 负责采集，不负责解析。

```rust
pub trait ConnectorPlugin: Send + Sync {
    fn id(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn capabilities(&self) -> ConnectorCapabilities;
    fn capture(&self, input: CaptureInput, ctx: &PluginContext) -> Result<RawCaptureItem, CaptureError>;
}
```

约束：

1. 只处理用户主动提交、当前页面保存、文件导入或官方 API。
2. 不做云端代登录，不托管第三方平台 cookie / session。
3. 必须写入 `PermissionContext`，标记是否允许云端处理和第三方 AI 处理。
4. 本地高级自动化 connector 必须显式展示风险，并默认关闭。

## 3. ParserPlugin 规范

Parser 只负责把已有内容转成 `ParsedDocument`。

```rust
pub trait ParserPlugin: Send + Sync {
    fn id(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn supports(&self, raw: &RawCaptureItem) -> bool;
    fn parse(&self, raw: &RawCaptureItem) -> Result<ParsedDocument, ParseError>;
}
```

开发约束：

1. **不要在 Parser 中发网络请求**。Parser 只处理 `raw.raw_html`、`raw.raw_text`、文件内容或 connector 已经提供的数据。二次拉取属于 Connector 的责任。
2. **不要 `unwrap()`**。DOM 选择器、meta 标签、正文提取失败时，返回缺省值或结构化错误。
3. **必须记录 parser_id 和 parser_version**。写入 `parsed_documents` 时必须保留解析器版本，方便后续重解析和追踪。
4. **必须注册到 ParserRegistry**。未注册的 Parser 不会被调用。

## 4. EvaluatorPlugin 规范

Evaluator 不是一个 prompt 文件。Prompt 只是 evaluator 的一个资源。Evaluator 必须具备计划、执行、证据和产物结构。

```rust
pub trait EvaluatorPlugin: Send + Sync {
    fn id(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn supports(&self, object: &KnowledgeObject) -> bool;
    fn plan(&self, object: &KnowledgeObject, doc: &ParsedDocument, ctx: &PluginContext) -> Result<EvaluationPlan, EvaluationError>;
    fn run(&self, plan: EvaluationPlan, ctx: &EvaluationContext) -> Result<EvaluationResult, EvaluationError>;
}
```

MVP 的 evaluator 可以先轻量实现：

- GitHub Repo Evaluator：读取 repo metadata、README、license、last commit，调用模型生成 verdict、dimensions、evidence 和 limitations。
- Prompt Evaluator：抽取 prompt 目标、变量、输出格式，生成测试样例和改进建议。

但即使 MVP 只调用模型，也必须把结果写入：

- `evaluation_runs.plan_json`
- `evaluation_runs.dimensions_json`
- `evaluation_runs.evidence_json`
- `evaluation_runs.limitations_json`
- `evaluation_runs.next_actions_json`
- `evaluation_artifacts`

## 5. Prompt 资源规范

Prompt 文件存放在 `prompts/system_prompts/` 下，供 evaluator 或 AI enrichment pipeline 使用。

要求：

1. Prompt 输出 JSON 时，示例 JSON 里禁止出现 `//` 注释，因为模型可能照抄导致非法 JSON。
2. Prompt 必须声明输出 schema、枚举值和置信度含义。
3. Prompt 文件名和版本必须写入 `ai_traces.prompt_template_id` 和 `ai_traces.prompt_template_version`。
4. 不允许在 Rust 代码中硬编码大段 prompt。

## 6. Registry 规范

新增插件后必须注册：

- `ConnectorRegistry`
- `ParserRegistry`
- `EvaluatorRegistry`
- `ModelProviderRegistry`

Registry 分发逻辑应基于能力声明和对象类型，不应只依赖硬编码域名。域名匹配可以作为 Parser 支持判断的一部分，但不能替代标准 contract。
