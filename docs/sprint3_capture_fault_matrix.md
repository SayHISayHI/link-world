# Sprint 3 采集与后台任务故障矩阵

状态: Draft，自动化门禁已建立；真实网络、进程中断和并发矩阵仍需在发布候选包上执行。
适用范围: Sprint 3 URL capture、统一解析、后台任务恢复、失败分类、重复 URL 幂等和诊断脱敏验收。

## 1. 目标与边界

Sprint 3 的核心承诺是：第三方页面不可达、拒绝访问、响应异常或应用被中断时，已保存的本地对象仍可读；失败任务进入稳定、可恢复的状态，不阻塞其他任务，也不把正文或凭据写入失败原因和诊断证据。

本矩阵分成两层：

- 自动化门禁：运行 `npm run readiness:sprint3`，覆盖确定性解析、失败分类、任务隔离、启动收敛、AI failure reason 和脱敏边界。
- 真实故障矩阵：使用 Windows 发布候选安装包、受控本地 HTTP fixture 和明确的网络切换执行；覆盖真实 DNS、断网、20 秒 fetch timeout、进程强制终止和并发调度。

自动化测试不能替代真实网络栈、Windows 电源/进程终止和安装包路径验证。真实矩阵不得对不受控第三方站点制造高频、超时或故障流量。

## 2. 自动化门禁

默认命令：

```powershell
npm run readiness:sprint3
```

脚本顺序执行：

- Rust 格式检查。
- capture parser、HTTP/受限页/空正文分类、任务隔离、URL 幂等和诱饵秘密脱敏测试。
- queued/running job 启动收敛、retry budget 和手动 retry 测试。
- AI job 稳定 failure reason 与本地持久化失败测试。
- Rust clippy warnings gate。

默认报告写入系统临时目录，包含命令、退出码、耗时和最多 80 行日志尾部。发布候选分支必须把 JSON 保存为 CI artifact 或发布验收证据。

可选参数：

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/sprint3-readiness.ps1 -OutputPath C:\tmp\link-world-sprint3-readiness.json
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/sprint3-readiness.ps1 -IncludeFrontend
```

`-IncludeFrontend` 仅在依赖完整时启用，用于执行 typecheck 和前端测试；后端可靠性门禁不因本机前端依赖缺失而被误报为通过。

## 3. 受控测试准备

准备一个只绑定 `127.0.0.1` 的 HTTP fixture，至少提供：

| 路径 | 行为 |
| --- | --- |
| `/ok` | 返回含标题和正文的 UTF-8 HTML |
| `/forbidden` | 返回 HTTP 403 |
| `/retryable` | 返回 HTTP 429 |
| `/server-error` | 返回 HTTP 503 |
| `/slow` | 接受连接后至少 25 秒不返回完整响应 |
| `/empty` | 返回只有导航/脚本、无可读正文的 HTML |
| `/malformed` | 返回标签不闭合但含可读正文的损坏 HTML |
| `/challenge` | 返回明显 CAPTCHA/challenge/verification 页面 |
| `/oversized` | 返回超过 5 MiB 的 HTML |

fixture 日志只记录路径、状态码、耗时和 request id，不记录 header、cookie、query 或 body。DNS 失败使用保留的 `.invalid` 域名；断网测试通过 Windows 网络适配器切换完成。

## 4. 真实 Windows/网络矩阵

| 编号 | 场景 | 步骤 | 必须证明 |
| --- | --- | --- | --- |
| W3-01 | 离线启动 | 联网保存 3 个对象，完全断网后重启应用 | Library、详情和 FTS 可用；启动不依赖模型或 URL 服务 |
| W3-02 | 真实断网 capture | 断网后保存一个新的 HTTPS URL | 对象保留且最终为 failed；原因是稳定 `capture.network_unreachable` 或 `capture.timeout`；其他本地操作可继续 |
| W3-03 | DNS 不存在 | 保存 `https://link-world-readiness.invalid/article` | 最终状态稳定且可重试；失败原因不含 resolver 原始 payload、本机 DNS 配置或完整错误链 |
| W3-04 | HTTP 拒绝访问 | 保存受控 `/forbidden` | 对象为 failed；显示 `capture.http_forbidden` 对应动作并建议浏览器扩展；不自动循环重试 |
| W3-05 | fetch timeout | 保存受控 `/slow` 并等待超过 20 秒 | 得到 `capture.timeout`；应用 UI 和后续任务不被冻结 |
| W3-06 | 空正文 | 保存受控 `/empty` | 得到 `capture.no_readable_text`；不创建 parsed document；提供 selection/扩展回退 |
| W3-07 | 损坏 HTML | 保存受控 `/malformed` | 若存在可读正文则容错解析；否则稳定 `capture.parse_failed`，不得崩溃或永久 running |
| W3-08 | challenge 页面 | 保存受控 `/challenge` | 得到 `capture.restricted_page`；不把 challenge body 写入失败原因 |
| W3-09 | 超大页面 | 保存受控 `/oversized` | 得到 `capture.too_large`；不持久化超过限制的 snapshot |
| W3-10 | 单任务失败隔离 | 先提交 `/forbidden`，紧接着提交 `/ok` | 前者失败，后者 parsed；失败任务不阻塞队列 |
| W3-11 | running 时强制结束 | capture job 进入 running 后结束主进程并重启 | 有 retry budget 的 capture job 回 queued；耗尽 job 为 failed；无永久 running |
| W3-12 | 手动 retry | 对 failed capture 执行 retry，再把 fixture 切为 `/ok` | attempt/lock/error 被重置，原对象成功 parsed；产生 retry audit |
| W3-13 | 重复 URL 幂等 | 连续保存 host 大小写、默认端口、fragment 不同但 canonical 等价的 URL | 只保留一个有效对象/snapshot/job；UI 明确显示 already saved |
| W3-14 | 10 个并发 capture | 混合提交 6 个 `/ok`、2 个 `/forbidden`、1 个 `/slow`、1 个 `/empty` | 每个对象独立收敛到 parsed/failed；无永久 running、死锁或全局饥饿 |
| W3-15 | 失败证据脱敏 | fixture 返回含诱饵 body、`cookie=session-secret` 和 `token=provider-secret` 的错误；检查 Diagnostics、DB failure reason、domain event 和日志 | 证据只含稳定 code/status/action；诱饵正文、cookie、token 和完整 URL query/fragment 均不存在 |
| W3-16 | 无模型配置 | 删除默认模型配置，完成 `/ok` capture | capture 仍完成解析；AI job 可独立失败/跳过，但不得把对象从 parsed 降级 |

## 5. 证据要求

每次发布候选保存：

- 应用版本、commit SHA、Windows 版本、安装包类型和 fixture 版本。
- `npm run readiness:sprint3` JSON 报告。
- W3-01 至 W3-16 的 pass/fail、执行时间、执行人、对象/job ID、关键截图和脱敏日志尾部。
- W3-05 的实际超时时间，W3-11 的终止阶段，W3-14 的最终状态计数。
- 失败项的修复 commit、release blocker 或有负责人和到期日的 waiver。

证据禁止包含用户正文、source snapshot、API key、token、cookie、session、完整 URL query/fragment、credential reference 或本机绝对数据路径。

## 6. 完成边界

Sprint 3 从“执行中”切换到“完成”必须同时满足：

- 自动化门禁在发布候选 commit 上通过。
- W3-01 至 W3-13、W3-15、W3-16 在 Windows 11 主目标上通过。
- W3-14 至少执行三轮且每轮所有任务收敛；不得以平均成功率替代单对象终态证据。
- 前端依赖完整环境中的 capture failure/retry/duplicate UI 测试通过。
- `/docs/post_mvp_roadmap.md` 和 `/docs/operational_readiness.md` 记录验收日期、报告位置和剩余风险。

只通过单元测试、只观察 UI 提示、或只确认数据库没有 `running` 行，都不足以单独证明 Sprint 3 完成。