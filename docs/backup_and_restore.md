# 本地备份与恢复规范

状态：Restore lifecycle、启动迁移保护、启动恢复界面与便携导出已实现；Sprint 2 readiness 自动化门禁已建立；真实 Windows 故障矩阵待回归
当前范围：创建、列出、验证、预检、重启恢复、失败回滚、迁移前 restore point、启动失败 recovery UI 和 Markdown/JSON 便携导出
尚未实现：自动保留策略、真实安装升级/进程故障矩阵执行证据

## 1. 目标

本地备份用于在升级、迁移或用户误操作前创建可验证的同机恢复点。它必须同时覆盖 SQLite 和对象存储，不能只复制主数据库文件，也不能把 WAL 状态不一致的文件暴露为成功备份。

当前实现保证：

- SQLite 通过 VACUUM INTO 生成一致性快照。
- 对象存储逐文件复制并计算 SHA-256。
- manifest 使用 schema version，记录应用版本、创建时间、数据库条目和对象文件条目。
- manifest 原始字节另有 SHA-256 sidecar。
- 所有内容先写入隐藏 staging 目录，完成后在同一文件系统内 rename 为正式目录。
- 验证会检查 manifest hash、路径安全、文件大小、文件 hash、额外文件和 SQLite quick_check。
- 创建失败时清理本次 staging 目录，不生成正式 restore point。

## 2. 明确边界

本地 restore point 与便携导出不是同一产品能力。

本地 restore point：

- 目标是同一安装环境的无损恢复。
- 包含数据库和对象存储中的用户内容，包括 personal、sensitive 和 secret 等级对象。
- SQLite 中可能保留不可解密的 credential reference，但不包含 API Key 值。
- API Key 值仍只存在于 Windows Credential Manager。
- 当前备份目录位于应用数据目录下，依赖 Windows 用户目录权限，不宣称端到端加密。
- Storage 设置页提供显式二次确认的 Restore；在线进程只准备候选和 safety backup，数据替换只发生在重启后的数据库连接池建立之前。

便携导出：

- 已实现为 `export_library`，后端只写入应用数据目录下的 `exports/<export-id>/`，不接受客户端传入任意路径。
- 默认排除 secret 内容、credential reference、内部任务记录和本机对象存储路径；当前包含 public/personal/sensitive 对象，后续如允许 secret/sensitive 细粒度选择必须增加显式确认。
- 输出 `manifest.json`、`manifest.sha256`、`objects.jsonl`，以及每个对象的 `metadata.json` 和可读 `document.md`。
- 不复用原始 SQLite restore point 作为对外分享格式，也不读取 Windows Credential Manager。
- startup recovery 模式禁用便携导出，避免在不确定 live 状态下导出普通库。

## 3. 目录布局

应用数据目录下使用以下结构：

    backups/
      <backup-id>/
        database.sqlite3
        objects/
          <object-id>/...
        manifest.json
        manifest.sha256
      .staging-<backup-id>/

backup-id 由后端生成 UUID。客户端不能指定备份路径或 backup-id。

staging 目录不属于可恢复备份：

- list_backups 忽略所有 .staging- 前缀目录。
- create_backup 只在 manifest、sidecar 和全部 payload 写入成功后 rename。
- 应用异常终止可能留下 staging 目录；后续将增加基于年龄的安全清理。

## 4. Manifest v1

manifest.json 包含：

| 字段 | 含义 |
| --- | --- |
| schemaVersion | 当前固定为 1 |
| backupId | 与父目录一致的 UUID |
| appVersion | 创建备份的 Link World 版本 |
| createdAt | RFC 3339 时间 |
| database | SQLite 相对路径、字节数和 SHA-256 |
| objects | 排序后的对象文件相对路径、字节数和 SHA-256 |
| totalSizeBytes | database 与 objects payload 总字节数 |
| containsUserContent | 固定为 true |
| credentialsIncluded | 固定为 false，指 credential value |

manifest.sha256 是 manifest.json 精确字节的 SHA-256。修改 JSON 的空白也会导致校验失败。

所有 manifest path 必须满足：

- 相对路径。
- 只包含 normal path component。
- 不允许绝对路径、父目录、当前目录或平台前缀。
- database path 必须严格为 database.sqlite3。
- object path 必须以 objects/ 开头。
- 重复 object path 视为无效。
- Windows 路径段禁止冒号、尾随点和尾随空格，避免 ADS 与规范化歧义。
- manifest 最大 16 MiB，hash sidecar 最大 256 bytes，对象目录深度最大 32。

## 5. 创建流程

1. 在 backups 下创建唯一 staging 目录。
2. 对在线数据库执行 VACUUM INTO staging/database.sqlite3。
3. 枚举对象存储中的普通文件。
4. 遇到 symlink、reparse link 或不支持的文件类型时终止。
5. 按原相对路径复制文件；复制过程中流式计算 SHA-256。
6. 对数据库快照计算 SHA-256。
7. 写入 manifest.json 并 fsync 文件。
8. 写入 manifest.sha256 并 fsync 文件。
9. 将 staging 目录 rename 为 backup-id 目录。
10. 返回 BackupSummary。

备份不暂停整个应用。SQLite 快照在数据库层保持一致；对象存储当前采用逐文件复制，因此本阶段要求写入对象文件后不再原地修改。未来如果允许原地改写对象文件，必须增加对象存储 generation 或写入锁。

## 6. 验证流程

verify_backup 进行：

1. 校验 backup-id，禁止路径注入。
2. 校验 manifest sidecar。
3. 解析并检查 manifest schema 与目录 identity。
4. 校验 totalSizeBytes。
5. 校验每个 payload 的存在性、大小和 SHA-256。
6. 枚举 objects 目录并报告 manifest 未声明的额外文件。
7. 以只读模式打开 SQLite 快照并执行 PRAGMA quick_check。

内容损坏作为 BackupVerification 返回，包含 valid=false 和 issues；无效 backup-id 或不存在的备份使用 ERR_BACKUP_INVALID。

## 7. 两阶段恢复协议

恢复不在持有在线 SQLite pool 的进程中替换文件。实现分为 prepare 与 restart/apply 两个阶段。

### 7.1 Prepare 阶段

用户在 Storage 设置页二次确认后，prepare_restore 顺序执行：

1. 重新验证目标 restore point 的 manifest、payload hash 和 SQLite quick_check。
2. 为当前在线数据创建一个独立 safety restore point。
3. 把目标数据库和对象文件复制到 restore/candidate-<transaction-id>。
4. 在候选数据库副本上运行当前全部 migration。
5. 执行 PRAGMA quick_check、foreign_key_check 和 WAL checkpoint。
6. 按迁移后的数据库重新生成候选 manifest 与 SHA-256 sidecar。
7. 再次完整验证候选目录。
8. 使用 create_new 写入 pending.prepared.json。
9. 返回 RestorePreparation；前端随后调用专用重启命令。

任一步失败时不写 pending marker，不触碰在线数据；已经创建的 safety restore point 保留，失败候选目录被清理。

prepare.lock 使用独占创建，阻止并发准备两个恢复事务；应用启动、命令尚未开放时会清理上次崩溃遗留的 stale lock。客户端只能提交 backupId，不能提交路径、transactionId、candidate path 或 rollback path。

### 7.2 Restart / Apply 阶段

应用重启后，在 Database::initialize 和后台服务启动之前读取 pending marker。控制目录只允许以下阶段文件：

| 阶段文件 | 含义 |
| --- | --- |
| pending.prepared.json | 候选已迁移并完整验证，尚未移动在线数据 |
| pending.moving-live.json | 正在把在线数据库、WAL/SHM 和 objects 移入 rollback |
| pending.live-moved.json | 在线 payload 已完整移入 rollback，准备安装候选 |
| pending.candidate-installed.json | 候选已安装，等待数据库初始化和最终完整性检查 |

阶段变化使用同目录 rename。数据库和 objects 是两个独立路径，因此不宣称底层存在单次文件系统原子操作；phase marker 与启动恢复逻辑保证用户可观察状态最终收敛为完整旧版本或完整恢复版本。

apply 流程：

1. 再次校验候选 manifest hash、payload 和 SQLite。
2. 把 prepared marker rename 为 moving-live。
3. 把在线 link-world.sqlite3、存在的 WAL/SHM 和 objects rename 到 restore/rollback-<transaction-id>。
4. 把 marker 推进为 live-moved。
5. 把候选 database.sqlite3 和 objects rename 到在线位置。
6. 把 marker 推进为 candidate-installed。
7. 初始化 SQLite、运行 migration，并再次执行 quick_check 与 foreign_key_check。
8. 初始化 ObjectStore。
9. 删除 rollback、candidate 和 pending marker，写入 succeeded 的 last-result.json。

### 7.3 自动回滚

- prepared 候选在启动校验失败：删除候选与 marker，在线数据保持不变，结果为 failed。
- 进程在 moving-live 或 live-moved 阶段中断：下次启动根据 rollback 目录恢复已移动的旧 payload，结果为 rolled_back。
- 候选安装后数据库初始化、migration、quick_check、foreign_key_check 或 ObjectStore 初始化失败：先关闭新 pool，再恢复 rollback 数据并重新初始化旧数据。
- Windows restore 文件替换、删除和 rename 对短暂文件锁执行有界重试，覆盖 SQLite handle、Defender 或索引器导致的 transient `os error 32/33`。
- 可选 WAL/SHM 按实际阶段处理；moving-live 中尚未移动的旧 sidecar 必须保留，不能当作候选 sidecar 删除。
- 自动回滚成功后应用继续使用旧数据启动；若 rollback payload 本身缺失，启动失败而不是在不一致数据上继续运行。
- safety restore point 不随事务清理，可用于人工恢复和问题调查。

恢复结果只包含 backupId、safetyBackupId、状态、完成时间和脱敏错误摘要，不返回正文、文件清单或本机绝对路径。

### 7.4 控制目录

    restore/
      prepare.lock
      pending.<phase>.json
      candidate-<transaction-id>/
        database.sqlite3
        objects/
        manifest.json
        manifest.sha256
      rollback-<transaction-id>/
        link-world.sqlite3
        link-world.sqlite3-wal
        link-world.sqlite3-shm
        objects/
      last-result.json

控制文件上限为 64 KiB。backupId、safetyBackupId 和 transactionId 均按后端生成的受限标识符校验；所有 candidate/rollback 路径由后端在 app data 目录内派生。

### 7.5 启动迁移保护

普通启动与用户主动 Restore 是两条独立协议。`MigrationService` 在生产 `AppState` 打开 SQLite、但尚未运行 migration 和启动后台服务时执行：

1. 检查正式 migration 版本、成功标记和 checksum。
2. fresh DB 直接迁移；已有用户 schema 且存在 pending migration 时，先创建并完整验证普通 restore point。
3. 生成 correlation UUID 并写入 `migration/guard.prepared.json`，随后原子 rename 为 `guard.running.json`，再执行 migration 与数据库完整性校验；成功后同一 UUID 写入 `last-result.json`。
4. 成功后清除 guard、写入脱敏 `last-result.json`，并保留 restore point。

`guard.running.json` 与 pending migration 同时存在时，启动会阻止自动重试并返回 verified backup ID；migration 已提交但 guard 尚未清理时，下次启动会验证完整性并完成清理。guard/result 中的 UUID 关联 `migration.started/prepared/running/succeeded/failed`；legacy guard 使用原 UUID backup id，损坏 guard 或 plan 读取失败只写稳定 `migration.*` code。结构化日志不复制新 guard 的 backup ID、控制文件内容、绝对路径或 raw database error；legacy guard 的 UUID backup id 只允许作为 `correlationId` 复用。该协议不会自动替换 live storage，也不复用两阶段 Restore 的 candidate/rollback 目录。控制文件上限同样为 64 KiB，且不包含正文、凭据或绝对路径。


### 7.6 启动恢复界面

`AppState::initialize_from_data_dir` 失败时，Tauri 进程不直接退出窗口；`lib.rs` 注册 `StartupState::Recovery` 并跳过普通 `AppState`、capture loopback server 和后台 AI/capture 服务。前端 `App` 先调用 `get_startup_status`：

- `mode=ready` 时挂载普通 Library shell。
- `mode=recovery` 时挂载 `StartupRecoveryScreen`，只展示脱敏错误、错误码、可用 verified backup ID、migration guard 元数据和 Storage recovery 操作。

Recovery UI 的后端命令边界：

- `list_backups`、`verify_backup`、`get_restore_status` 和 `restart_to_apply_restore` 不依赖完整 `AppState`，可在 recovery 模式读取 app data 下的控制目录和正式 backups。
- `prepare_restore` 在 recovery 模式下临时连接 live SQLite，但不会运行普通 startup migration；它仍会重新验证目标、创建 safety backup、迁移私有候选并写 pending marker。若 live DB 已不可读，命令 fail closed，不会直接替换数据。
- `create_backup` 在 recovery 模式下禁用，避免把不确定 live 状态标记为新的安全 restore point。
- `restart_app` 仅安排进程重启，用于用户修复外部环境问题后重试启动；它不修改 storage。

该界面仍不暴露本机绝对路径、正文、manifest 文件清单或 credential value。用户必须显式点击 Restore 并通过内联确认后，才会准备恢复并重启应用。

### 7.7 便携导出

便携导出由 `PortableExportService` 执行，和同机 restore point 完全分离：

- `export_library` 只在普通 ready 模式可用；startup recovery 模式返回 policy denied。
- 客户端不能指定输出路径。后端在 app data 下创建 `exports/<export-id>.staging/`，完成后 rename 为 `exports/<export-id>/`。
- `manifest.json` 记录 schema version、app version、创建时间、对象数量、跳过的 secret 数量、每个对象的 metadata/document 相对路径和内容 hash。
- `objects.jsonl` 提供机器可读的逐对象 metadata；每个对象目录提供 `metadata.json` 和 `document.md`。
- `metadata.json` 保留对象 metadata、parsed document metadata、AI analysis/trace 摘要、evaluation verdict/evidence/limitation；不包含 source snapshot `storage_uri`、evaluation artifact `storage_uri`、credential reference、后台 job、SQLite 内部记录或本机绝对路径。
- `document.md` 使用解析后的 markdown，缺失时回退纯文本，并添加来源、捕获时间和导出时间等可读头部信息。
- secret 对象默认跳过并计入 `skippedSecretCount`；导出结果不会把 secret 正文写入 manifest、JSONL 或 markdown。

便携导出用于用户可读、可搬迁的 Markdown/JSON 副本；它不是安全备份，也不能用于自动 restore。

## 8. API 契约

- get_startup_status：返回 ready/recovery 模式、后端版本、脱敏启动错误和 migration guard 元数据。
- restart_app：安排应用重启；不修改 storage。
- create_backup：创建 restore point，返回 BackupSummary；startup recovery 模式禁用。
- list_backups：按创建时间倒序返回 summary；损坏 manifest 以 status=invalid 暴露；startup recovery 模式可用。
- verify_backup：执行完整 hash 与 SQLite 校验，返回 BackupVerification；startup recovery 模式可用。
- prepare_restore：重新验证、创建 safety backup、迁移并验证候选，返回 RestorePreparation；startup recovery 模式可用但要求 live DB 可临时连接。
- restart_to_apply_restore：仅在 pending restore 存在时安排应用重启；startup recovery 模式可用。
- get_restore_status：返回最近一次 succeeded、rolled_back 或 failed 结果；没有结果时返回 null；startup recovery 模式可用。

- export_library：导出全库非 secret 对象到 Markdown/JSON 目录，返回 PortableExportSummary；不接受路径参数，startup recovery 模式禁用。

命令不得：

- 接收任意文件系统路径。
- backup/restore/recovery 命令不得返回用户正文或 manifest 文件列表；`export_library` 只返回 summary，正文只写入用户显式触发的导出目录。
- 读取或导出 Windows Credential Manager 中的值。
- 自动删除旧备份或 safety backup。
- 未经用户明确确认在后台准备或应用恢复。
- 在在线 SQLite pool 存活时替换数据库或 objects。


- 在 startup recovery 模式启动普通 Library、capture server 或后台 AI/capture 服务。
## 9. 测试要求

自动化测试当前覆盖：

- 成功创建、列出和完整验证 restore point。
- 对象 payload 篡改与不安全 manifest path。
- 备份复制失败时清理 staging。
- prepare 阶段创建 safety backup、迁移候选并生成 pending marker。
- prepared 候选被篡改时不触碰在线数据。
- prepared、moving-live、live-moved、candidate-installed 四个阶段的确定性中断与下次启动收敛。
- 候选数据库损坏后的 rollback，并验证旧数据库重新可读。
- live-moved 中部分候选已安装时删除候选并恢复旧数据库/objects。
- moving-live 中尚未移动的 SQLite sidecar 不被删除。
- rollback 必需 payload 缺失时停止启动，不在不一致数据上继续。
- 候选复制会二次校验源 hash；重复 prepare 不创建额外 safety backup。
- prepare 崩溃遗留的 stale lock 在下次启动时清理。
- existing v1 数据库升级前创建并验证 restore point；备份数据库保持迁移前 schema。
- fresh DB 不生成无意义备份；running guard 阻止不确定 migration 的自动重试；已提交 migration 的遗留 guard 可在下次启动收敛。
- 设置页显式确认、恢复结果和 safety backup 展示。
- 启动 recovery UI 展示脱敏错误、verified backup ID，且隐藏 create_backup，只保留 restore 操作。
- 便携导出会写入 manifest、JSONL、metadata 和 markdown，默认跳过 secret，且 metadata 不包含 source/evaluation storage URI 或 credential reference。
- Storage 设置页可触发 portable export；startup recovery 模式隐藏导出入口。

发布前仍需完成：

- Sprint 2 自动化门禁：`npm run readiness:sprint2`，输出 JSON 报告并作为发布候选 artifact 保存。
- 空对象存储、嵌套 evaluation artifact 和额外/缺失 payload 的独立 fixture。
- 0001/0002/0003 migration fixture 已自动化；新增 migration 时必须持续追加上一发布版本。
- 真实进程在四个 phase 边界被强制终止的 Windows 安装包集成测试；当前是函数级确定性中断模拟，真实矩阵以 `docs/sprint2_windows_fault_matrix.md` 为准。
- 非 ASCII Windows 用户目录、Defender、长路径、磁盘空间不足和只读目录，真实矩阵以 `docs/sprint2_windows_fault_matrix.md` 为准。
- 真实安装包的进程重启、连接池释放和 capture server 停止回归。
