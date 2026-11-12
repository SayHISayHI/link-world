# Tauri IPC 契约

状态: Current

## 1. 权威来源

**类型定义的唯一真源是 `src/types/api.ts`。** 本文只记录约定、命令清单和错误码，不再复制类型声明——历史上 `docs/api_contracts.ts` 曾是第二份类型副本，导致每次改契约都要改两处、且两边经常漂移（已在清理中删除）。

规则：

- 新增/修改 DTO：先改 `src/types/api.ts` 和 Rust `domain/` 模块，两边字段名必须一致。
- 新增 command：必须在 `src-tauri/src/lib.rs` 的 `generate_handler!` 注册，并在本文命令清单登记。
- 本文与代码冲突时，以代码为准，并在同一个提交里回写本文。

## 2. 通用信封

所有 command 返回 `IpcResponse<T>`（`src-tauri/src/errors.rs`）：

```ts
type IpcResponse<T> =
  | { status: "success"; data: T }
  | { status: "error"; error: { code: IpcErrorCode; message: string; details?: string } };
```

- 成功必须带 `data`；前端 `invokeCommand` 对缺失 `data` 视为错误。
- 失败只暴露稳定 `code` 和脱敏后的 `message`；原始错误、路径和正文不得进入 `message` / `details`。
- Rust 侧一律 `#[serde(rename_all = "camelCase")]`；前端类型使用 camelCase。
- 前端禁止直接 `import { invoke }`，统一走 `src/lib/tauri.ts` 的 `invokeCommand`，以便把传输层异常映射成 `AppUiError`。

## 3. 命令清单

共 52 个 command，按模块分组（与 `src-tauri/src/commands/` 一一对应）：

### system（4）

`get_startup_status` `restart_app` `ping` `get_local_metrics_snapshot`

### capture（1）

`submit_capture`

### library（3）

`get_recent_objects` `get_object_detail` `delete_object`

### organization（14）

`get_library_navigation` `list_library_objects` `create_collection` `create_smart_view` `update_collection` `archive_collection` `add_object_to_collection` `remove_object_from_collection` `mark_object_triaged` `add_user_tag` `remove_object_tag` `accept_tag_suggestion` `reject_tag_suggestion` `get_object_organization`

### search（7）

`search_hybrid` `search_library` `check_search_index` `rebuild_search_index` `get_search_index_rebuild_status` `cancel_search_index_rebuild` `reindex_object`

### ai（8）

`get_model_provider_config` `list_model_provider_configs` `save_model_provider_config` `delete_model_provider_config` `set_default_model_provider` `test_model_provider_config` `update_model_provider_config` `trigger_ai_enrichment`

### evaluation（4）

`trigger_evaluation` `retry_evaluation` `list_evaluator_capabilities` `get_evaluation_run`

### operations（3）

`get_background_job` `get_object_jobs` `retry_background_job`

### backup（6）

`create_backup` `list_backups` `prepare_restore` `get_restore_status` `restart_to_apply_restore` `verify_backup`

### diagnostics（1）

`export_support_bundle`

### portable_export（1）

`export_library`

## 4. 错误码

`IpcErrorCode`（`src-tauri/src/errors.rs`）是稳定契约，前端按 code 分支，不解析 message：

| code | 语义 | 用户可采取的典型动作 |
| --- | --- | --- |
| `ERR_BACKUP_INVALID` | 备份文件/清单损坏或不可信 | 换一个备份或重新创建 |
| `ERR_RESTORE_INVALID` | 恢复候选不合法 | 检查备份来源 |
| `ERR_RUNTIME_BUSY` | 另一个进程持有数据目录锁 | 关闭其他实例或 CLI |
| `ERR_DB_CONSTRAINT` | 唯一/外键约束冲突 | 重命名或删除冲突项 |
| `ERR_DB_MIGRATION` | 迁移失败，已 fail-closed | 使用备份恢复并上报 |
| `ERR_NETWORK_TIMEOUT` | 外部请求超时 | 重试或切换网络/模型 |
| `ERR_PARSE_FAILED` | 抓取后解析失败 | 重试或改用浏览器扩展采集 |
| `ERR_MODEL_AUTH` | 模型凭据无效 | 更新 API key |
| `ERR_MODEL_RATE_LIMIT` | 模型限流 | 等待或切换 provider |
| `ERR_MODEL_NOT_FOUND` | 模型不存在或无权限 | 检查模型名 |
| `ERR_MODEL_OUTPUT_SCHEMA` | 模型输出不符合 schema | 重试或换模型 |
| `ERR_POLICY_DENIED` | 隐私/策略 gate 拒绝 | 调整隐私级别或改用本地模型 |
| `ERR_PLUGIN_PERMISSION` | 插件权限不足 | 预留码，插件运行时尚未实现 |
| `ERR_JOB_NOT_FOUND` | 任务不存在或已清理 | 刷新任务列表 |
| `ERR_OBJECT_NOT_FOUND` | 对象不存在或已删除 | 返回列表 |
| `ERR_SECRET_STORAGE` | 凭据后端不可用 | Windows 检查凭据管理器；非 Windows 为开发后端 |
| `ERR_UNKNOWN` | 未分类失败 | 导出支持包 |

## 5. 维护检查表

- 新增 command → 注册 `generate_handler!`、登记第 3 节、加 Rust 测试与前端 hook。
- 新增错误 → 在 `AppError` 和 `IpcErrorCode` 双向映射，并补第 4 节一行。
- 删除 legacy command（`get_recent_objects`、`search_hybrid`）时，同步删除第 3 节的标注和 CLI 兼容引用。
