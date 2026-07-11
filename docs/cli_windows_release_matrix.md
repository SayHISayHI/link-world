# Node Tide CLI Windows 发布验收矩阵

状态：CLI 代码已更名为 Node Tide；2026-07-04 former Link World 证据仅作历史记录，当前 `node-tide-cli.exe` 的完整 readiness/build/package、签名、Defender、代理/防火墙和用户 PATH 会话验证待重新执行
适用范围：`node-tide-cli.exe`、`install-node-tide-cli.ps1`、共享 Local Edition 数据目录

## 1. 自动化入口

```powershell
npm run readiness:cli
npm run build:cli
```

`readiness:cli` 使用中文临时目录，覆盖 rustfmt/check/clippy、参数与 completion、单 JSON document、稳定退出码、路径脱敏、破坏性操作显式确认、实时 runtime lock 竞争、capture/AI request-id identity、共享 service capture→search→show→delete、便携导出、backup create/list/verify 以及安装/卸载脚本，并原子写出 JSON report。

## 2. 2026-07-04 former Link World 本地历史证据

| 项目 | 结果 |
| --- | --- |
| Debug CLI readiness | 通过；16/16 个编译、测试和真实进程步骤 passed，未跳过 clippy |
| Readiness report SHA-256 | `00354CD94EB2B41A24DADC25D1190963C2E3D71E25EF3E2408D75B5926140B75` |
| `cargo clippy --all-targets -- -D warnings` | 通过 |
| Release build | 历史通过，`link-world-cli.exe` 17,933,312 bytes；build stamp 与二进制 hash 一致 |
| Release SHA-256 | `7D7395742BFBFF41A6E9617A2E989F28694666116E6E5F90706ED0BD7B175C22` |
| Release 签名 | `NotSigned`；Alpha 可留证分发，公开发布前阻断 |
| Release version/JSON | 通过，schema v1 单 JSON document |
| Release 临时 APPDATA fresh DB/diagnostics/list | 通过，migration 0006、路径显示为 `<app-data>` |
| 用户级安装/卸载（不修改 PATH） | Debug 和 Release 均通过 |
| Alpha package rehearsal | 通过；MSI、NSIS、CLI、CLI installer、readiness report、manifest 与 SHA256SUMS 同包 |

上述 hash 对应 former Link World 工件，不适用于 `node-tide-cli.exe`，也不替代最终干净 commit 的 release evidence。

## 3. 发布候选矩阵

| ID | 场景 | 操作 | 通过条件 | 当前状态 | Owner（角色） | 放行级别 |
| --- | --- | --- | --- | --- | --- | --- |
| C1 | 无数据初始化命令 | 在全新 profile 执行 `version`、`--help`、`completion powershell` | 不创建 app data；退出 0；completion 可加载 | 自动化通过 | CLI maintainer | 已关闭 |
| C2 | Fresh DB | 临时 APPDATA 执行 `status`、`diagnostics show`、`object list` | 自动迁移到 latest；JSON schema v1；无绝对路径 | Release 通过 | CLI maintainer | 已关闭 |
| C3 | 中文用户目录 | 中文 APPDATA 执行读写、export、backup | UTF-8 正常；无乱码/绝对路径泄漏 | Debug 自动化与 Release fresh-profile smoke 通过 | Windows QA | 已关闭 |
| C4 | 桌面/CLI 竞争 | 桌面持有数据目录时运行 CLI，反向再测 | 第二进程退出 5，返回 `ERR_RUNTIME_BUSY`，无等待/抢锁/迁移 | Debug 自动化与 Release lock contention 通过；安装版双进程待复核 | Windows QA | RC 阻断 |
| C5 | 崩溃释放锁 | 强制终止持锁进程，再启动另一入口 | OS 自动释放；无 stale marker 阻塞 | 待真实强制终止 | Windows QA | RC 阻断 |
| C6 | Capture 成功/失败 | 本地 fixture、离线、403、超时各执行一次 | 稳定终态和 code；无 raw body/query/secret 日志 | 共享 service 自动化通过；真实网络矩阵待执行 | Windows QA + Security | RC 阻断 |
| C7 | Request idempotency | capture/AI/Evaluation 重复 UUID及跨对象复用 | 同 identity 复用；跨 identity fail closed；无重复 artifact | 自动化通过 | CLI maintainer | 已关闭 |
| C8 | Privacy output | object show 默认/`--include-content`、diagnostics/support bundle | 默认不输出正文；显式升级有警告；路径与 secret 边界不变 | 自动化与单测通过 | Security | 已关闭 |
| C9 | Maintenance | rebuild/reindex/export/backup create/list/verify | 无半成品；结果路径脱敏；secret 对象不导出 | 自动化通过 | CLI maintainer | 已关闭 |
| C10 | Recovery mode | migration guard/损坏 DB 下执行 status、backup list/verify | 普通命令 fail closed；backup catalog 仍可读取/验证 | malformed guard + valid backup 自动化通过；安装版 RC 待复核 | Windows QA | RC 阻断 |
| C11 | 安装与 PATH | 用户级 install、`-AddToPath`、新终端调用、remove | 不需管理员；只修改 User PATH；卸载只删固定 CLI 文件/自身 PATH entry | copy/remove 通过；User PATH 需用户授权实测 | Release engineering + Windows QA | RC 阻断 |
| C12 | Artifact | 执行 build/package，核对 manifest/hash/signature | CLI 与 installer 入包；hash 一致；公开发布签名有效 | package rehearsal 通过；签名待完成 | Release engineering | 公开发布阻断 |
| C13 | Defender/代理 | Windows 10/11 Defender、企业代理/防火墙 | 无误报；失败稳定可解释；不泄露代理凭据 | 待真实环境 | Security + Windows QA | 公开发布阻断 |

## 4. 发布阻断条件

- CLI 与桌面端可同时写同一数据目录。
- JSON 模式混入 banner、spinner、warning 或多份 document。
- 参数、错误、日志、支持包出现 API Key、cookie、session、正文、raw provider/SQLite error 或本机绝对路径。
- request ID 跨对象/跨操作复用未 fail closed。
- 安装脚本修改 Machine PATH、要求管理员或删除非 Node Tide 文件。
- release manifest/checksum 不包含 CLI，或签名/来源状态与实际不符。
- public/commercial 分发时 CLI/安装脚本未签名。

## 5. 留证要求

最终发布证据必须记录 commit、Rust/Node 版本、`readiness:cli` report SHA-256、CLI bytes/hash/signature、Windows 版本、安装/PATH/卸载结果、runtime lock 双进程结果、Defender/代理结果及所有未通过项的 owner/workaround。不得把本文件中的开发工作树 hash 冒充最终 RC hash。
