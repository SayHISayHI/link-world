# Link World 前端架构规范

状态: Draft
适用范围: React 18+ / Vite / Tauri / TypeScript / Zustand / Tailwind / shadcn/ui

## 1. Purpose

本文档规定 Link World 前端的状态、组件、路由、数据访问和交互架构。目标是让三栏知识库、AI Analysis、Evaluation、后台任务、设置、插件和搜索在长期迭代中保持清晰边界。

前端必须：

- 严格 TypeScript。
- UI state 与 domain/server state 分离。
- Tauri command 调用集中在 hooks 或 container 层。
- 展示组件只通过 props 接收数据。
- 所有 async UI 都有 loading、empty、failed、retry 状态。
- 不直接访问数据库、文件系统或 secret。

## 2. Directory Structure

```text
src/
├── app/
│   ├── App.tsx
│   ├── routes.ts
│   └── providers.tsx
├── components/
│   ├── ui/                  # shadcn/ui primitives
│   ├── layout/              # shell, panes, resizable layout
│   ├── library/             # object list/detail
│   ├── analysis/            # AI Analysis presentation
│   ├── evaluation/          # Evaluation result presentation
│   ├── settings/            # settings panels
│   ├── recovery/            # startup recovery surfaces before normal AppState is available
│   └── shared/              # common presentational components
├── containers/              # data-aware components
├── hooks/
│   ├── commands/            # typed Tauri command wrappers
│   ├── library/
│   ├── search/
│   ├── jobs/
│   └── settings/
├── store/
│   ├── uiStore.ts
│   ├── libraryStore.ts
│   ├── searchStore.ts
│   ├── jobStore.ts
│   └── pluginStore.ts
├── types/
│   └── api.ts               # generated or copied from docs/api_contracts.ts
├── lib/
│   ├── tauri.ts
│   ├── errors.ts
│   ├── formatting.ts
│   └── cn.ts
└── styles/
    └── globals.css
```

Rules:

- `components/ui` 只放 shadcn primitives。
- `components/*` 默认无 side effects。
- `containers/*` 可以调用 hooks 和 store。
- `hooks/commands/*` 是唯一允许直接调用 `invoke` 的地方。
- `store/*` 不允许保存 API key、token、cookie、完整正文的长期副本。

## 3. State Management Topology

使用 Zustand，但必须分 slice。

### 3.1 UI state

`uiStore` 管理纯界面状态：

- sidebar collapsed。
- active pane。
- selected navigation item。
- active modal。
- command palette open。
- detail panel width。
- theme。

不得放入：

- KnowledgeObject 列表。
- parsed document 正文。
- API key。
- job payload。

### 3.2 Domain cache state

`libraryStore` 管理轻量 domain cache：

- current object id。
- current list filter。
- recent object list。
- object detail cache。
- optimistic deletion state。

Rules:

- 缓存必须有 stale 标记。
- 后端事件到达后标记对象 stale 或局部更新。
- 不长期缓存大量正文，详情页离开后可释放。

### 3.3 Search state

`searchStore` 管理：

- query。
- results。
- selected result index。
- loading/error。
- last submitted query。

Rules:

- 输入防抖。
- 空查询不调用后端。
- 搜索结果只保存必要 snippet 和 object metadata。
- Search command calls include the active Library filter when one is selected, so Sidebar filters and search results share backend semantics.
- Search maintenance UI may display index health counts and capped object-id samples, but not parsed text or snippets from the health check.
- Search empty state must be actionable: name the submitted query, suggest broader terms or clearing filters, and provide index check/rebuild actions.
- Search failure state must show the structured command error plus check/rebuild actions; it must not silently fall back to stale results.
- Search rebuild UI must display persisted backend progress and a cancel action only while `cancellable=true`. During `finalizing`, the UI must explain that the atomic swap cannot be cancelled.

### 3.4 Job state

`jobStore` 管理：

- objectId -> active jobs。
- jobId -> status。
- recent failures。
- retry pending。

Rules:

- job 详情来自后端，不在前端构造。
- job failed 必须展示 user-readable message 和 retry action。

### 3.5 Settings and plugins state

Provider 配置属于后端 server state，通过 `useModelProviderConfigs` 读取和修改。编辑草稿只存在于 `SettingsPanel` 的组件本地 state；对象详情不拥有 provider 表单或凭据状态。`pluginStore` 管理插件状态：

- provider config metadata，包括稳定 `id`、provider、API protocol、base URL、model、enabled、`hasApiKey` 和 `isDefault`。
- connection test result。
- plugin manifests。
- plugin permissions。

禁止：

- 存 API key 明文。
- 在 localStorage 持久化 secret。
- 回显完整 key。
- 后端配置加载完成后覆盖用户已经开始编辑的 draft。
- 配置字段变化后继续展示旧 connection test 的成功状态。

## 4. Server State and Command Hooks

Tauri command 调用必须封装。

```ts
export async function invokeCommand<TArgs, TResult>(
  command: string,
  args: TArgs,
): Promise<TResult> {
  const response = await invoke<IpcResponse<TResult>>(command, args);
  if (response.status === 'error') {
    throw toAppUiError(response.error);
  }
  return response.data as TResult;
}
```

Hook examples:

- `useSubmitCapture`
- `useStartupStatus`
- `useRestartApp`
- `useRecentObjects`
- `useObjectDetail`
- `useSearchHybrid`
- `useTriggerEvaluation`
- `useRetryEvaluation`
  - generates a new UUID per user retry, calls `retry_evaluation`, and never resets the failed parent run locally.
- `useObjectJobs`
- `useModelProviderConfig`
- `useModelProviderConfigs`
- `useLocalMetricsSnapshot`
  - owns `get_local_metrics_snapshot` state for Diagnostics; snapshot contains aggregate health and sanitized failed job summaries only.
- `useSupportBundleExport`
  - owns explicit-confirmation export state and the returned local summary/path; it never reads the generated JSON into React state or uploads it.
- `useBackups`
  - owns create/verify/prepare/restart command state and the last restore result; payload content never enters React state.
- startup hooks are owned by `App` / recovery surfaces; `get_startup_status` gates whether the normal Library shell may mount.
- `usePortableExport`
  - owns explicit `export_library` state; it stores only the returned summary/path and never loads exported object bodies back into React state.
- `restart_app` only schedules process restart and does not mutate storage.
- `usePluginPermissions`

Rules:

- Components never call `invoke` directly.
- Hooks translate `IpcErrorCode` to UI error type.
- Hooks decide retryability only from explicit error code, not message text.
- Hooks must support cancellation or stale response guards.
- Recovery-safe hooks must be explicitly documented; normal library hooks must not be called while `mode=recovery`.

## 5. Routing Strategy

MVP 使用 internal route state，不必立即引入 React Router。

Route model:

```ts
type AppRoute =
  | { name: 'library'; filter?: string; objectId?: string }
  | { name: 'search'; query?: string }
  | { name: 'settings'; panel?: 'models' | 'privacy' | 'capture' | 'plugins' | 'storage' | 'diagnostics' | 'about' }
  | { name: 'evaluation'; objectId: string; runId?: string };
```

Rules:

- Tauri desktop 不依赖 browser history。
- Startup recovery is not a normal route. `App` must query startup status first and avoid mounting `LibraryShellContainer` while `mode=recovery`.
- `StartupRecoveryScreen` may call only startup/backup/restore hooks that are explicitly recovery-safe.
- Recovery UI must not subscribe to library events, capture state, model provider settings, or object detail commands.
- `StorageSettings` can render in `startupRecovery` mode; it hides `create_backup` and keeps verify/restore actions visible.
- Browser preview may fall back to normal shell if Tauri runtime is unavailable; the desktop runtime must use `get_startup_status`.
- 如果未来引入 React Router，必须使用 `HashRouter`，避免本地文件路径刷新问题。
- Deep link / browser extension capture 进入应用后，只能转换为 route action，不直接改 domain store。
- Sidebar 分类通过 `library.filter` 驱动后端过滤；`inbox` / `failed` 是 lifecycle 过滤，其余值按 object type 过滤。
- Settings 是与 Library 同级的正式 route；凭据表单不得嵌入对象详情。

## 6. Container and Presentational Components

Container components:

- 调用 hooks。
- 读取/写入 stores。
- 处理 loading/error/retry。
- 组合 presentational components。

Presentational components:

- 只接收 props。
- 不调用 Tauri command。
- 不读取 global store。
- 不知道后端错误码。

Examples:

```text
LibraryShellContainer
  -> Sidebar
  -> ObjectListContainer
       -> ObjectList
       -> ObjectListItem
  -> ObjectDetailContainer
       -> ObjectDetail
       -> MarkdownDocumentView (lazy)
       -> AIAnalysisPanel
       -> EvaluationPanel
```

### 6.1 Document rendering pipeline

`parsed_documents.markdown_content` 是持久化展示格式；前端 AST 只在渲染时临时派生，不写入数据库或全局 store。

```text
Markdown
  -> unified + remark-parse + remark-gfm
  -> AST summary (TOC, structure metrics, inferred display mode)
  -> rehype-sanitize -> rehype-slug -> trusted Link World Callout plugin
  -> stable React component map
```

Rules:

- 禁止 `rehype-raw`，并保持 `skipHtml`，原始 HTML 不进入 React DOM。
- 固定项目插件在编译期注册，不提供运行时第三方渲染插件安装接口。
- 目录只收录 `h2-h4`，至少三个条目时显示；标题 ID 使用稳定 GitHub slug 规则。
- 相对链接和图片以对象 canonical URL 为基准解析；链接只允许 `http`、`https`、`mailto`，图片只允许 `http`、`https`。
- 图片必须使用 lazy loading、async decoding 和
o-referrer`。
- 超过 40 行的代码块支持折叠和复制；复制失败必须显示明确状态。
- 文档模式限定为 `article`、`tutorial`、`reference`、`code-heavy`，默认由 AST 结构确定。
- AST 推断顺序固定：代码字符占比至少 30% 或代码块至少 4 个为 `code-heavy`；表格至少 2 个，或至少 6 个且平均不足 600 字符的短章节为 `reference`；至少 2 个标题且有 4 个有序列表项为 `tutorial`；其余为 `article`。
- 只读取匹配当前 `parsedDocumentId` 的最新 AI analysis；`displayHints` 版本合法且 confidence `>= 0.75` 时才覆盖 AST 模式。
- AI 只能选择文档级展示模式，不能修改正文、AST、组件注册表或安全策略。

## 7. Three-Pane Layout

布局要求：

- 左侧 Sidebar 宽度稳定。
- 中间列表按 30 条分页并显式 Load more；后续可在不改变 command contract 的前提下替换为虚拟滚动。
- 右侧详情支持 loading、empty、failed、deleted。
- 三栏区域不得出现嵌套卡片堆叠。
- 工具按钮使用 lucide icons + tooltip。

Desktop first breakpoints:

- `>= 1200px`: 三栏完整显示。
- `900px - 1199px`: 左栏可折叠。
- `< 900px`: 详情页覆盖列表或使用单栏路由状态。

## 8. UI State Patterns

每个异步区域必须表达：

- `idle`
- `loading`
- `success`
- `empty`
- `failed`
- `stale`

Failure UI must show:

- 简短原因。
- 错误码或诊断 ID。
- retry action, if retryable。
- fallback action, e.g. browser capture。
- persisted `capture.*` failure reasons must be formatted through the shared capture failure formatter before rendering; user-facing surfaces show a clear title/recovery message and must not expose the raw stable prefix as primary copy。

禁止：

- 无限 spinner。
- 只显示 “Something went wrong”。
- 无法关闭的 modal。
- 操作后无反馈。

## 9. Error Handling

Frontend error model:

```ts
interface AppUiError {
  code: IpcErrorCode;
  title: string;
  message: string;
  retryable: boolean;
  action?: 'retry' | 'open_settings' | 'use_browser_capture' | 'view_logs';
}
```

Mapping examples:

| IpcErrorCode | UI action |
| --- | --- |
| `ERR_NETWORK_TIMEOUT` | retry |
| `ERR_PARSE_FAILED` | use browser capture |
| `ERR_MODEL_AUTH` | open model settings |
| `ERR_MODEL_RATE_LIMIT` | retry later |
| `ERR_POLICY_DENIED` | request explicit authorization |
| `ERR_PLUGIN_PERMISSION` | open plugin permissions |
| `ERR_SECRET_STORAGE` | open diagnostics |

## 10. Real-Time Updates

后端通过 Tauri event 通知前端：

- object lifecycle changed。
- analysis created。
- evaluation completed。
- job failed。
- object deleted。

Rules:

- Event payload 保持小，只传 id/status。
- 前端收到事件后重新拉取 detail 或局部更新。
- 不通过 event 传完整正文、API key、AI raw output。
- Event listener 必须在组件 unmount 时清理。

## 11. Forms and Validation

表单库不是当前边界要求；当前 Settings 使用受控本地 draft 和后端验证。表单复杂度提高后可引入 `react-hook-form + zod`，但不得复制后端安全策略。

Settings forms:

- model provider config。
- plugin permissions。
- storage path。
- export options。

- `StorageSettings` exposes portable export only in normal settings mode. It must stay hidden in `startupRecovery` mode.
- Export result display may show the export directory path returned by the backend, but components must not accept arbitrary destination paths from the user until a dedicated file-picker boundary exists.
Rules:

- API key 输入框支持 paste，但提交后不保留明文。
- 保存后清空 key draft；读取接口只返回 `hasApiKey`，不返回 masked key、明文或 `secretRef`。
- Base URL 必须校验 URL。
- Model name 不能为空。
- 多配置列表必须表达 enabled/default/credential-available；只能有一个默认 Chat 配置。
- 禁用当前默认项前必须先选择另一个默认项；删除默认项后 AI 显式变为未配置，不自动切换。
- 权限变更必须显示影响范围。

## 12. Accessibility

Minimum requirements:

- 所有 icon button 有 accessible label。
- Command palette 支持键盘导航。
- 列表项支持 arrow navigation。
- Modal 支持 focus trap。
- destructive action 有确认。
- Storage restore point 必须明确标注包含用户内容。
- Restore 是 destructive action：先展示内联确认，说明 safety backup 与重启边界；准备期间禁用并发恢复，失败时保留可读错误。
- AI confidence 不只靠颜色表达。

## 13. Performance

Rules:

- 大列表必须支持 pagination 或 virtualization。
- Markdown 渲染对大正文要 lazy。
- AST 分析必须按 Markdown 内容 memoize；动态展示策略通过稳定 Context 传递，组件映射不得在 render 内创建。
- Markdown/AST 依赖必须保留在独立 lazy chunk，不能进入应用主入口。
- 搜索输入 debounce 150-250ms。
- 避免把完整正文放入全局 store。
- AI / evaluation panels 可以独立 suspense/loading。
- expensive formatting 使用 memo，但不要滥用。

## 14. Visual System

产品风格：

- Quiet, dense, utilitarian。
- 面向重复使用和扫描，而不是营销页。
- 8px 或更小圆角，除非 shadcn 默认不同。
- 工具操作优先 icon button + tooltip。
- 避免大面积单色主题和装饰性渐变。
- 不使用可见说明文字解释功能，界面应通过结构和控件自解释。

## 15. Testing Requirements
- `StartupRecoveryScreen` / `StorageSettings(mode=startupRecovery)` hides create backup, surfaces verified backup id, and preserves explicit restore confirmation。

- `StorageSettings` portable export button calls `usePortableExport`, displays summary, and is hidden in startup recovery mode。
Minimum frontend tests:

- `ObjectList` renders lifecycle states。
- `ObjectDetail` renders parsed document, AI analysis, evaluation。
- `ObjectDetail` and capture submission UI render persisted `capture.*` failure reasons as user-facing recovery text, not raw backend prefixes。
- capture submission UI renders duplicate URL responses as an already-saved state and opens the existing object。
- AI analysis UI renders persisted `ai.*` failure reasons as user-facing recovery text, not raw backend prefixes。
- `MarkdownDocumentView` renders TOC, heading anchors, GFM tables, Callout and long-code controls。
- unsafe HTML/URL、纯文本 fallback、AI hint 失效和 clipboard fallback 必须有组件测试。
- `SearchCommand` handles loading, empty, failed, keyboard navigation。
- `ObjectList` handles search empty, search failed, rebuild progress and rebuild cancellation boundary states。
- `SettingsPanel` masks API key。
- `DiagnosticsSettings` renders local health, sanitized failed job summaries and normal model-configuration degradation；support-bundle export is disabled until the inline confirmation is checked, then shows only path/size/SHA-256 and never loads bundle content into React state。
- `PluginPermissionPanel` displays required vs optional permissions。
- `DeleteObjectDialog` confirms destructive action。

E2E scenarios:

- Add URL -> object appears -> parsed detail visible。
- Failed parse -> browser capture fallback action visible。
- Configure model -> AI analysis appears。
- Trigger evaluation -> evaluation verdict visible。
- Search -> open result。
- Delete object -> result disappears from search。

## 16. Frontend Implementation Checklist

Before adding a feature:

- Does it need a new IPC contract?
- Which store owns the state?
- Is it server/domain state or UI-only state?
- Which hook wraps the command?
- What are loading/empty/failed states?
- What is the keyboard path?
- Does it expose or store sensitive data?
- Does it need event subscription cleanup?
- Does it need a test fixture?
