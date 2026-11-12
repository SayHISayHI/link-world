# Storage ABI 冻结清单

状态: Current

产品在首次外部 Alpha 之前由 Link World 更名为 Node Tide（ADR-0010）。UI、包名、二进制和文档都改了，但**磁盘上的标识没有改**——因为改了等于老用户数据"消失"。

本文列出所有不可随意重命名的持久化标识，以及真要改时的步骤。

## 1. 冻结标识

| 标识 | 值 | 位置 | 影响面 |
| --- | --- | --- | --- |
| 应用数据目录 | `com.linkworld.app` | `src-tauri/tauri.conf.json` 的 `identifier`；`src-tauri/src/legacy_abi.rs` 的 `APP_DATA_DIR_IDENTIFIER`（CLI 与测试共用） | 整个数据目录路径。改名 = 老库、对象存储、日志全部"找不到" |
| 数据库文件 | `link-world.sqlite3` | `legacy_abi::DATABASE_FILE_NAME` | 库文件本身；还派生 `-wal` / `-shm` 同名 sidecar |
| 结构化日志 | `link-world.jsonl` / `link-world.jsonl.1` | `legacy_abi::LOG_FILE_NAME` / `ROTATED_LOG_FILE_NAME` | 支持包与诊断读取的日志流，改名会切断历史日志 |
| 运行时锁 | `link-world.lock` | `legacy_abi::RUNTIME_LOCK_FILE_NAME` | 单进程互斥；残留文件本身无害（flock 语义），但改名会短暂允许双开 |
| 凭据服务名 | `com.linkworld.app.model-provider` | `legacy_abi::KEYRING_SERVICE` | OS 凭据管理器里的模型 API key 条目 |
| 前端偏好 | `link-world-ui` | `src/config/storageKeys.ts` 的 `LEGACY_UI_STORAGE_KEY` | 语言、主题、布局偏好 |
| 数据目录名（文档/测试样例） | `NodeTide` 等仅出现在测试 fixture | 测试代码 | 只影响测试，可随时调整 |

## 2. 为什么保留

- **数据优先于命名一致性**：改名不会带来功能收益，却会造成数据丢失或需要一次高风险迁移。
- **可信度**：`docs/product_requirements.md` 承诺"备份可验证、恢复不丢数据"，改名会直接违背这条承诺。
- **成本不对称**：保留旧名只在代码里多一层注释；改名失败是 P0 数据事故。

## 3. 真要改名时的步骤

1. 在 `src-tauri/migrations/` 之外新建一次性迁移流程（不能是 SQL migration，因为涉及目录和凭据）。
2. 启动时检测新名字不存在、旧名字存在 → 复制（不是移动）到新路径，校验 SQLite `quick_check` 与对象存储哈希。
3. 保留旧路径只读至少一个发布周期，并在下一版删除。
4. 凭据迁移：读取旧服务名 → 写入新服务名 → 验证读回 → 删除旧条目；失败时保留旧条目并告警。
5. 前端偏好迁移：`persist` 的 `migrate` 钩子读取旧 key 后写新 key。
6. 更新本文、ADR-0010 和 `docs/backup_and_restore.md` 的兼容说明，并补充回滚验证。

## 4. 现状

- 所有标识集中在 `src-tauri/src/legacy_abi.rs` 与 `src/config/storageKeys.ts`，其他文件只引用常量。
- 新增持久化标识时，先判断是否属于本清单；属于就必须走上面的流程，而不是"先改名字以后再说"。
