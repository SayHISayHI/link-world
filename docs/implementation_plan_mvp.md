# Link World MVP 开发执行计划

这是写给 AI 代理 (Agent) 的原子级任务清单。实现时必须按照 Phase 推进，避免在基础 contract 未稳定前提前接入复杂 AI 或平台自动化。开始写代码前必须阅读 `docs/README.md` 中定义的文档顺序，尤其是 `backend_architecture.md` 和 `frontend_architecture.md`。

## Phase 0: 文档对齐与工程边界确认 (Architecture Alignment)

目标：确认实现前的架构边界，避免脚手架阶段就写出后续难以维护的结构。

1. 阅读并遵守：
   - `docs/architecture.md`
   - `docs/backend_architecture.md`
   - `docs/frontend_architecture.md`
   - `docs/api_contracts.ts`
   - `docs/database_schema.sql`
   - `docs/security_privacy_compliance.md`
2. 确认后端分层目录：`commands`, `services`, `repositories`, `domain`, `jobs`, `events`, `runtime`, `security`。
3. 确认前端分层目录：`components`, `containers`, `hooks/commands`, `store`, `types`, `lib`。
4. 验证标准：项目脚手架设计不违反 command/service/repository 分层，不让前端直接访问数据库或 secret。

## Phase 1: 基础设施脚手架与环境构建 (Scaffold)

目标：建立一个可以双向通信、包含基本 UI 框架的 Hello World 级应用。

1. 使用 `create-tauri-app` 初始化项目，选择 Vite、React、TypeScript。
2. 安装并配置 Tailwind CSS、shadcn/ui、lucide-react。
3. 在 Rust 端引入 `tauri`, `tokio`, `serde`, `serde_json` 等核心依赖。
4. 编写最基础的 Tauri Command `ping`，并在 React 中调用展示。
5. 验证标准：启动 `npm run tauri dev` 后，出现带 Tailwind 样式的原生桌面窗口，前端能收到 `ping` 响应。

## Phase 2: 本地数据库与迁移基建 (Database)

目标：Rust 能够初始化 SQLite，并创建与 `docs/database_schema.sql` 对齐的核心表。

1. 在 Rust 端引入 `sqlx`，配置 SQLite feature。
2. 在 `src-tauri/migrations` 创建第一版迁移，覆盖：
   - `knowledge_objects`
   - `source_snapshots`
   - `parsed_documents`
   - `ai_analysis`
   - `ai_traces`
   - `evaluation_runs`
   - `evaluation_artifacts`
   - tags / collections
   - `object_relations`
   - `background_jobs`
   - `domain_events`
   - `plugin_manifests`
   - `plugin_permissions`
   - `model_provider_configs`
   - `deletion_tombstones`
   - `local_settings`
   - `audit_logs`
   - `knowledge_fts`
   - 可选 `vec_chunks` / `vector_chunks_meta`
3. 在 Tauri setup 阶段连接本地 SQLite，并执行自动迁移。
4. 实现 `insert_knowledge_object`, `get_all_objects`, `get_object_detail` 的最小数据库访问函数。
5. 建立基础 repository 层，禁止 command 直接写 SQL。
6. 验证标准：应用启动时不报错，本地磁盘中创建 `.sqlite` 数据库文件，核心表存在。

## Phase 3: 核心业务 UI 层搭建 (UI Layout)

目标：实现 PRD 的三栏布局，但先使用 mock 数据。

1. 配置 Zustand，建立 `useLibraryStore`。
2. 按 `docs/frontend_architecture.md` 拆分 `uiStore`, `libraryStore`, `searchStore`, `jobStore`；Settings server state 通过 command hook 管理，draft 保持组件本地。
3. 拆分 React 组件：`Sidebar.tsx`, `ObjectList.tsx`, `ObjectDetail.tsx`, `SettingsPanel.tsx`。
4. Store 中填入覆盖 `captured`, `parsed`, `enriched`, `evaluated`, `failed` 的 mock objects。
5. 详情页展示 mock `parsedDocument`, `aiAnalyses`, `evaluations`, `jobs`。
6. 验证标准：不依赖后端即可完成分类切换、列表点击、详情切换、失败态和后台任务状态展示。

## Phase 4: 采集与解析引擎连通 (Capture Pipeline)

目标：打通 URL 提交、快照保存、正文解析和状态更新。

1. 实现前端 Add URL 输入框，调用 `submit_capture` Command。
2. Rust 收到请求后先入库为 `captured`，并创建 `RawCaptureItem`。
3. 写入 `domain_events(capture.submitted)` 并创建 `background_jobs(capture.fetch_url)`。
4. Job runner 使用 `reqwest` 拉取用户主动提交的 URL，不做云端代登录和后台批量抓取。
5. 保存原始 HTML 到对象存储，并写入 `source_snapshots`。
6. 使用共享 Rust `document_parser` 提取 `title`, `meta`, 正文纯文本和 Markdown；URL HTML 与扩展 DOM 使用相同解析管线，记录 parser id/version，重型解析使用 `spawn_blocking`。
7. 将解析后正文写入 `parsed_documents`，更新对象状态为 `parsed`，写入 `object.parsed` 事件，并触发 Tauri Event 推送前端更新。
8. 如果拉取或解析失败，更新对象状态为 `failed`，写入 `failure_reason`，job 标记失败，不阻塞队列。
9. 验证标准：输入普通博客文章 URL 后，列表出现文章标题，详情页显示 `parsed_documents` 中的正文；输入受限页面时出现 `failed` 状态和可读失败原因。

## Phase 5: 最小浏览器主动保存路径 (Browser Capture)

目标：让 MVP 覆盖真实使用场景，而不是只支持手动粘贴 URL。

1. 定义浏览器扩展与桌面端通信方式，优先采用本地 loopback capture endpoint 或系统 deep link。
2. 浏览器扩展 MVP 只实现“保存当前页”，提交 URL、标题、选中文本和已清洗的可访问 DOM 片段；扩展不生成 Markdown，也不包含站点专用排版逻辑。
3. 扩展不得后台批量抓取，不读取非当前页内容，不托管第三方平台 cookie。
4. 桌面端验证 loopback 请求来源、URL scheme、payload 大小和 DOM 结构，将扩展内容转换为 `RawCaptureItem`，并复用 Phase 4 的 Rust parser。
5. 验证标准：用户在普通网页点击扩展按钮后，桌面端出现新的 Knowledge Object；同一合成页面经 URL HTML 与扩展 DOM 解析后，标题、段落、列表、代码和表格结构保持一致。

## Phase 6: AI Enrichment 与模型配置 (AI Integration)

目标：对解析后的正文进行 AI 摘要，并完整记录 AI trace。

1. 正式 Settings route 允许维护多个 `ModelProviderConfig`，包括稳定 id、供应商标识、`apiFamily`、Chat Base URL、Embedding Base URL、默认模型、能力、enabled 和唯一默认 Chat 项，并提供不泄露凭据的连接测试；对象详情不得编辑 provider。
2. Windows Alpha 的 API Key 保存到 Credential Manager；普通配置表只保存 credential reference。删除 provider 时必须同步删除 credential，禁止写入日志或前端持久化状态。
3. Rust 端由项目自有 `ModelProviderRegistry` 和 capability contract 隔离业务层，内置 `genai` adapter 支持 OpenAI Chat Completions/Responses、Anthropic Messages、Google Generative AI、Ollama 及 OpenAI-compatible Chat；Embedding 使用独立 capability contract 后续实现。
4. 当对象进入 `parsed` 状态后，从 `parsed_documents` 读取正文，使用 `builtin.general_enrichment` prompt `0.2.0` 拼接输入并发送 Chat 请求。
5. 解析 schema version 2 的 LLM JSON，写入 `ai_analysis` 和可选的版本化 `display_hints_json`，同时写入 `ai_traces`；无效展示提示不得让主体分析失败。
6. AI 调用失败时对象保持 `parsed`，记录错误，不允许后台任务崩溃。
7. 验证标准：切换协议时请求由 registry 发送到正确 endpoint；多配置互不覆盖且默认项显式；连接测试返回 provider/model/latency；文章详情页在数秒后出现 AI 摘要、分数、行动项和 trace 摘要；删除默认配置、提示无效或记录过期时仍使用 Markdown AST 推断完成基础阅读展示。

## Phase 7: Evaluation Engine 最小闭环

目标：保留产品核心差异，即“判断价值并产生证据”，而不是只做总结。

1. 实现 `EvaluatorPlugin` trait，包含 `supports`, `plan`, `run`。
2. 实现 GitHub Repo Evaluator 的最小版本：
   - 读取 repo URL、README、license、stars、last commit 等元数据。
   - 生成 `EvaluationPlan`。
   - 调用模型产出 verdict、score、dimensions、evidence、limitations、next actions。
3. 实现 Prompt Evaluator 的最小版本：
   - 抽取 prompt 目标、变量和输出格式。
   - 生成少量测试样例和改进建议。
4. 将结果写入 `evaluation_runs` 和 `evaluation_artifacts`。
5. 验证标准：用户点击“重新评估”后，详情页出现结构化 verdict、维度评分和 evidence。

## Phase 8: 全文检索、可选向量检索与打磨 (Search & Polish)

1. 连通 SQLite FTS5，在写入或更新 `parsed_documents` / `ai_analysis` 后维护 `knowledge_fts`。
2. 前端绑定 CMD/CTRL + K，全局搜索实时调用 `search_hybrid`。
3. 如果 sqlite-vec 可稳定分发，接入 `vec_chunks` 和 `vector_chunks_meta`，通过 rowid 关联 chunk metadata。
4. 如果 sqlite-vec 遇到编译或分发风险，MVP 回退到纯 FTS5，但保留 API 与 schema 边界。
5. 检查隐私红线：日志中不得出现正文、API Key、token、cookie。
6. 验证标准：关键词搜索能命中标题、正文和 AI 摘要；语义搜索不可用时产品仍可完整使用。
