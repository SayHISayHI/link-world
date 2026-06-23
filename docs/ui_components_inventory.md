# Link World UI 组件清单与规范

状态: Draft  
适用范围: React / shadcn/ui / Tailwind / Lucide

## 1. Purpose

本文档定义 MVP 到商业化版本的 UI 组件边界、组件职责和交互要求。目标是保证界面长期保持一致、可扫描、可维护，不因为功能增长变成零散组件堆。

## 2. Design Principles

- 工作台优先，不做营销页。
- 信息密度高但不拥挤。
- 操作按钮优先使用 Lucide icon + tooltip。
- 8px 或更小圆角，除非 shadcn 默认组件不同。
- 不用大面积渐变、装饰性背景、浮夸 hero。
- 不用可见说明文字解释功能；通过布局、状态和控件表达。
- 所有异步区域必须有 loading、empty、failed、retry。

## 3. Layout Components

| Component | Purpose | Data source | Notes |
| --- | --- | --- | --- |
| `AppShell` | App root layout | route/ui store | owns global layout only |
| `Sidebar` | navigation and settings entry | props | no command calls |
| `MainToolbar` | add/search/evaluation actions | container props | icon buttons with tooltip |
| `ThreePaneLayout` | sidebar/list/detail structure | ui store | stable dimensions |
| `ResizablePane` | pane resizing | ui store | persist only UI sizes |
| `CommandPalette` | global search/actions | search hooks | keyboard-first |

## 4. Library Components

| Component | Purpose | Required states |
| --- | --- | --- |
| `ObjectListContainer` | fetch and own list state | loading, empty, failed |
| `ObjectList` | render paged/filterable object rows | empty, loading more, end of page |
| `ObjectListItem` | object summary row | selected, failed, archived |
| `ObjectDetailContainer` | fetch detail | loading, failed, deleted |
| `ObjectDetailHeader` | title/source/actions | stale, failed |
| `MarkdownDocumentView` | lazy safe Markdown/AST reader | empty, plain-text fallback, AI/AST mode |
| `DocumentToc` | h2-h4 section navigation | hidden, collapsed, expanded |
| `CodeBlock` | fenced code, copy and long-content collapse | expanded, collapsed, copied, copy failed |
| `Callout` | trusted NOTE/TIP/IMPORTANT/WARNING/CAUTION presentation | regular quote, five callout kinds |
| `TagList` | render tags | empty |
| `SourceLinkButton` | open source URL | unavailable |

Document rendering rules:

- `MarkdownDocumentView` 必须通过 lazy import 加载，AST 不进入全局 store。
- 普通引用与显式 Callout 必须保持不同语义；不得用 AI 猜测段落级 Callout。
- AI 布局生效时显示低干扰来源标识，低置信度或旧文档提示不得改变布局。
- 目录、标题锚点、代码操作和折叠控件必须支持键盘与 accessible name。
- renderer 不渲染原始 HTML，不允许危险 URL 协议，也不允许 AI 绕过安全组件。

## 5. AI and Evaluation Components

| Component | Purpose | Required data |
| --- | --- | --- |
| `AIAnalysisPanel` | run action, summary and trace metadata; link to model settings | `AIAnalysis` |
| `AITracePopover` | provider/model/time/cost | `AITrace` |
| `QualityScoreBadge` | 0-10 score | score + confidence |
| `RiskList` | risk display | risk type/severity/detail |
| `ActionItemsList` | next actions | action title/tools |
| `EvaluationPanel` | verdict, dimensions, evidence | `EvaluationRun` |
| `EvaluationVerdictBadge` | verdict display | verdict |
| `DimensionRadar` | optional dimension visualization | dimensions |
| `EvidenceList` | evaluator evidence | evidence items |
| `ArtifactList` | logs/screenshots/reports | artifacts |

Rules:

- AI panel must label model inference separately from original facts.
- Evaluation panel must show limitations when present.
- Scores must not rely only on color.
- Trace details can be collapsed but must be accessible.

## 6. Capture Components

| Component | Purpose | Required states |
| --- | --- | --- |
| `AddUrlDialog` | submit URL | idle, submitting, failed |
| `CaptureDropZone` | future file import | idle, dragging, rejected |
| `SelectionCapturePreview` | browser extension payload preview | ready, failed |
| `CaptureStatusToast` | capture acknowledgement | queued, failed |

Rules:

- Capture acknowledgement should be fast and not wait for parsing.
- Failed capture must show fallback action if browser extension can help.

## 7. Search Components

| Component | Purpose |
| --- | --- |
| `SearchInput` | query input |
| `SearchResultsList` | result list |
| `SearchResultItem` | result row with snippet |
| `SearchEmptyState` | no results |
| `SearchErrorState` | failed query |

Rules:

- Debounce query 150-250ms.
- Empty query does not call backend.
- Keyboard navigation required.
- Result snippets must not reveal secret content.

## 8. Settings Components

| Component | Purpose |
| --- | --- |
| `SettingsPanel` | formal settings route, section navigation and milestone boundaries |
| `ModelSettings` | provider list, create/edit/delete/default, protocol, base URL, model and connection test |
| `StorageSettings` | create/list/verify local restore points and communicate content boundary |
| `SettingsBoundary` | explicit placeholder for privacy/capture/plugins/diagnostics/about until implemented |

Rules:

- API key never displayed in full after save；读取只显示 credential available / no credential。
- 模型凭据只能在 Settings 编辑；对象详情只显示运行、结果和进入 Settings 的动作。
- 只有 enabled 且支持 chat 的配置可以成为默认项；UI 不提供隐式 failover。
- Provider 允许常见预设和自定义标识；protocol 必须显式可见，不能从品牌名隐式猜测后隐藏。
- Base URL 输入 API 根路径，不包含具体 operation endpoint。
- Provider test result must distinguish auth, network, model name and schema errors.
- Plugin permissions show required vs optional.
- Sensitive permissions require explicit confirmation.
- Storage backup UI must state that restore points contain user content, never claim encryption, and must not expose Restore before rollback is implemented.

## 9. Feedback Components

| Component | Purpose |
| --- | --- |
| `InlineError` | localized section error |
| `RetryButton` | retryable action |
| `EmptyState` | empty area |
| `LoadingSkeleton` | loading state |
| `StatusBadge` | lifecycle/job status |
| `ConfirmDialog` | destructive action |
| `Toast` | short non-blocking feedback |

Rules:

- No infinite spinner without text.
- Error must include next action where possible.
- Destructive action requires confirm or undo.

## 10. Component Quality Rules

- Presentational components receive props only.
- Components do not call `invoke` directly.
- Components do not import repositories, filesystem APIs or secret utilities.
- Components must handle long titles and long URLs.
- Text must not overflow buttons or badges.
- Icon-only buttons require accessible label and tooltip.
- All lists must support empty state.
- All cards/panels must avoid nested card visual style.

## 11. Story/Test Coverage

Each major component should have fixtures for:

- normal data.
- empty data.
- long text.
- failed state.
- sensitive object.
- no AI configured.
- AI output with low confidence.
- evaluation unsafe verdict.

## 12. MVP Component Set

MVP must implement:

- `AppShell`
- `Sidebar`
- `ThreePaneLayout`
- `ObjectListContainer`
- `ObjectList`（包括后端 filter、30 条分页和 Load more）
- `ObjectListItem`
- `ObjectDetailContainer`
- `MarkdownDocumentView`
- `DocumentToc`
- `CodeBlock`
- `Callout`
- `AIAnalysisPanel`
- `EvaluationPanel`
- `AddUrlDialog`
- `SearchInput`
- `SearchResultsList`
- `SettingsPanel`
- `ModelSettings`
- `SettingsBoundary`
- `DiagnosticsPanel`
