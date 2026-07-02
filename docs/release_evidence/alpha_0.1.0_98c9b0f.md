# Windows Alpha 0.1.0 RC 自动化证据

状态: 代码级与构建级通过；真实安装、升级、卸载、Credential Manager 和邀请用户矩阵未完成。

证据时间: 2026-07-02 10:39–10:40 +08:00

代码 commit: `98c9b0f44e2ba7af1fbcb31dbc0dc4f800356488`

最终文档 commit: 本文件所在的 docs-only 后续 commit，与上述应用代码等价。

应用版本: `0.1.0`

schema version: `6`

构建环境: Windows x64、Node `v24.14.0`、npm `8.19.4`

## 1. Alpha readiness

在干净工作区执行：

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/alpha-readiness.ps1 -IncludeSprintGates -IncludeTauriBuild -IncludeNetworkAudits
```

结果：17/17 steps passed，0 failed，0 warning，报告记录 `dirtyWorktree=false`，累计步骤耗时 526,932 ms。

覆盖：

- Node 版本预检、frontend typecheck、39 个 Vitest tests、production build。
- Rust fmt、check、132 个通过/2 个 ignored 的 tests、Clippy warnings gate。
- Sprint 2 数据安全、Sprint 3 capture、Sprint 5 observability 聚合门禁。
- MSI/NSIS Tauri package build。
- npm production audit、RustSec audit 和 RSA waiver 活跃依赖检查。

Readiness report SHA-256：

```text
103529E73EF72331AC62B6F7A920EF750BB92E72A5705E5BBDC26E6CE8A599A6
```

## 2. 发布候选工件

本地证据目录：`C:\tmp\link-world-alpha-0.1.0-98c9b0f`。

| 工件 | 大小 | SHA-256 |
| --- | ---: | --- |
| `link-world-0.1.0-windows-x64-98c9b0f4.msi` | 9,048,064 bytes | `3CF840DC1B6794521152672772ADA2BBC3DF4BA61C80FF4105ACA5784822CE6F` |
| `link-world-0.1.0-windows-x64-98c9b0f4-setup.exe` | 6,416,807 bytes | `922E159E1CCB8A72EEDB8D369B700C2B6F4913B507598A0A581FF41504524137` |
| `release-manifest.json` | — | `5532F082501D1B15A7F201F1BEF1D2D8B4263E59456BC2391209CB88876D531E` |

独立复算的工件 hash 与 manifest 一致。MSI metadata 为 Link World `0.1.0`、x64；NSIS file/product metadata 为 Link World `0.1.0`。

## 3. 签名与分发边界

MSI 和 NSIS Authenticode 均为 `NotSigned`，manifest 明确记录 `unsigned_alpha`。该工件只适合受控邀请制测试，必须通过可信的带外渠道提供 SHA-256；不满足公开或商业分发要求。

## 4. 未执行项

当前 Windows profile 已存在 Link World 用户数据。为避免用真实数据执行未经授权的迁移，本轮没有直接启动 RC，也没有安装、覆盖或卸载应用。

因此以下项目仍是 Week 9 发布门禁，不得由本文件推断为通过：

- W9-01/W9-02 干净 Windows 11/10 安装与启动。
- W9-03/W9-04 原地升级与中断恢复。
- W9-05/W9-06 卸载数据保留和显式删除边界。
- W9-08 Credential Manager 跨重启/升级行为。
- W9-09/W9-10/W9-11/W9-12 网络、非 ASCII profile、长路径与 Defender。
- W9-14 rollback，以及 Week 10 真实用户观察。

下一次安装级测试必须使用一次性 Windows profile、VM 或经用户明确授权并先完成可验证备份的 profile。
