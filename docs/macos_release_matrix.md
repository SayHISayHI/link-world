# macOS Alpha 发布工程化矩阵

状态: Draft  
适用范围: Node Tide macOS `.app` / `.dmg` release candidates

## 目标

macOS 打包链路必须把当前代码从“开发机可运行”推进到“可以在 macOS 构建机上产出可审计候选包”。公开或商业分发前仍必须完成 Developer ID 签名、Apple notarization、stapling 和真实机安装矩阵。

本矩阵不新增自动更新、Mac App Store、托管模型网关或云同步。

## 构建顺序

在干净 macOS 工作树上执行：

```bash
npm ci
npm run lint
npm run typecheck
npm test
npm run tauri:build:macos
npm run build:cli
npm run package:macos
```

`npm run release:macos` 等价于最后三步串联：Tauri `.app` / `.dmg` build、平台 CLI build、release package assembly。

## 工件要求

每个 macOS release candidate 必须保存：

| 工件 | 要求 |
| --- | --- |
| `.dmg` | 文件名包含 product、version、platform、arch 和 short commit |
| `.app.zip` | 使用 `ditto -c -k --sequesterRsrc --keepParent` 从 `.app` 生成 |
| CLI | `node-tide-cli`，无 `.exe` 后缀 |
| manifest | `release-manifest.json`，包含 commit、schema migration、target、signature、notarization 和 SHA-256 |
| checksums | `SHA256SUMS.txt` 覆盖所有发布文件和 manifest |
| readiness reports | 至少保存 `readiness:alpha`；公开分发还需要 macOS 手工矩阵证据 |

`scripts/package-macos-release.mjs` 拒绝脏工作区、失败 readiness report、commit 不匹配、缺失 `.app` / `.dmg` / CLI / CLI metadata 的候选包。它不会替代签名、公证或真实安装验证。

## 签名与公证

| 场景 | 要求 |
| --- | --- |
| 邀请制内部 Alpha | 可以使用 unsigned build，但 manifest 必须标记 `unsigned_alpha` 或 `notarization_unstapled_or_unverified`，并向测试用户提供可信 SHA-256 |
| 公开下载 / 商业发布 | 必须使用 Developer ID Application 证书签名，并完成 notarization 与 stapling |
| CI secrets | Apple 证书、密码、Team ID、API key 或 app-specific password 只允许进入 CI secret manager |

0.1.x 继续按 ADR-0010 保留 legacy Tauri identifier `com.linkworld.app`；Tauri 的 macOS `.app` 后缀 warning 只能通过另立数据目录/keyring 迁移 ADR 解决。

验收命令：

```bash
codesign --verify --deep --strict --verbose=2 "Node Tide.app"
xcrun stapler validate "Node Tide.app"
xcrun stapler validate "Node Tide_0.1.0_aarch64.dmg"
spctl -a -vv "Node Tide.app"
```

## 验收矩阵

| ID | 范围 | 操作 | 通过标准 |
| --- | --- | --- | --- |
| M-01 | Apple Silicon clean install | 在干净用户目录打开 `.dmg`，拖入 Applications 后启动 | 无损坏提示；首次启动进入 Ready 或可恢复状态 |
| M-02 | Intel compatibility | 在 Intel Mac 或 `macos-13` runner 上构建和 smoke | 构建成功；`.dmg`、`.app.zip`、CLI metadata target 为 `macos-x64` |
| M-03 | Gatekeeper | 下载态 `.dmg` 保留 quarantine 后打开 | 签名/公证 build 无阻断；unsigned Alpha 有明确测试说明和 checksum |
| M-04 | Keychain | 创建、编辑、删除模型 provider API key，重启后读取 | Keychain 保存成功；SQLite 只保存 `keyring:model-provider:<config-id>` |
| M-05 | CLI | 运行 `node-tide-cli --help` 和机器输出 smoke | 无 `.exe` 后缀；退出码和 JSON contract 正确 |
| M-06 | 数据目录 | 非 ASCII / 中文用户名下保存 URL、搜索、备份、支持包 | 路径派生正确；支持包不泄漏本机绝对路径 |
| M-07 | 升级 | 从上一 macOS candidate 覆盖安装 | 数据保留；migration 有 restore point；Keychain 引用仍可解析 |
| M-08 | 卸载 | 删除 app 后重装 | 用户数据默认保留；重新安装可继续打开旧库 |
| M-09 | 网络环境 | 离线、代理、防火墙下保存 URL 和模型连接测试 | 错误稳定映射；不记录 raw secret、URL query 或响应正文 |
| M-10 | Artifact provenance | 复算 SHA-256、检查 manifest、签名和公证状态 | 所有 hash 匹配；manifest commit 为当前 release candidate |

## 明确未覆盖

- Mac App Store sandbox、receipt、provisioning profile。
- 自动更新签名和 delta update。
- MDM 部署、企业 Gatekeeper 白名单。
- iCloud、移动端和多设备同步。
