# Link World MVP 产品需求文档 (PRD)

本文档面向 AI 开发者，详细说明 MVP 阶段必须实现的用户故事和界面交互约束。MVP 必须控制范围，但不能破坏 `docs/architecture.md`、`docs/api_contracts.ts` 和 `docs/database_schema.sql` 定义的长期架构边界。

## 1. 核心目标

MVP 是一个本地优先的桌面应用 (Tauri 驱动)，并提供最小可用的浏览器主动保存路径。用户的核心心智模型是：“我在浏览内容时一键保存，或者把一个链接/一段文本扔给它，它会在本地解析、用 AI 总结出核心价值，并在我需要的时候立刻被搜出来。”

MVP 不做云端代登录和后台批量抓取。所有采集都必须来自用户主动提交、浏览器扩展当前页保存或官方 API。

## 2. 界面与布局规范 (UI Layout)

桌面端采用经典的极简三栏布局 (Three-pane layout)，风格参考主流 Premium 生产力工具。

- **左侧边栏 (Sidebar)**
  - 顶部：全局搜索框，CMD/CTRL + K 触发指令面板。
  - 中间：固定分类导航，包括 All、Inbox、Articles、GitHub Repos、Prompts、Failed。
  - 底部：设置入口，用于配置模型提供商、凭据、插件和本地存储。
- **中间列表栏 (List View)**
  - 根据左侧选中的分类，显示对应的 Knowledge Object 列表；All/Inbox/Articles/GitHub Repos/Prompts/Failed 必须映射为后端过滤，而不是只改变高亮。
  - 列表按固定页大小加载，MVP 使用显式 Load more，不一次读取全部对象。
  - 每个列表项显示标题、来源图标、AI 一句话摘要、生命周期状态、标签和失败提示。
- **右侧详情区 (Detail View)**
  - 顶部操作栏：来源链接、重新解析、重新评估、删除。
  - 主视图：左侧使用安全 Markdown 阅读器显示 `parsed_documents` 正文，包括目录、标题锚点、Callout、表格和长代码折叠；右侧显示 AI Analysis 和 Evaluation 模块。
  - AI Analysis 显示摘要、质量初评分、关键行动点、风险、置信度和 trace 摘要；只提供运行和进入模型设置的动作，不在对象上下文编辑 provider 或 API Key。
  - Evaluation 显示 verdict、score、维度评分、evidence、limitations 和 artifacts。

## 3. 核心用户故事 (User Stories)

### Epic 1: 配置与初始化 (Onboarding)

- **US 1.1**: 作为首次打开应用的用户，我需要配置供应商品牌、API 协议、Chat Base URL、Embedding Base URL、默认 Chat Model、默认 Embedding Model 和 API Key；系统应为常见供应商提供预设，同时允许输入自定义 OpenAI-compatible 供应商标识。
- **US 1.2**: API Key 必须保存到系统安全凭据存储或本地加密 secret store，不允许进入普通配置表、日志或前端持久化状态。
- **US 1.3**: 如果没有配置模型，应用应该只做基础解析和 FTS 检索，并提示“配置 AI 获取深度洞察”。
- **US 1.4**: 用户可以在保存前测试候选 provider 配置；编辑已有配置且省略 API Key 时可按稳定 config id 复用凭据，结果必须区分鉴权、限流、网络和输出 schema 错误，且不得回显凭据。
- **US 1.5**: 用户可以维护多个 provider 配置，并显式选择唯一默认 Chat 配置；系统不得静默切换到另一家第三方 provider。
- **US 1.6**: 模型配置只出现在正式 Settings route。删除配置必须同时删除 OS credential；删除默认项后 AI 变为未配置，保存、解析和 FTS 仍可用。

### Epic 2: 数据捕获与解析 (Capture & Parse)

- **US 2.1**: 作为用户，我可以在应用界面点击 Add，粘贴一个包含文章或 GitHub Repo 的 URL。
- **US 2.2**: 系统接收到 URL 后，前端调用 Tauri Command 传递给 Rust。Rust 后端立刻创建 `captured` 状态对象，并异步拉取用户提交的 URL。
- **US 2.3**: 拉取完成后，系统通过 Parser 提取正文文本并转换为 Markdown。原始快照写入 `source_snapshots`，解析后正文写入 `parsed_documents`，对象状态变更为 `parsed`。
- **US 2.4**: 作为用户，我可以通过浏览器扩展保存当前页面的 URL、标题、选中文本和清理后的可访问 DOM 片段。扩展只负责安全采集，不生成 Markdown 或实现网站特定排版；URL HTML 和扩展 DOM 统一交给 Rust Parser。
- **US 2.5**: 如果解析失败，系统必须把对象状态标记为 `failed`，记录 `failure_reason`，并允许用户重新尝试或改用浏览器扩展采集当前页面。
- **US 2.6**: 即使没有配置 AI，系统也必须通过 Markdown AST 结构分析提供稳定、安全、可读的基础展示，不依赖模型完成正文排版。

### Epic 3: AI 增强与总结 (AI Enrichment)

- **US 3.1**: 对象进入 `parsed` 状态后，Rust 后台任务从 `parsed_documents` 读取正文，拼接内部 Prompt 模板，并发送给配置好的本地或云端模型。
- **US 3.2**: 获得结构化响应后，系统自动写入 `ai_analysis`，包括 `summary`、`tags`、`key_points`、`risks`、`action_items`、`quality_score`、`confidence` 和可选、版本化的 `display_hints_json`。
- **US 3.3**: 每次 AI 调用必须写入 `ai_traces`，记录 provider、model、capability、prompt template、input hash、output hash、tokens、成本和耗时。
- **US 3.4**: 如果 AI 调用失败，对象保持 `parsed`，错误写入日志和用户可见提示，不允许阻塞后续队列。
- **US 3.5**: AI 展示提示只能在匹配当前 `parsed_document` 且置信度达到门槛时建议文档级布局；它不得修改 Markdown、AST、链接/图片策略或其他安全规则。提示缺失、无效或过期时自动回退到确定性 AST 推断。

### Epic 4: 评估验证 (Evaluation)

- **US 4.1**: 对 GitHub Repo 和 Prompt，用户可以手动触发 Evaluation。系统生成 `EvaluationPlan`，运行对应 evaluator，并把结果写入 `evaluation_runs`。
- **US 4.2**: Evaluation 结果必须包含 verdict、score、维度评分、evidence、limitations、next actions 和 artifacts，而不是只有 AI 摘要。
- **US 4.3**: MVP 的 evaluator 可以先做轻量验证，例如 GitHub 元数据检查、README 分析、Prompt 基准测试样例生成；但接口必须保留 sandbox artifact 和 evidence。

### Epic 5: 检索 (Search)

- **US 5.1**: 用户在全局搜索框输入关键字，系统调用 Rust 提供的 FTS5 接口，在标题、解析正文和 AI 总结中进行全文匹配，快速展示下拉结果。
- **US 5.2**: 如果 Embedding Provider 与 sqlite-vec 可用，则系统提供语义搜索；如果 sqlite-vec 在时间盒内遇到编译或分发困难，MVP 可回退到纯 FTS5，但 schema 和 API 必须保留向量能力边界。

### Epic 6: 本地数据安全 (Post-MVP)

- **US 6.1**: 用户可以在 Storage 设置中创建、列出和验证同机 restore point；备份必须同时覆盖一致的 SQLite 快照与对象存储，并明确提示包含用户内容。
- **US 6.2**: 在 safety snapshot、preflight、数据库关闭和失败 rollback 完成前，产品不得提供 Restore 动作。便携导出必须与原始 restore point 分离，默认排除 secret 与 credential reference。

## 4. 边界异常处理 (Edge Cases)

- **平台限制**: 如果 URL 所在网站开启强力反爬或需要 JavaScript 渲染，Parser 需要识别异常，标记状态为 `failed`，并提示“由于平台限制，请尝试使用浏览器扩展采集当前页面”。
- **LLM 超时或费用耗尽**: AI 调用失败必须被捕获，状态保持 `parsed`，错误详情写入可追踪日志，不允许后台队列崩溃。
- **Embedding 不可用**: 语义搜索自动降级为 FTS5，不影响保存、解析和普通搜索。
- **敏感内容**: `sensitive` 或 `secret` 内容默认禁止发送到第三方 AI，除非用户对该对象或 collection 显式授权。

## 5. 目标用户与核心场景

### 5.1 Primary personas

- **AI/开发者重度信息消费者**
  - 经常收藏 GitHub repo、prompt、技术文章、工具帖。
  - 关注“这个东西值不值得花时间试”。
  - 愿意配置自己的 API Key 和本地模型。
- **独立开发者 / 创业者**
  - 需要把碎片化信息转成产品、竞品、技术选型和执行任务。
  - 更重视可搜索、可回顾、可导出和长期沉淀。
- **研究型知识工作者**
  - 收藏文章、论文、报告、视频和观点帖。
  - 关注来源、证据强度、反例和观点关联。

### 5.2 Core jobs to be done

- 当我看到一个有用内容时，我想用最少动作保存它，避免打断当前浏览流。
- 当我打开收藏库时，我想知道哪些内容真正值得看，而不是面对一堆未读链接。
- 当我搜索一个问题时，我想同时查到原文、AI 摘要、旧评估和相关内容。
- 当我保存一个 prompt 或 repo 时，我想知道它是否可执行、是否过期、是否值得投入时间。
- 当我很久没看某类内容时，我希望系统能重新唤醒真正高价值的内容，而不是机械提醒。

## 6. MVP 功能优先级

| 模块 | Must | Should | Later |
| --- | --- | --- | --- |
| Capture | Add URL、选中文本、浏览器当前页保存 | 截图保存、文件导入 | 移动端分享、官方 API 连接器 |
| Parse | 网页正文、GitHub README、Prompt 文本 | PDF / Markdown | 视频 transcript、Newsletter |
| AI Enrichment | 摘要、标签、行动项、质量初评分 | 风险、关联旧内容 | 自动生成学习路径 |
| Evaluation | GitHub repo、Prompt 最小评估 | 教程可执行性评估 | 浏览器自动试用、代码 sandbox |
| Search | FTS5 全文搜索 | sqlite-vec 语义搜索 | 知识图谱探索 |
| Privacy | 本地优先、安全凭据、敏感内容授权 | Collection 级授权 | 端到端加密云同步 |
| Export | Markdown、JSON | Obsidian folder | Notion、Readwise、团队空间 |

## 7. 商业级非功能要求

### 7.1 Performance

- 应用冷启动到可交互目标：本地桌面端小于 3 秒。
- 1000 条对象内的 FTS 搜索响应目标：P95 小于 200ms。
- 普通网页 URL 从提交到 `parsed` 目标：P95 小于 8 秒，不含网络极慢情况。
- AI enrich 必须异步执行，不得阻塞保存和浏览。
- 列表滚动和详情切换必须在 5000 条对象内保持流畅。

### 7.2 Reliability

- 单个 capture、parse、AI 或 evaluation 任务失败不得阻塞队列。
- 任务必须可重试，且写入结果必须幂等。
- 应用异常退出后，重启必须能恢复未完成任务或标记为可重试。
- 数据库迁移失败时不得破坏已有用户数据，必须给出备份和回滚提示。

### 7.3 Privacy and trust

- 默认本地保存，不要求用户创建云账号。
- 默认不把 sensitive / secret 内容发送到第三方模型。
- 用户必须能看到每个对象是否被 AI 处理、由哪个模型处理、何时处理。
- 用户必须能删除对象及其派生产物，包括快照、索引、向量、AI 分析、评估产物。

### 7.4 UX quality

- 任意失败状态都必须有用户可读的原因和下一步动作。
- AI 输出必须区分“原文事实”“模型推断”“评估结论”。
- 不允许出现空白详情页、无解释 loading、无恢复入口的失败状态。
- 所有 destructive action 必须二次确认或可撤销。

## 8. 产品指标

MVP 需要内置本地可查看的匿名化指标面板；云端遥测必须默认关闭或显式授权。

核心指标：

- 保存成功率：成功创建 `KnowledgeObject` 的 capture 占比。
- 解析成功率：进入 `parsed` 的对象占比。
- AI enrich 成功率：进入 `enriched` 的对象占比。
- Evaluation 使用率：被用户主动评估的对象占比。
- Search success proxy：搜索后用户打开结果的比例。
- Resurfacing value：被重新唤醒后打开、归档、转任务或导出的比例。
- Time to value：从保存到得到可读摘要或失败提示的时间。

## 9. 端到端验收场景

- 用户首次打开应用，不配置 AI，仍可保存 URL、解析正文、全文搜索。
- 用户配置模型后，保存文章会生成摘要、标签、行动项、AI trace。
- 用户保存 GitHub repo 后，可以看到 README 摘要、基础质量评分，并手动触发 repo evaluation。
- 用户保存 Prompt 后，可以看到变量、适用场景、测试样例和改进建议。
- 用户保存受限网页时，系统进入 `failed`，展示原因，并建议使用浏览器扩展当前页保存。
- 用户删除对象后，详情、搜索、AI 分析、向量 chunk、评估产物都不可再被检索到。
- 用户把对象标记为 sensitive 后，第三方 AI 调用必须要求显式授权。
