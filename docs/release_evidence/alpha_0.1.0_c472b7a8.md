# Node Tide Windows Alpha 0.1.0 — `c472b7a8`

状态：Automated RC passed；真实 Windows 安装矩阵与代码签名待完成。

2026-07-13 在隔离的干净 worktree 中，以完整 commit `c472b7a8cf01d59505d16ea38a3e36ece9ffe964` 完成 22/22 自动化步骤，0 failed、0 warning，报告记录 `dirtyWorktree=false`。本候选包含空库三步 onboarding、可选 AI 设置入口、Alpha 用户 README 和 fail-closed 反馈证据门禁。

本地候选目录：`src-tauri/target/release-evidence/node-tide-alpha-0.1.0-c472b7a8`。该目录由 Git 忽略，本文保存可复核的非敏感摘要。

## 候选工件

| 工件 | Bytes | SHA-256 |
| --- | ---: | --- |
| `node-tide-0.1.0-windows-x64-c472b7a8.msi` | 16,318,464 | `23A7B48CFBFEB9F789582215547C51812A6F155CB5CE455032250EEA5AAFEA16` |
| `node-tide-0.1.0-windows-x64-c472b7a8-setup.exe` | 11,418,185 | `8487E9B98B21D3C94556EB17BE4025802C3ECC20DA55D1D744C97ADF02D7974D` |
| `node-tide-cli.exe` | 17,990,656 | `1257B2C203EF3E460047475E70A901881EDDAA031006D4B512795DD027AF8A54` |
| `install-node-tide-cli.ps1` | 2,964 | `D477833CBF8247DE9AEBD022A728006ECD4687A3E26911F79B33C4FAC3DEB90F` |
| `alpha-readiness.json` | 49,648 | `956C1EAA62CE748A4EC6CC90F6DBDAB8398E9C6D1DBB20D88EE3079283BE5119` |
| `release-manifest.json` | — | `F31CD4374AE2C43CE0C6545B61C99C6E1D00B31FB21B989F6773F31E4A589D0B` |

所有 `SHA256SUMS.txt` 条目均已使用 `Get-FileHash -Algorithm SHA256` 独立复算并匹配。Manifest、readiness report 与 CLI build metadata 均绑定同一 commit；manifest 和 report 均记录 `dirtyWorktree=false`。

## 22 项自动门禁

- Node.js runtime、frontend lint/typecheck/Vitest/production build。
- Alpha 反馈证据 contract，包括 synthetic-as-real、敏感/未知字段、小 cohort、开放 P0 和不完整 P1 的 fail-closed 测试。
- Playwright 浏览器主流程，包括 onboarding、保存、搜索、无模型降级和 Settings。
- Rust fmt/check/test/clippy，Sprint 2/3/5 readiness 与 CLI readiness。
- Tauri MSI/NSIS 和独立 CLI release build。
- RustSec waiver validation、npm production audit 和 cargo audit。

## 签名和剩余边界

全部可执行工件当前为 `unsigned_alpha`。它们只适用于带外提供可信 SHA-256 的小规模邀请制 Alpha；公开下载前必须完成 Authenticode 签名。

此证据不证明 Windows 10/11 真实安装、原地升级、卸载/重装数据保留、Credential Manager、代理/防火墙、Defender、非 ASCII 用户目录、CLI 安装版锁竞争，也不证明 5-15 名用户已受邀。后续必须按 `docs/windows_alpha_release_matrix.md` 和 `docs/alpha_feedback_playbook.md` 留存真实证据。
