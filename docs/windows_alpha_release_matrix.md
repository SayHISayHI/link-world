# Week 9 Windows Alpha 发布工程化矩阵

状态: Draft，自动化门禁与证据模板已建立；真实 Windows 10/11 安装、升级、卸载、签名和安全审计证据仍需在发布候选包上执行。
适用范围: Week 9 Windows Local Alpha 的打包、签名、安装、升级、卸载、数据保留、环境矩阵、凭据回归和依赖安全审计。

## 1. 目标与边界

Week 9 的目标是把当前代码从“开发机可运行”推进到“可以交给邀请制用户安装的 Windows Alpha 发布候选”。本矩阵不新增云账号、自动更新、托管模型网关或动态插件安装。

完成 Week 9 不能只看 `cargo test` 或 `npm test`。最低证据必须同时覆盖:

- 可追踪的发布候选 commit、版本、schema version 和构建时间。
- Windows 安装包可以安装、启动、卸载和重装。
- 从上一 Alpha 原地升级时，本地数据库、对象存储、AI trace、Evaluation 和 Windows Credential Manager 凭据仍可用。
- 卸载默认不删除用户数据；若未来提供删除数据选项，必须由用户明确选择。
- 安装包来源、版本和完整性可以被用户或测试人员验证。
- 依赖、安全和许可证风险经过审计，P0/P1 风险进入发布阻断或 waiver。

## 2. 自动化门禁

默认命令:

```powershell
npm run readiness:alpha
```

发布候选环境应使用完整门禁:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/alpha-readiness.ps1 -IncludeSprintGates -IncludeTauriBuild -IncludeNetworkAudits -OutputPath C:\tmp\link-world-alpha-readiness.json
```

该脚本负责聚合:

- frontend typecheck、Vitest 和 production build。
- Rust format、check、test 和 clippy warnings gate。
- 本地 dependency inventory。
- 可选 Sprint 2/3/5 readiness JSON 报告。
- 可选 Tauri Windows package build。
- 可选 `npm audit` / `cargo audit` 网络审计。
- release metadata: app version、commit SHA、branch、schema version、构建时间。

自动化报告只能证明代码级和构建级门槛。它不能替代 Windows 安装包真实安装、进程强制终止、Credential Manager、代理/防火墙、非 ASCII 用户目录、卸载器行为或人工安全复核。

## 3. 发布候选工件

每个 Windows Alpha release candidate 必须保存:

| 工件 | 要求 |
| --- | --- |
| 安装包 | `.msi` 或 `.exe`，文件名包含 product、version、platform、arch 和 short commit |
| checksum | SHA-256，独立于安装包保存 |
| release manifest | app version、schema version、commit SHA、branch、build time、package type、签名状态 |
| readiness reports | `readiness:alpha`，以及 Sprint 2/3/5 reports |
| dependency inventory | npm runtime dependency list、Cargo dependency tree、许可证/安全审计摘要 |
| release notes | migration 风险、已知问题、回滚方式、支持包说明 |
| rollback note | 如何保留数据、如何使用 restore point、何时停止升级 |

签名边界:

- Public/commercial distribution requires signed installer and signed executable.
- 邀请制 Alpha 可以使用待签名 build，但必须在 manifest 中标记 `signatureStatus=unsigned_alpha`，并向测试用户说明来源验证方式。
- 证书、私钥、密码不得进入仓库、日志、支持包或 release artifact。

## 4. Windows 真实矩阵

| 编号 | 场景 | 步骤 | 必须证明 |
| --- | --- | --- | --- |
| W9-01 | 干净安装 | 在干净 Windows 11 安装当前包，首次启动，不配置模型，保存和搜索一个 URL | 安装成功；无模型时保存、解析、FTS 可用；Diagnostics 显示正常降级 |
| W9-02 | Windows 10 兼容 | 在 Windows 10 执行 W9-01 smoke | 不崩溃；若平台限制存在，记录为 release note 或 blocker |
| W9-03 | 上一 Alpha 升级 | 用上一 Alpha 创建含 public/personal/sensitive/secret、AI trace、Evaluation、failed job、provider credential 的 profile，再安装当前包 | 迁移前 restore point 创建并验证；升级后对象、派生产物、FTS 和 credential 可用 |
| W9-04 | 升级中断 | 在安装或首次升级启动期间强制结束应用，再重新启动 | 进入可恢复状态或完成收敛；不得打开损坏 Library |
| W9-05 | 卸载保留数据 | 卸载应用后检查 app data，再重装当前包 | 默认不删除用户数据；重装后 profile 可启动；Credential Manager 行为被记录 |
| W9-06 | 删除数据选项边界 | 若安装器提供删除数据选项，执行保留和删除两条路径 | 删除必须由用户明确选择；保留路径不得误删；删除路径不得残留误导性状态 |
| W9-07 | 安装包来源验证 | 对安装包计算 SHA-256，检查版本、commit 和 manifest | checksum 匹配；应用 About/Diagnostics 或 release manifest 能追踪到版本和 commit |
| W9-08 | Credential Manager | 新建、编辑、删除 provider key，重启和升级后再次测试 | SQLite、日志、支持包不含 key；删除配置会删除 credential；空 key 编辑保留旧凭据 |
| W9-09 | 代理/防火墙/离线 | 在代理、防火墙拦截和完全离线下启动、capture、GitHub evaluator 和 AI connection test | 本地库可用；网络失败使用稳定 code；不泄漏 raw provider/network error |
| W9-10 | 非 ASCII 用户目录 | 在中文或 emoji 用户目录安装，执行启动、backup、support bundle、capture | 路径派生正确；UI 不乱码；支持包不含绝对 profile 路径 |
| W9-11 | 长路径 | 使用长标题、深对象目录和便携导出 | Windows 非法路径被拒绝或规范化；无任意路径写入 |
| W9-12 | Defender/杀软 | Defender 实时保护开启时执行 capture、backup、restore、support bundle | transient file locks 可恢复或可诊断；无半成品伪成功 |
| W9-13 | 依赖/许可证审计 | 保存 npm/Cargo inventory，执行 high/critical security review | high/critical runtime 风险有修复、waiver 或 release blocker；许可证风险有记录 |
| W9-14 | 回滚演练 | 当前包升级后回滚到上一 Alpha 或用 restore point 恢复 | 兼容策略清楚；不兼容 schema 必须阻断二进制回滚并提示 restore |

## 5. 证据要求

每个矩阵项保存:

- app version、commit SHA、schema version、package type、Windows version。
- 执行人、执行时间、profile 类型和测试数据集版本。
- pass/fail、截图或脱敏日志尾部。
- 关联的 object/job/evaluation/correlation id，禁止保存正文或 secret。
- 失败项的 owner、严重级别、修复 commit、workaround 或 release waiver。

证据禁止包含:

- API key、token、cookie、session。
- source snapshot、parsed document 正文、prompt 原文或 embedding。
- URL query/fragment。
- credential reference value。
- 本机绝对数据路径，除非只存在于本地测试记录且不进入可共享包；共享证据必须脱敏。

## 6. 完成边界

Week 9 可以标记为完成的最低条件:

- `readiness:alpha` 在发布候选 commit 上通过，报告被保存。
- Sprint 2/3/5 自动化报告在同一 commit 或明确记录的等价 commit 上通过。
- W9-01、W9-03、W9-05、W9-07、W9-08、W9-09、W9-10、W9-13 在 Windows 11 主目标上通过。
- W9-02 在 Windows 10 至少执行一次；失败必须有 release note 或 blocker。
- W9-04、W9-11、W9-12、W9-14 至少执行一次；失败必须有 owner、到期日和明确风险。
- `docs/post_mvp_roadmap.md`、`docs/devops_and_ci.md` 和 `docs/operational_readiness.md` 同步记录验收日期、报告位置和剩余风险。

只完成自动化脚本、只生成安装包、或只在开发机运行 `tauri dev`，都不足以证明 Week 9 完成。
