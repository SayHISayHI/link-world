# ADR-0010: Node Tide（拾海）产品身份与 legacy 存储兼容

Status: Accepted
Date: 2026-07-10

## Context

项目在首次外部 Alpha 之前由 Link World 更名为 Node Tide，中文产品名为“拾海”。旧名称已经同时进入 UI、npm/Cargo 包名、桌面和 CLI 二进制、浏览器扩展、发布脚本，以及 Windows app data、SQLite、结构化日志、runtime lock、Credential Manager 和前端本地偏好等持久化标识。

直接替换所有持久化标识会让现有用户数据、备份和 API Key 看起来消失，也会破坏旧安装包升级、CLI/桌面互斥和恢复边界。因此产品身份更新必须与存储 ABI 迁移分开处理。

## Decision

1. 可见中文品牌使用“拾海”，英文品牌使用“Node Tide”，需要同时展示时使用“拾海 · Node Tide”。
2. 工程 slug、npm/Cargo package、桌面 binary、CLI binary、浏览器扩展内部消息前缀和新发布工件统一使用 `node-tide` / `NodeTide` / `NODE_TIDE`。
3. CLI 正式名称为 `node-tide-cli`，安装脚本为 `install-node-tide-cli.ps1`。
4. 新开发环境变量使用 `NODE_TIDE_DATA_DIR` 和 `NODE_TIDE_<PROVIDER>_API_KEY`；读取端继续接受对应的 `LINK_WORLD_*` legacy 变量作为低优先级回退。
5. 0.1.x 保留以下 legacy 存储 ABI，直到另立数据迁移 ADR 并完成 restore point、双向兼容和真实升级矩阵：
   - Tauri identifier 和默认 app data 目录：`com.linkworld.app`；
   - SQLite 文件：`link-world.sqlite3` 及 WAL/SHM sidecar；
   - runtime lock：`link-world.lock`；
   - structured log：`link-world.jsonl`；
   - Credential Manager service：`com.linkworld.app.model-provider`；
   - 前端持久化偏好 key：`link-world-ui`。
6. 旧 commit `98c9b0f` 的 Alpha 证据作为 former Link World 品牌的历史记录保留，不改写原始工件名称。Node Tide 必须在干净的新 commit 上重新生成 readiness、安装包、CLI、manifest、checksum 和真实 Windows 证据。

## Consequences

- 现有数据库、对象、备份、凭据和 UI 偏好在品牌升级后继续可见。
- 用户可见界面和新分发工件不再使用旧品牌；诊断或文件系统中可能仍看到明确记录的 legacy 存储文件名。
- 公开发布前必须验证旧安装原地升级到 Node Tide、桌面/CLI 继续竞争同一 runtime lock、Credential Manager 凭据可读取、备份可验证和恢复。
- 仓库托管平台上的 repository slug 可独立改名；本 ADR 不要求在本地重命名用户当前工作目录。

## Alternatives Considered

- 立即更换 Tauri identifier 和全部存储文件名：拒绝，因为缺少安全的跨目录、keyring 和备份迁移协议。
- 永久保留所有旧工程名：拒绝，因为会继续把 former brand 暴露到新二进制、扩展和发布工件。

## Revisit When

- 需要移除 `com.linkworld.app` 等 legacy 标识；
- 引入稳定版安装器、自动更新或跨平台数据目录迁移；
- 真实升级矩阵证明新旧目录、凭据和备份可以安全切换。
