# Alpha 运行时依赖安全审计

状态: 2026-07-02 自动化处置已完成；真实发布候选仍需在最终干净 commit 上重新生成报告。

适用范围: Windows Local Alpha 的 npm 生产依赖、Rust `Cargo.lock`、当前 Windows 活跃依赖图与 Tauri 发布二进制。

## 1. 审计门禁

发布候选使用：

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/alpha-readiness.ps1 -IncludeSprintGates -IncludeTauriBuild -IncludeNetworkAudits
```

门禁包含 npm high/critical audit、RustSec lockfile audit 和 `scripts/rustsec-waiver-check.ps1`。任何新增 high/critical 漏洞必须修复、形成有边界的 waiver 或阻断发布。

## 2. 2026-07-02 处置记录

| 项目 | 处置 | 发布判断 |
| --- | --- | --- |
| npm production dependencies | 官方 npm registry 审计返回 0 vulnerabilities | 通过 |
| `RUSTSEC-2026-0185` / `quinn-proto` | 从 0.11.14 升级到已修复的 0.11.15 及以上锁定版本 | 已修复 |
| `RUSTSEC-2026-0190` / `anyhow` | 从 1.0.102 升级到已修复的 1.0.103 及以上锁定版本 | 已修复 |
| `RUSTSEC-2023-0071` / `rsa` | SQLx macro 的 lockfile-only 可选依赖；`cargo tree -i rsa` 证明不在当前 Windows 活跃依赖图 | 有条件 waiver |
| RustSec informational warnings | 主要来自非 Windows GTK 依赖或构建/宏依赖；不以 warning 冒充无风险，后续 Tauri/SQLx 升级时复核 | Alpha 接受，持续跟踪 |

## 3. RSA waiver 边界

`rsa 0.9.10` 没有上游修复版本。当前项目只启用 SQLx SQLite，不实现 MySQL、RSA 私钥操作、TLS 终止或服务端签名。该 crate 存在于 lockfile 的 SQLx 可选依赖集合，但不存在于当前 Windows 活跃依赖图，因此不进入 Link World Windows Alpha 运行时。

此 waiver 仅在以下条件同时成立时有效：

- `scripts/rustsec-waiver-check.ps1` 证明 `cargo tree -i rsa` 没有活跃节点。
- 产品仍然不启用 SQLx MySQL/Postgres、RSA 私钥操作或可远程观测的 RSA 服务端路径。
- 安全报告明确记录 `RUSTSEC-2023-0071`，不得删除记录后声称 lockfile 零告警。

一旦 RSA 进入活跃依赖图，waiver 检查必须 fail closed，并将 Alpha 发布标记为阻断，直到移除依赖或完成新的风险评审。

## 4. 依赖面缩减

SQLx 已设置 `default-features = false`，只保留 Tokio runtime、SQLite、chrono、UUID、migration 和 compile-time migration macro。项目不为未来可能性携带 MySQL、Postgres 或 `any` runtime capability。

## 5. 剩余边界

- 自动化不能替代安装包签名、Defender、代理/防火墙与真实 Windows 10/11 验证。
- lockfile 扫描会覆盖跨平台和可选依赖；发布判断必须同时参考当前 Windows 活跃依赖图和真实发布二进制。
- Public/commercial distribution 前必须再次审计并处理所有 high/critical，且完成签名来源验证。
