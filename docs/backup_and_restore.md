# 本地备份与恢复规范

状态：Partial implementation
当前范围：创建、列出和验证本地 restore point
尚未实现：恢复、导出、自动保留策略

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
- 当前不支持从 UI 恢复，防止在缺少 preflight 和 rollback 时替换在线数据。

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

## 7. 恢复设计边界

恢复功能必须在启用前完成以下流程：

1. 完整验证目标备份。
2. 验证 manifest appVersion 与当前 migration 兼容范围。
3. 为当前在线数据创建 safety restore point。
4. 停止后台任务、capture server 和所有数据库写入。
5. 关闭 SQLite pool。
6. 把当前数据库与对象目录 rename 到 rollback 位置。
7. 把备份复制到新的 staging 数据目录。
8. 运行 migration、foreign_key_check、quick_check 和对象清单检查。
9. 原子切换数据目录并重启 AppState。
10. 任一步失败时恢复 rollback 目录。

在上述流程和真实 Windows 回归完成前，UI 只能创建、列出和验证，不提供 Restore 按钮。

## 8. API 契约

- create_backup：创建 restore point，返回 BackupSummary。
- list_backups：按创建时间倒序返回 summary；损坏 manifest 以 status=invalid 暴露。
- verify_backup：执行完整 hash 与 SQLite 校验，返回 BackupVerification。

命令不得：

- 接收任意文件系统路径。
- 返回用户正文或 manifest 文件列表。
- 读取或导出 Windows Credential Manager 中的值。
- 自动删除旧备份。
- 在后台静默恢复数据。

## 9. 测试要求

最低测试矩阵：

- 空对象存储。
- 含嵌套对象和 evaluation artifact。
- 在线 WAL 数据库生成一致快照。
- manifest hash 被修改。
- payload 内容被修改。
- payload 缺失或出现额外文件。
- SQLite 文件损坏。
- staging 创建或复制中断。
- 非 ASCII Windows 用户目录。
- 真实 Windows Defender、长路径和磁盘空间不足。

当前自动化测试已覆盖成功创建、列出、完整验证和对象文件篡改检测。
