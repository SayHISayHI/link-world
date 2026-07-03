# ADR-0008: Share application services between the desktop app and CLI

Status: Proposed  
Date: 2026-07-03

## Context

Link World 当前通过 Tauri commands 暴露本地能力。总体架构已经把 CLI 列为客户端，但代码库没有 CLI binary、机器输出契约或桌面/CLI 并发访问同一数据目录的规则。若直接把 Tauri command 逐个复制成 CLI handler，会产生两套参数校验、业务编排、错误映射和隐私边界，并增加状态机漂移风险。

Windows 桌面宿主当前生成 `link-world.exe`，release build 还使用 Windows GUI subsystem。CLI 需要 console subsystem，不能以同名二进制覆盖桌面宿主。Local Edition 的 migration、restore、FTS rebuild 和对象存储操作也要求明确的单写入者边界。

## Decision

1. 新增独立的 `link-world-cli.exe` binary，桌面宿主和 CLI 都链接现有 Rust library。
2. Tauri commands 与 CLI 都是 adapter；两者只能调用共享 application service、policy、repository、job、event 和 telemetry，不能互相调用或复制业务逻辑。
3. 首期 CLI 直接打开本地数据目录，但桌面端和 CLI 必须通过跨进程 runtime lock 实现单写入者/单运行者互斥。竞争失败立即返回稳定的 `ERR_RUNTIME_BUSY`。
4. help、version 和 shell completion 不初始化本地数据。生产首期不开放任意数据目录参数。
5. CLI 提供人类文本和版本化 JSON 两种输出；stdout、stderr、退出码和 error envelope 构成公开自动化契约。
6. CLI 复用现有 privacy、secret、audit、job、correlation 和 structured logging 边界。API Key 不允许作为普通命令参数。
7. 首期不引入 daemon 或通过本地 HTTP/IPC 远程控制正在运行的桌面端。若真实使用证明需要并发，另立 ADR。

## Consequences

Positive:

- GUI、CLI 和未来受控 Agent 接口共享同一业务语义。
- 可通过 JSON、退出码和 correlation ID 建立可重复自动化。
- 独立 console binary 不改变桌面 executable 的 subsystem 和启动体验。
- 互斥策略使 migration、restore 和对象存储边界在首期保持简单、可验证。

Negative:

- 首期用户运行 CLI 前需要退出桌面端。
- 部分 service/state 装配需要去除对 Tauri handle/window 的不必要依赖。
- 安装器、签名、PATH、completion 和双 binary 升级增加发布矩阵。
- JSON 输出一旦发布就需要版本兼容治理。

## Alternatives Considered

### Copy Tauri commands into CLI handlers

Rejected。它会复制业务编排和安全判断，并让 GUI/CLI 状态机逐渐漂移。

### Add CLI flags to the desktop executable

Rejected for the first release。Windows GUI subsystem 与 console 交互、同名安装 artifact 和 Tauri 启动生命周期会使行为难以预测。

### Make the desktop app a daemon and CLI an IPC client

Deferred。它支持并发和单一数据拥有者，但需要进程发现、认证、协议版本、生命周期与恢复设计，超过首期自动化需求。

### Expose a localhost HTTP API

Rejected for the first release。它新增端口、认证、CSRF/来源验证和驻留服务攻击面。

## Revisit When

- Alpha/CLI 用户必须在桌面端保持运行时调用自动化。
- MCP 或其他客户端需要共享长生命周期任务和实时进度。
- Local Edition 引入多进程 worker 或常驻 background service。
- CLI 需要远程数据目录、Cloud Edition 或团队身份。
