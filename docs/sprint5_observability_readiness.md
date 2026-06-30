# Sprint 5 可观测性与支持就绪矩阵

状态: Draft，自动化门禁已建立；真实 Windows、用户确认、运行态轮转和支持交接矩阵仍需在发布候选包上执行。
适用范围: Sprint 5 本地 Diagnostics、结构化日志、correlation id、脱敏支持包、失败任务恢复入口和运维证据验收。

## 1. 目标与边界

Sprint 5 的核心承诺是：用户能够在本机判断应用、数据和后台任务是否健康，能够在明确确认后生成可交给支持人员的最小诊断证据；诊断路径自身不得复制正文、完整 URL、凭据、embedding、原始第三方错误或本机绝对路径。

本矩阵分成两层：

- 自动化门禁：运行 `npm run readiness:sprint5`，覆盖 Diagnostics 健康聚合与脱敏、有界日志校验和轮转、支持包确认/隐私/原子发布，以及 capture 生命周期 correlation 一致性。
- 真实发布候选矩阵：在 Windows 安装包上执行用户确认、目录权限、非 ASCII 用户目录、运行态轮转、100 个失败任务、支持包人工复核和支持交接。

自动化只证明确定性代码边界。它不能证明已安装应用的交互可理解、Windows 文件系统行为、真实运行时性能，或人工接收方能用有限证据完成问题定位。

## 2. 自动化门禁

默认命令：

```powershell
npm run readiness:sprint5
```

脚本顺序执行：

- Rust 格式检查。
- 本地 Diagnostics 健康、模型正常降级和 failed-job 脱敏测试。
- JSONL schema、敏感 marker 拒绝、2 MiB 边界和单份轮转测试。
- 支持包 command-level 确认、固定目录、原子发布、size/SHA-256 和诱饵秘密隐私测试。
- capture submit/fetch 以及 AI enrichment 成功/失败生命周期的 job payload、domain event、IPC result 与日志 correlation 一致性和 payload 脱敏测试。
- search rebuild 使用持久化 job UUID、reindex 使用生成的 operation/job UUID 作为 correlation；覆盖完成/取消/失败日志、重复取消幂等、原子 swap 故障清理，以及 query/rebuild/reindex 在 job/log/support-bundle/IPC 边界的稳定 `search.*` 失败证据。
- Rust clippy warnings gate。

默认报告写入系统临时目录，包含命令、退出码、耗时和最多 80 行日志尾部。发布候选分支必须把报告保存为 CI artifact 或发布验收记录。

可选参数：

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/sprint5-readiness.ps1 -OutputPath C:\tmp\link-world-sprint5-readiness.json
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/sprint5-readiness.ps1 -IncludeFrontend
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/sprint5-readiness.ps1 -SkipClippy
```

`-IncludeFrontend` 仅在 `node_modules` 完整时启用，用于执行 typecheck 和全部前端测试。发布候选的 Diagnostics 交互验收仍必须包含前端门禁；本机依赖不完整时不得把未运行写成通过。`-SkipClippy` 只用于定向开发诊断，不构成发布证据。

## 3. 受控数据集与隐私诱饵

使用独立测试 profile，写入以下合成数据：

- 3 个 public、2 个 personal、1 个 sensitive、1 个 secret 对象。
- 100 个 failed jobs，混合可重试 capture、不可重试 capture、AI auth/rate-limit/timeout 和未知 job type。
- URL query/fragment 诱饵：`support-query-canary`、`support-fragment-canary`。
- 凭据与正文诱饵：`support-api-key-canary`、`support-cookie-canary`、`support-body-canary`。
- Windows 路径诱饵：`C:\Users\Canary\PrivateProfile`。
- 至少 3 次成功 capture 与 3 次失败 capture，用于 correlation 和日志轮转检查。

诱饵只能使用合成值，不得复制真实用户数据或真实凭据。检查支持包和日志时必须对上述字符串做精确搜索。

## 4. 真实 Windows/支持矩阵

| 编号 | 场景 | 步骤 | 必须证明 |
| --- | --- | --- | --- |
| W5-01 | 健康 profile | 启动含已解析对象的发布候选包，打开 Settings > Diagnostics | 版本、schema、DB/object store、job 和 model 能力状态可读；健康状态与实际数据一致 |
| W5-02 | 未配置模型 | 删除全部模型配置后刷新 Diagnostics | 显示正常能力降级而非系统故障；保存、解析和搜索仍可用 |
| W5-03 | 失败任务恢复入口 | 从 100 个 failed jobs 中分别打开有关联对象和无关联对象的条目，并重试可重试 capture | 有对象时可导航；无对象时不产生坏链接；仅允许支持的 job type 重试，状态最终收敛 |
| W5-04 | UI 确认边界 | 未勾选确认时尝试导出，再勾选并导出 | 未确认时 UI 与 command 均拒绝；确认文案明确说明内容和排除项；确认后只生成一个正式文件 |
| W5-05 | 固定目录与原子发布 | 导出期间观察 `support-bundles`，并模拟目标目录权限失败 | 成功时不暴露半成品；失败时无伪成功文件；输出始终位于 app data 固定子目录 |
| W5-06 | hash 与大小 | 对成功支持包独立计算 SHA-256 和字节数 | 与命令返回值完全一致；JSON `schemaVersion=1` 且能被独立解析 |
| W5-07 | 支持包隐私复核 | 对支持包搜索第 3 节全部诱饵，并检查各 section | 不含正文、title 诱饵、完整 URL/query/fragment、credential reference、API key/token/cookie/session、embedding、raw job/event payload、raw error、本机绝对路径 |
| W5-08 | capture、AI 与 search correlation | 各完成一次成功/失败 capture、成功/失败 AI enrichment、成功/取消/失败 search rebuild 和对象 reindex，比较 job、domain event、IPC result、结构化日志和支持包摘要 | 每次流程共享一个稳定 UUID；search 使用 job UUID；不同操作不错误复用；AI event payload 只含 analysis id 或稳定 `ai.*` code；search 不含 query/index content/raw SQLite error；支持包只含允许字段 |
| W5-09 | 运行态日志轮转 | 连续生成足够多的合成 capture 事件跨过 2 MiB 边界 | 当前文件不超过边界，仅保留一份 `.1`；每行是独立合法 JSON；应用持续可用 |
| W5-10 | 轮转时进程终止 | 在高频写日志和临近轮转时强制结束进程，随后重启并导出支持包 | 业务数据可启动；logger 不阻塞启动；损坏/不合规行被跳过且原始内容不进入支持包 |
| W5-11 | 非 ASCII 用户目录 | 在中文或 emoji Windows 用户目录执行 W5-01、W5-04、W5-09 | 日志和支持包写入成功；UI 文本不乱码；导出内容不泄漏绝对 profile 路径 |
| W5-12 | 目录不可写 | 临时限制 logs 和 support-bundles 子目录权限，执行 capture 与导出 | 日志 best-effort 失败不回滚已提交 capture；支持包明确失败且不发布半成品 |
| W5-13 | 100 个失败任务性能 | 使用第 3 节数据集打开 Diagnostics、滚动、打开对象并触发 retry | 页面保持可交互；摘要有界；不得为诊断读取正文、snapshot 或 embedding |
| W5-14 | 支持交接演练 | 仅把支持包和版本/复现步骤交给未参与开发的工程师 | 接收方能定位模块、稳定 error code、job/object/correlation id 和时间窗口；若证据不足，记录缺失字段，不扩大默认隐私范围 |

## 5. 证据要求

每次发布候选保存：

- 应用版本、commit SHA、Windows 版本、安装包类型和测试 profile 版本。
- `npm run readiness:sprint5` JSON 报告；前端依赖完整环境还需 `-IncludeFrontend` 报告。
- W5-01 至 W5-14 的 pass/fail、执行时间、执行人、关键截图和脱敏标识符。
- W5-06 的独立 hash/size、W5-09 的轮转前后大小、W5-13 的交互耗时、W5-14 的定位结果。
- 失败项的修复 commit、release blocker，或包含负责人、到期日和风险说明的 waiver。

证据不得包含用户正文、source snapshot、完整 URL query/fragment、credential reference、API key、token、cookie、session、embedding、本机绝对数据路径或原始第三方错误。

## 6. 完成边界

Sprint 5 从“执行中”切换到“完成”必须同时满足：

- 自动化门禁在发布候选 commit 上通过，且 JSON 报告被保留。
- 前端依赖完整环境中的 Diagnostics、失败任务操作和支持包确认组件测试通过。
- W5-01 至 W5-08、W5-11 至 W5-14 在 Windows 11 主目标上通过。
- W5-09 和 W5-10 各至少执行三轮，不能用单元测试轮转代替真实运行态证据。
- 路线图中列出的关键流程均已接入受限结构化日志和稳定 correlation id；当前 capture、AI enrichment 与 search maintenance 满足，restore、migration 等启动期流程仍待接入，因此该项仍未完成。
- `/docs/post_mvp_roadmap.md`、`/docs/operational_readiness.md` 和本矩阵记录验收日期、报告位置及剩余风险。

只通过单元测试、只成功导出一个文件、只确认诱饵字符串未出现，或只观察 UI 健康标签，都不足以单独证明 Sprint 5 完成。