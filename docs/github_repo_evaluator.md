# GitHub Repo Evaluator

本文档定义 Week 7 GitHub Repo Evaluator 的数据来源、执行边界、评分、降级和隐私约束。实现入口为 `services/github.rs`（外部 adapter）和 `services/github_evaluator.rs`（纯评分）。

## 1. 当前交付

- `github_repo_evaluator` 使用 `network_optional` execution kind。
- 没有网络、没有 token、被限流或元数据缺失时，仍可使用已保存正文完成带 limitation 的评估。
- 不 clone 仓库、不执行代码、不安装依赖、不调用模型或 sandbox。
- Evaluation 总上限 15 秒；外部元数据预算最多占 80%，且不超过 12 秒，为本地评分和持久化保留时间。
- 可选 `GITHUB_TOKEN` 仅通过 `SecretStore` 的 `env:GITHUB_TOKEN` 引用解析，不写 SQLite、artifact、trace、日志或 IPC。

## 2. 官方 API 契约

请求固定使用：

- `Accept: application/vnd.github+json`；README 使用 `application/vnd.github.raw+json`。
- `X-GitHub-Api-Version: 2022-11-28`。
- 有界 `User-Agent`。
- 仅访问 `api.github.com`，测试通过显式注入的 loopback base URL，产品命令不接受自定义 base URL。

接口：

1. `GET /repos/{owner}/{repo}`：公开仓库 metadata、license、stars、forks、open issues、archive/disabled 状态、`pushed_at`。
2. `GET /repos/{owner}/{repo}/readme`：README 原始字节只在内存中提取 install/usage/example/security signals 和 SHA-256；正文不持久化。
3. `GET /repos/{owner}/{repo}/releases/latest`：最新公开 release；404 是“无 release”，不是运行失败。

依据：

- [Get a repository](https://docs.github.com/en/rest/repos/repos#get-a-repository)
- [Repository contents and README](https://docs.github.com/en/rest/repos/contents)
- [REST API rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)
- [Repository licenses](https://docs.github.com/en/rest/licenses/licenses)

## 3. 有界数据模型

允许进入 Evaluation output/artifact 的公开 metadata：

- owner/repo、description（最多 512 字符）、default branch、primary language。
- topics 最多 20 个，每项最多 64 字符。
- stars、forks、open issues。
- archived、disabled、fork。
- pushed timestamp、license name/SPDX、latest release tag/time/prerelease。
- README byte length、SHA-256 和布尔 signals。
- 是否使用认证及稳定 limitation code；永不保存 token。

响应上限：普通 JSON 512 KiB，README 256 KiB。先校验 `Content-Length`，流式读取时再次累计校验；越界为 `github.response_too_large`。

## 4. 隐私策略

| privacy level | GitHub metadata |
| --- | --- |
| `public` / `personal` | 允许公开 metadata 查询并写 trace |
| `sensitive` | 仅 metadata 查询；不发送已保存正文、标题、批注或 AI output |
| `secret` | 禁止外部查询，使用本地 saved-content fallback，记录 `github.policy_denied` |

即使配置 token，adapter 也只评估返回 `private=false` 的公开仓库。404 无法可靠区分不存在和无权限，统一为 `github.not_found_or_private`；明确返回 private 时为 `github.private_repository`，两者都不复制响应 body。

## 5. 限流和失败语义

稳定降级码：

- `github.auth_failed`
- `github.forbidden`
- `github.invalid_repository`
- `github.not_found_or_private`
- `github.private_repository`
- `github.policy_denied`
- `github.rate_limited`
- `github.response_too_large`
- `github.timeout`
- `github.unavailable`
- `github.invalid_response`

403/429 且 `X-RateLimit-Remaining: 0`，或存在 `Retry-After`，统一为 `github.rate_limited`。发生后立即停止 README/release 等后续可选请求，不自动重试，也不读取或记录 error body；由用户在窗口恢复后显式 retry。

主 repository metadata 不可用时，Evaluation 使用 saved-content fallback。README/release 子请求失败时保留已成功的 repository metadata，并把稳定 code 加入 limitations。

## 6. 评分边界

维度：

- documentation
- licensing
- maintenanceSignals
- adoptionSignals
- actionability
- riskPosture

stars/forks 只提供有上限的 adoption context：即使数值极高，也不能单独产生高价值结论，不能挽救 archived/disabled repository。archive/disabled 强制 `low_value`，并要求比较活跃替代方案。

每个外部判断必须对应 `external_check` evidence：README、license、activity、adoption context、repository state。缺失项必须进入 limitation，不得伪造成已验证事实。

## 7. 自动化证据

- 固定公开 fixture：无 token 完成 repo/README/release 三次请求，输出稳定 external evidence 和 15 秒 trace 上限。
- README 正文诱饵不进入 output、artifact metadata 或 structured logs。
- secret object 不发网络请求，只返回 policy limitation。
- rate limit 后不发送后续 release 请求。
- 百万 stars 的 archived fixture 仍为 `low_value`。
- 网络、评分、持久化分别测试；真实 GitHub API 和 Windows 发布候选仍需矩阵留证。
