# Link World 运营就绪与发布标准

状态: Draft  
目标: 定义从 MVP 到商业化版本必须满足的工程运营标准。

## 1. Release Readiness

发布前必须检查：

- 数据库 migration 能从空库启动。
- 数据库 migration 能从上一版本升级。
- 应用可以在无网络环境启动并浏览已有内容。
- 未配置模型时，保存、解析、搜索仍可用。
- 配置模型后，AI enrich 可以成功并写入 trace。
- 删除对象后，FTS、vector、object store 和 AI 派生产物被清理。
- Windows 安装包可以安装、启动、卸载。

## 2. Environment Matrix

MVP 主支持：

- Windows 11。
- Tauri v2。
- SQLite + FTS5。
- OpenAI Chat Completions/Responses、Anthropic Messages、Google Generative AI、OpenAI-compatible 和 Ollama Chat provider。

兼容目标：

- Windows 10。
- macOS。
- Linux。
- Ollama / LM Studio。
- sqlite-vec semantic search。

## 3. Observability

### 3.1 Local diagnostics

设置页必须能展示：

- 应用版本。
- 数据库路径。
- 对象存储路径。
- 数据库大小。
- 对象存储大小。
- 后台任务队列深度。
- 最近失败任务。
- 模型 provider 连接测试结果。
- 当前 provider、API protocol、base URL、model 与凭据是否已配置；不得显示 secret 内容。
- 插件列表和权限状态。

### 3.2 Logs

日志必须结构化：

- timestamp
- level
- module
- event
- object_id
- job_id
- error_code
- message

日志不得包含：

- API key。
- token。
- cookie。
- session。
- 完整正文。
- secret / sensitive 内容。

## 4. Backup and Recovery

Local Edition 必须支持：

- 手动导出全库。
- 数据库迁移前创建 restore point。
- 对象存储路径可定位。
- 检测数据库损坏并给出用户可理解提示。

建议恢复策略：

- SQLite `.backup` 或复制前 checkpoint WAL。
- 导出 JSONL + objects manifest。
- 校验 source snapshot content hash。

## 5. Migration Policy

迁移规则：

- 每个 migration 必须有版本号和描述。
- 破坏性 migration 必须先创建备份。
- 失败后停止启动并给出恢复入口。
- 不允许静默删除用户数据。
- 旧 AI analysis 和 evaluation result 不因 schema 升级被覆盖。

## 6. Incident Playbooks

### 6.1 AI provider failure

处理：

- 将 job 标记为 `failed` 或 `blocked`。
- 保留对象在 `parsed` 状态。
- 提示用户检查 key、base URL、余额或模型名称。
- 不重试鉴权失败。

### 6.2 Parser regression

处理：

- 标记 parser version。
- 新 parser 发布后允许用户手动 reparse。
- 旧 `parsed_documents` 保留，不覆盖。

### 6.3 Database migration failure

处理：

- 停止后续写入。
- 展示错误和备份路径。
- 提供恢复上一版本数据库的说明。

### 6.4 Plugin misbehavior

处理：

- 自动禁用连续失败插件。
- 记录 audit log。
- 展示插件权限和最近错误。
- 提供撤销权限和卸载入口。

## 7. Performance Regression Gates

发布前采样验证：

- 1000 个对象列表和搜索。
- 5000 个对象列表滚动。
- 100 个 failed jobs 的诊断页。
- 50MB 对象存储下启动。
- 10 个并发 URL capture jobs。

任何明显卡顿必须进入发布阻断项。

## 8. Supportability

用户反馈问题时，应用应能生成本地诊断包。

诊断包包含：

- 应用版本。
- OS 版本。
- schema version。
- 最近错误码。
- 任务状态摘要。
- 插件列表。
- 已脱敏日志。

诊断包不得包含：

- API key。
- token。
- cookie。
- session。
- 正文。
- source snapshots。
- embeddings。
