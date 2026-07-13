# Node Tide Windows Alpha 0.1.0 — `ba3a45a`

状态：Automated RC passed；真实 Windows 安装矩阵与代码签名待完成。

2026-07-12 在干净 commit `ba3a45a` 上完成 21/21 自动化步骤，候选目录为本机构建输出 `src-tauri/target/release-evidence/node-tide-alpha-0.1.0-ba3a45a`。该目录由 `.gitignore` 排除，本文只保存可复核的非敏感摘要。

## 候选工件

| 工件 | SHA-256 |
| --- | --- |
| MSI | `5A63896C7CCB4A245F91CD78F9C8C1EAC91C43197C0B7DBFF444EEC74F53FD13` |
| NSIS setup | `33ADD141448F0FE94B4043BF3B6AAA4920D50F8E87608D778D089BBCF95506BB` |
| `node-tide-cli.exe` | `FE602B696C24615BD4F1305F9BCB507CE94FB7B9C3724288DAA45A9EABBCB9A7` |

Manifest 将签名状态标记为 `unsigned_alpha`。安装前必须通过带外可信渠道取得本页或发布清单并独立复算 SHA-256。

## 自动门禁

- Frontend lint、typecheck、Vitest、production build 和 Playwright E2E。
- Rust fmt、check、test、clippy。
- Sprint 2/3/5 与 CLI readiness。
- Tauri MSI/NSIS、独立 CLI release build、manifest/checksum 组装。
- npm production audit、cargo audit 与 RustSec waiver 活跃依赖校验。
- 候选报告与工件绑定同一 commit，工作树为 clean。

本次门禁发现并修复 `quick-xml` 的 RUSTSEC-2026-0194 / RUSTSEC-2026-0195，锁文件升级到 `quick-xml 0.41.0`（经 `plist 1.10.0`），并将 cargo audit 改为强制门禁。

## 未由此证据证明

- Windows 10/11 干净安装、升级、卸载、重装和数据保留语义。
- Credential Manager、代理、防火墙、Defender、长路径与非 ASCII 用户目录。
- Authenticode 签名、第二台机器的独立 checksum 复算和真实分发来源。
- 5-15 名邀请用户、至少一次完整任务观察、P0/P1 状态和下一阶段优先级。

这些项目必须继续按 `docs/windows_alpha_release_matrix.md` 与 `docs/alpha_feedback_playbook.md` 留存真实证据；不能由自动测试或 synthetic fixture 替代。
