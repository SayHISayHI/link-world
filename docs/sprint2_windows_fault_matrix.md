# Sprint 2 Windows 故障矩阵

状态: Draft，自动化门禁已建立；真实安装包矩阵仍需在发布候选包上执行。
适用范围: Sprint 2 数据安全、迁移、恢复、启动 recovery UI 和便携导出的 Windows 发布验收。

## 1. 目标

Sprint 2 的核心承诺是：用户数据在备份、迁移、恢复和导出路径中不会因为进程中断、安装升级、路径差异或权限异常进入“看似成功但实际损坏”的状态。

本矩阵把验收分成两层：

- 自动化门禁：在开发机或 CI 上运行 `npm run readiness:sprint2`，覆盖函数级确定性故障注入、fixture migration、restore rollback 和隐私边界。
- 真实 Windows 矩阵：在签名或待签名安装包上执行，覆盖安装包升级、真实进程强制终止、Windows 用户目录和文件系统异常。

## 2. 自动化门禁

默认命令：

```powershell
npm run readiness:sprint2
```

脚本会顺序执行：

- Rust 格式检查。
- 0001/0002/0003 historical migration fixture → current 0007 与 startup guard 测试。
- 普通启动 migration restore point 和 fail-closed guard 测试。
- backup manifest、hash、staging cleanup 测试。
- 两阶段 restore 的 prepared、moving-live、live-moved、candidate-installed 和 rollback 矩阵。
- portable export 隐私边界测试。
- startup recovery status 脱敏测试。
- Rust clippy warnings gate。

脚本默认把 JSON 报告写入系统临时目录，报告包含每个步骤的命令、退出码、耗时和日志尾部。发布候选分支必须保存该报告到 CI artifact 或发布验收记录。

可选参数：

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/sprint2-readiness.ps1 -OutputPath C:\tmp\node-tide-sprint2-readiness.json
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/sprint2-readiness.ps1 -IncludeFrontend
```

`-IncludeFrontend` 只有在本机 `node_modules` 完整且能运行 Vitest/TypeScript 时才作为发布门禁启用；核心 Sprint 2 数据安全门禁不依赖前端包状态。

## 3. 真实 Windows 发布矩阵

| 编号 | 场景 | 步骤 | 必须证明 |
| --- | --- | --- | --- |
| W2-01 | 新安装启动 | 安装当前 Windows 包，首次启动，保存 3 个对象，创建 restore point | 空库 migration 成功；restore point 可验证；Library 可读 |
| W2-02 | 旧版本原地升级 | 用上一发布版本创建含 public/personal/sensitive/secret、AI trace、evaluation、failed job 的库，再安装当前包 | 迁移前自动创建并验证 restore point；升级后对象、FTS、AI/eval 派生产物和 privacy level 不丢失 |
| W2-03 | migration running guard 中断 | 在 migration guard running 后强制结束主进程，再启动当前包 | 应用进入 recovery/fail-closed，不进入普通 Library；显示 verified backup ID；只开放 verify/restore/restart |
| W2-04 | restore prepared 后中断 | Storage 中确认 restore，生成 pending marker 后强制结束进程并重启 | 下次启动在 pool 建立前继续 restore；成功后 candidate 数据可读，旧 safety backup 保留 |
| W2-05 | moving-live 阶段中断 | 在 live DB/objects 移入 rollback 过程中强制结束进程并重启 | 已移动 payload 被恢复；未移动 payload 不被删除；旧库可启动 |
| W2-06 | live-moved 阶段中断 | 在线 payload 已完整移入 rollback、candidate 尚未完全安装时强制结束进程 | 删除不完整 candidate；恢复旧 DB/objects；last result 为 rolled_back 或 failed 且脱敏 |
| W2-07 | candidate-installed 后初始化失败 | 注入损坏 candidate DB 或缺失对象 payload 后重启 | 关闭新 pool 后 rollback；旧 DB quick_check 通过；不得在损坏 candidate 上启动 |
| W2-08 | 非 ASCII 用户目录 | 在中文或 emoji Windows 用户目录下安装并执行 W2-01/W2-04 | 所有路径派生在 app data 内；UI/日志不乱码；restore/report 不泄漏正文 |
| W2-09 | 长路径 | 构造较长对象标题/导出目录并创建 backup/export | manifest path 规范化通过；Windows 禁止路径段被拒绝；无任意路径写入 |
| W2-10 | 只读或权限受限目录 | 将 objects 或 backups 子目录设为只读后执行 backup/restore/export | 命令失败且不发布半成品；staging/candidate 清理或保留可诊断状态 |
| W2-11 | 磁盘空间不足 | 在受限磁盘/虚拟磁盘中执行 backup 和 portable export | 失败不生成正式 restore point/export；原 Library 可启动 |
| W2-12 | Defender/杀软干扰 | 启用 Defender 实时扫描执行 restore 和 migration | 错误可诊断；无正文/凭据泄漏；旧数据保持可恢复 |
| W2-13 | 卸载与重装 | 卸载应用后重装，保留用户数据目录 | 用户数据不被卸载器误删；重装后 migration/recovery 语义一致 |

## 4. 证据要求

每次发布候选至少保存：

- 安装包版本、commit SHA、Windows 版本和安装路径。
- `npm run readiness:sprint2` JSON 报告。
- 每个真实矩阵项的结果：pass/fail、执行人、时间、数据目录位置、关键截图或日志尾部。
- 对失败项的处置：修复 commit、降级决策或明确阻断发布。

不得保存：

- API Key、token、cookie、session。
- 用户正文、secret 对象正文、embedding。
- 带 query/fragment 的原始 URL。
- Windows Credential Manager 中的 credential value。

## 5. 完成边界

Sprint 2 可以从“执行中”切到“完成”的最低证据是：

- 自动化门禁在发布候选 commit 上通过。
- W2-01 至 W2-07 在 Windows 11 当前主目标上通过。
- W2-08 至 W2-13 至少完成一次手动矩阵；若失败，必须有明确 release-blocker 或 documented waiver。
- `/docs/backup_and_restore.md`、`/docs/operational_readiness.md` 和 `/docs/post_mvp_roadmap.md` 同步记录验收日期与剩余风险。