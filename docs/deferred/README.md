# Deferred designs

本目录存放**保留但尚未实现**的设计。它们记录当初的思路与边界，不是当前行为依据：

- 不得据此认为功能已存在；
- 实现相关能力前，必须先把对应文档升级回 `docs/` 并按 ADR 流程更新；
- 与代码冲突时，以 `docs/` 下的当前规范和代码为准。

| 文档 | 内容 | 状态 |
| --- | --- | --- |
| [sync_architecture.md](./sync_architecture.md) | 多端同步、冲突合并、tombstone、隐私同步边界 | 未实现。当前只有本地单设备；`plugin_manifests` 里的 `sync_provider` 是保留枚举值 |
| [plugin_development_guide.md](./plugin_development_guide.md) | Connector / Parser / Evaluator / Prompt 资源与 Registry 插件规范 | 未实现。`0001` 迁移已建 `plugin_manifests` / `plugin_permissions` 空表，运行时缺失 |

保留的契约残件：`AppError::PluginPermission` 与 `ERR_PLUGIN_PERMISSION` 仍定义在 `src-tauri/src/errors.rs`，但没有任何代码路径产生它——这是给未来插件运行时预留的稳定错误码，不要当成已实现能力。

相关但仍在使用的参考资产（没有代码引用，属于离线资料）：

- `evals/dataset/` — prompt / repo evaluator 的确定性基准样本
- `prompts/system_prompts/evaluate_repo.md` — repo evaluator 的 prompt 草稿；线上 prompt 目前内嵌在 `src-tauri/src/services/ai.rs`

如果确认这些资产不再需要，删除时请同步移除 `docs/testing_strategy.md` 和相关指南里的引用。
