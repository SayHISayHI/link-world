# 本地备份与恢复规范

状态：Restore lifecycle implemented；真实 Windows 故障矩阵待回归
当前范围：创建、列出、验证、预检、重启恢复和失败回滚
尚未实现：便携导出、自动保留策略、跨版本迁移 fixture

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

- 后续单独实现。
- 默认排除 secret 内容、credential reference、内部任务记录和本机路径。
- 必须让用户显式选择 sensitive 内容。
- 不得复用原始 SQLite restore point 作为对外分享格式。

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

## 8. API 契约

- create_backup：创建 restore point，返回 BackupSummary。
- list_backups：按创建时间倒序返回 summary；损坏 manifest 以 status=invalid 暴露。
- verify_backup：执行完整 hash 与 SQLite 校验，返回 BackupVerification。
- prepare_restore：重新验证、创建 safety backup、迁移并验证候选，返回 RestorePreparation。
- restart_to_apply_restore：仅在 pending restore 存在时安排进程重启。
- get_restore_status：返回最近一次 succeeded、rolled_back 或 failed 结果；没有结果时返回 null。

命令不得：

- 接收任意文件系统路径。
- 返回用户正文或 manifest 文件列表。
- 读取或导出 Windows Credential Manager 中的值。
- 自动删除旧备份或 safety backup。
- 未经用户明确确认在后台准备或应用恢复。
- 在在线 SQLite pool 存活时替换数据库或 objects。

## 9. 测试要求

自动化测试当前覆盖：

- 成功创建、列出和完整验证 restore point。
- 对象 payload 篡改与不安全 manifest path。
- 备份复制失败时清理 staging。
- prepare 阶段创建 safety backup、迁移候选并生成 pending marker。
- 模拟重启后恢复数据库和对象文件。
- 候选数据库损坏后的 rollback，并验证旧数据库重新可读。
- moving-live 中尚未移动的 SQLite sidecar 不被删除。
- prepare 崩溃遗留的 stale lock 在下次启动时清理。
- 设置页显式确认、恢复结果和 safety backup 展示。

发布前仍需完成：

- 空对象存储、嵌套 evaluation artifact 和额外/缺失 payload 的独立 fixture。
- 从每个已发布 schema 版本升级的 migration fixture。
- 进程在四个 phase 边界被强制终止的 Windows 集成测试。
- 非 ASCII Windows 用户目录、Defender、长路径、磁盘空间不足和只读目录。
- 真实安装包的进程重启、连接池释放和 capture server 停止回归。
