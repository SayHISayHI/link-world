# Link World 技术选型与规范 (Tech Stack)

本文档定义了 Link World 的核心技术基石。任何 AI 工具在接手开发时，必须严格遵守本文档定义的库与框架。

## 1. 整体架构 (Architecture)

Link World 采用 **桌面端 Local-first** 的架构。底层由 Rust 驱动 (Tauri)，提供原生性能、文件系统访问与 SQLite 数据库管理；上层由 React 渲染极致的现代化 UI，前后端通过 Tauri IPC (Inter-Process Communication) 进行异步通信。

## 2. 核心技术栈 (Core Stack)

### 2.1 桌面端宿主 / 后端服务 (Tauri & Rust)
负责所有涉及系统级资源访问的重活。

- **框架**: [Tauri (v2)](https://v2.tauri.app/)
  - 原因: 内存占用极低，包体积小，Rust 安全性高。
- **数据库驱动 / SQL 构建**: [sqlx](https://github.com/launchbadge/sqlx)
  - 原因: 提供类型安全的 SQL 执行、迁移管理和纯 Rust 异步实现。编译期 SQL 检查只在配置好 offline metadata 后启用，避免早期开发被环境变量阻塞。
- **向量数据库扩展**: [sqlite-vec](https://github.com/asg017/sqlite-vec)
  - 原因: 极轻量级的 SQLite 向量搜索扩展，专为本地优先设计。
  - 注意: sqlite-vec 仍处在 pre-v1 阶段，MVP 必须允许在编译或分发遇阻时回退到纯 FTS5。
- **多线程 / 异步运行时**: [Tokio](https://tokio.rs/)
- **网络请求 (官方 API、用户提交 URL 拉取、本地模型网关)**: [reqwest](https://docs.rs/reqwest)
  - 注意: 不得用于规避第三方平台限制或云端代登录批量抓取。

### 2.2 前端展现层 (Frontend)
负责与用户交互，要求具有顶级的商业设计感和流畅度。

- **框架**: React 18+ (配合 Vite)
  - 原因: 虽然 Next.js 也可以静态导出，但 Vite 配合 Tauri 是目前最顺滑的本地桌面端构建流，无需处理多余的 SSR 概念。
- **语言**: TypeScript 5+
- **CSS 框架**: [Tailwind CSS](https://tailwindcss.com/) (v3.4+)
- **UI 组件库**: [shadcn/ui](https://ui.shadcn.com/)
  - 原因: 组件代码下发到项目中，高度可定制，符合 Premium 商业设计感。禁止引入带有浓重后台系统味道的组件库（如 AntD/MUI）。
- **动画库**: [Framer Motion](https://www.framer.com/motion/)
  - 原因: 用于实现丝滑的微交互 (Micro-animations)，增强产品的高级感。
- **图标**: [Lucide React](https://lucide.dev/)
- **全局状态管理**: [Zustand](https://github.com/pmndrs/zustand)
  - 原因: 极度轻量，无需包裹 Provider，适合处理 Tauri IPC 触发的全局状态。
- **表单与校验**: react-hook-form + zod
- **Markdown / AST 阅读器**:
  - `react-markdown` + `remark-gfm` 负责 React 渲染和 GFM。
  - `unified` + `remark-parse` + `mdast-util-to-string` 负责临时 AST 分析和目录模型。
  - `rehype-sanitize` + `rehype-slug` 负责安全清洗和稳定标题锚点。
  - `unist-util-visit` 用于受信任的项目级 Callout 插件。
  - AST 不持久化；禁止引入 `rehype-raw`；重型语法高亮引擎不属于 MVP。

### 2.3 大模型与 AI 集成 (LLM & AI)
遵守 BYO API (Bring Your Own API) 原则。

- **Model Provider Runtime**: 项目自有 capability contract + registry，首个 Rust adapter 使用 `genai 0.6.x`
  - Chat 能力用于摘要、分类、评估计划和 Agent 回答。
  - Embeddings 能力用于向量索引、语义搜索和相关内容召回。
  - 当前文本生成支持 OpenAI Chat Completions、OpenAI Responses、Anthropic Messages、Google Generative AI、Ollama 和 OpenAI-compatible base URL。
  - `provider` 与 `apiFamily` 分离；Ollama、LM Studio、OpenAI、Anthropic、Gemini 及兼容网关都必须通过 registry adapter 接入，业务层不得写供应商专用 HTTP 逻辑。
  - 不允许把模型接口简化为单一 `Chat Completions`，否则会阻塞 embedding、rerank、vision 等后续能力。
- **凭据存储**: Windows Alpha 使用 `keyring 3.6.x` 的 `windows-native` backend 写入 Windows Credential Manager；其他平台在实现对应 OS backend 前不宣称支持。
  - 禁止将 API Key 明文写入普通 SQLite 配置表、日志或前端状态持久化；数据库只保存不可解密的 credential reference。
- **AI 展示提示**: 作为 `ai_analysis` 上版本化、可选的结构化 sidecar 保存。模型只能建议文档级显示模式，不参与基础 Markdown 解析或安全决策。

## 3. 目录架构约定 (Directory Structure)

```text
link-world/
├── src-tauri/                 # Rust 后端与宿主环境
│   ├── src/
│   │   ├── main.rs            # Tauri 应用入口
│   │   ├── commands/          # 暴露给前端的 IPC 接口定义
│   │   ├── db/                # sqlx 相关的数据库模型、迁移文件
│   │   ├── ai/                # AI 请求代理层
│   │   ├── core/              # 业务逻辑 (如 Capture Engine, Evaluation Engine)
│   │   └── plugins/           # 插件系统宿主运行环境
│   ├── migrations/            # SQLite sql 迁移脚本
│   └── Cargo.toml
├── src/                       # React 前端
│   ├── app/                   # 页面入口或路由配置
│   ├── components/
│   │   ├── ui/                # 从 shadcn/ui 下载的基础组件
│   │   └── features/          # 组合业务组件
│   ├── hooks/                 # 自定义 Hook (封装 Tauri Command 调用)
│   ├── store/                 # Zustand 状态定义
│   ├── lib/                   # 工具类 (例如 tailwind `cn` 合并)
│   └── styles/                # 全局 CSS 与 Tailwind 变量
├── docs/                      # AI 基础架构文档库
├── .cursorrules               # AI 全局行为规范
└── package.json
```

## 4. 规范与边界红线
- 前端 `src` 不应该包含任何强耦合系统底层的操作。前端对文件系统的读取，应该发送 Command 让 Rust 去读取，然后返回 Uint8Array 或字符串。
- 绝不要用前端去直连数据库。
- 后端使用 `sqlx` 时，所有业务数据库迁移脚本放在 `src-tauri/migrations` 目录下，交由 Rust 在启动时自动校验与合并。
- FTS5 与 sqlite-vec 都是派生索引。解析后正文的唯一可信来源是 `parsed_documents`，不要从 FTS 表反读正文作为业务数据。

## 5. 测试策略 (Testing Strategy)

商业级实现必须从第一版就具备分层测试，而不是只靠手动试用。

### 5.1 Rust 后端测试

- Unit tests:
  - lifecycle 状态机。
  - parser fallback。
  - privacy policy decision。
  - retry policy。
  - JSON schema parsing。
- Integration tests:
  - SQLite migration 从空库启动。
  - capture -> snapshot -> parsed_document。
  - parsed_document -> ai_analysis -> ai_trace。
  - evaluation_run -> evaluation_artifacts。
  - object deletion -> FTS/vector/object store cleanup。

### 5.2 Frontend 测试

- Component tests:
  - 三栏布局。
  - failed 状态。
  - AI Analysis 卡片。
  - Evaluation 结果展示。
  - Settings 密钥表单不回显完整 key。
- Interaction tests:
  - Add URL。
  - 搜索并打开结果。
  - 手动触发 evaluation。
  - 删除对象确认。

### 5.3 E2E 测试

使用 Playwright 或同等级工具覆盖：

- 首次启动和模型配置。
- 无模型配置下保存 URL。
- 有模型配置下 AI enrich。
- 解析失败和重试。
- 搜索命中正文和 AI summary。
- 删除对象后搜索不可见。

## 6. 质量门槛 (Quality Gates)

合并代码前必须满足：

- TypeScript strict mode 无类型错误。
- Rust `cargo test` 通过。
- Rust `cargo clippy` 无高优先级 warning。
- 前端 lint 通过。
- 数据库 migration 能从空库启动。
- 不存在明文 API Key、token、cookie 日志。
- 核心 IPC command 有错误类型覆盖。
- 对象状态机覆盖 `failed`。

发布前必须满足：

- Windows 主平台打包成功。
- 本地数据目录、日志目录、对象存储目录可配置或可定位。
- 应用升级后能打开旧数据库。
- 删除对象能清理 FTS 和向量索引。
- 失败任务可以查看、重试或解释。

## 7. 发布与打包 (Release & Packaging)

MVP 先以 Windows 桌面为主，但架构不应阻碍 macOS / Linux。

要求：

- Tauri 配置必须最小权限原则。
- 自动更新暂不作为 MVP 必须项，但版本号和 migration 版本必须规范。
- 本地数据目录不能放在应用安装目录。
- crash report 和 telemetry 默认关闭或显式授权。
- release artifact 必须包含版本、平台、构建时间和 schema version。

## 8. 依赖治理 (Dependency Governance)

- 新增核心依赖必须说明用途、替代方案和安全风险。
- 前端禁止引入重型 UI 框架替代 shadcn/ui。
- 后端禁止引入会绕过 Tauri 权限模型的 Node 原生数据库访问。
- sqlite-vec、浏览器扩展、sandbox execution 都必须受 feature flag 控制。
- 所有 prompt template、plugin interface、export schema 都必须版本化。
