# 平台源适配器

状态: Current

部分平台的正文不在匿名抓取能看到的 DOM 里：它们把内容以内嵌 JSON 或移动端片段下发给客户端。适配器直接读这份载荷，因此拿到的是平台自己的原始文本与原图 URL，而不是重新排版后的猜测。

实现目录：`src-tauri/src/services/sources/`。

## 1. 覆盖范围

| 平台 | 数据位置 | 拿到什么 | 已知限制 |
| --- | --- | --- | --- |
| X / Twitter | `api.fxtwitter.com/<user>/status/<id>`，失败回退 `cdn.syndication.twimg.com/tweet-result` | 完整推文、作者、全部图片与视频原址 | 纯转推/引用推文本以接口返回为准；受保护账号不可见 |
| 小红书 | 笔记页内嵌 `window.__INITIAL_STATE__` → `note.noteDetailMap[id].note` | 标题、正文、`imageList` 原图、视频流 | 登录墙或风控页会返回校验壳，此时适配器报错并回退 |
| 抖音 | 分享短链跳转后的 `iesdouyin.com/share/video/<id>` 内嵌 `window._ROUTER_DATA` | `desc`、图文帖原图、视频播放地址 | 同上；私密作品不可见 |
| 知乎 | 移动端页面 `div.RichText` | 标题、作者、正文、正文内图片原址 | 盐选/登录墙返回空正文 → 适配器返回 `None`，交给通用抓取报错 |

## 2. 处理顺序

`CaptureService::fetch_and_parse_job` 先按 URL 匹配适配器：

1. 匹配成功且解析成功 → 用适配器结果构造 `FetchedHtmlDocument`，原始载荷作为 source snapshot 保存，便于事后核对。
2. 匹配成功但失败（结构变化、风控、内容已删）→ **静默回退**到通用 HTML 抓取管线，不阻塞用户。
3. 无适配器匹配 → 通用管线。

`SourceAsset` 记录原图/视频 URL，按出现顺序写进 Markdown（`![image](url)` / `[video](url)`），因此详情页能直接看到原图。

## 3. 失败语义

| 情况 | 行为 |
| --- | --- |
| 平台结构改版 | 适配器返回 `Err(ParseFailed)`，回退通用抓取；用户看到的是通用失败原因，不是编造内容 |
| 登录墙 / 私有内容 | 同上，绝不返回空壳或占位内容 |
| 无媒体但有正文 | 小红书/抖音要求至少一个媒体资源，否则视为解析失败 |

原则：**宁可失败，不要产出错误的"原文"**。

## 4. 已知缺口

- 需要登录态的内容（小红书部分笔记、知乎盐选、受保护推文）目前无法解析。计划是让用户按域注入 cookie（加密存 keyring），只在适配器请求中使用。
- 图片/视频仅保存**原始 URL**，尚未下载到本地对象存储。原始 URL 会过期或被防盗链拒绝，长期保存需要落盘（`CaptureAsset` 结构与 `write_capture_artifact` 已就绪）。
- 适配器无速率限制与缓存；同一 URL 重复采集会重复请求。

## 5. 怎么验证适配器

不要用真实采集来试错——有一个不写库、不上锁、不需要关闭 App 的探针命令：

```bash
cd src-tauri && cargo build --bin node-tide-cli
./target/debug/node-tide-cli sources probe "<链接>"
./target/debug/node-tide-cli --output json sources probe "<链接>"   # 看完整字段
```

输出包含：匹配到的适配器、标题、作者、正文长度、媒体数量与前 10 条媒体 URL、正文摘要、Markdown 预览。失败时返回 `ERR_PARSE_FAILED`，此时通用抓取管线会接手。

当前实测状态：

| 平台 | 状态 | 说明 |
| --- | --- | --- |
| X | 已实测通过 | `api.fxtwitter.com` 与 syndication 均返回 200，文字路径确认；媒体路径由单元测试覆盖 |
| 小红书 | 待真实链接验证 | 已修正：必须带分享链接里的 `xsec_token`，裸 `/explore/<id>` 会 404 |
| 抖音 | 本机无法验证 | 开发环境到 `douyin.com` / `iesdouyin.com` 全部连接失败，需在能访问的网络里跑探针 |
| 知乎 | 需 cookie | 匿名请求返回 403 |

## 6. 新增适配器

1. 在 `services/sources/` 新建模块，实现 `SourceAdapter`（`id` / `matches` / `capture`）。
2. 解析函数保持纯函数（输入载荷字符串 → `SourceCapture`），便于用 fixture 写单测。
3. 在 `services/sources/mod.rs` 的 `adapters()` 注册，顺序即匹配优先级。
4. 测试至少覆盖：URL 匹配、正常载荷、结构损坏、无正文/无媒体。
5. 在本文件第 1 节登记平台与已知限制。
