# Node Tide 开发规范

本文件是 AI 编码工具与人类贡献者的**唯一规范入口**。`CLAUDE.md` 与 `.cursorrules` 只是指向本文件的指针，不要在那里另写规则。

## 1. 项目是什么

Node Tide（拾海）是本地优先的个人信息资产工作台：保存文章 / GitHub 仓库 / Prompt → 解析成 Markdown → AI 提炼 → 人工评估 → 全文检索。

不是书签管理器：核心价值在「可复核的判断」（evaluation 产出 verdict / evidence / limitations / artifacts）而不是收藏。

## 2. 架构红线（不可协商）

- **Local-first**：未配置模型、断网、云端不可用时，保存、解析、阅读、FTS 检索必须照常工作。外部依赖只能降级，不能阻塞主链路。
- **前后端边界**：前端不得直接访问 SQLite、文件系统或凭据；一切通过 Tauri command 走 `invokeCommand`（`src/lib/tauri.ts`），不得直接 `import { invoke }`。
- **职责分层**：command 只做校验、调用 service、映射错误；service 拥有状态流转与事务边界；repository 拥有 SQL。
- **长任务**：一律走 `background_jobs`，不得在 command 里做长阻塞操作。
- **数据真源**：`parsed_documents` 是解析文本唯一真源；FTS / 向量 / 缓存都是可重建的派生物。
- **隐私**：`secret` / `sensitive` 对象在未取得显式授权前禁止发送给第三方模型；日志、错误消息、支持包里不得出现正文、URL query、API key。
- **凭据**：API key 只进 OS 凭据库（Windows Credential Manager）；非 Windows 开发态回退到 `<app_data>/secrets.json`（0600），仅限本地开发。
- **可追溯**：每次模型调用必须写 `ai_traces`（provider / model / tokens / 耗时）。
- **删除**：破坏性操作必须可审计、可确认，并清理派生数据（tombstone + purge job）。
- **存储 ABI**：`link-world.*` 类文件与目录名已冻结，禁止"顺手改名"，见 `docs/storage_abi.md`。

## 3. 技术栈（当前真实状态）

| 层 | 选型 | 备注 |
| --- | --- | --- |
| 桌面宿主 | Tauri v2 | 桌面三平台，发布目标仍是 Windows |
| 后端 | Rust（1.88+，edition 2021） | 见 `src-tauri/Cargo.toml` |
| 前端 | React 18 + Vite 6 + TypeScript（strict） | 禁止 `any`（约定，非 lint 强制） |
| 状态 | Zustand | 全局状态 + persist |
| 样式 | Tailwind CSS 3 + 自建 `src/components/ui` 原语 | 未引入 shadcn 依赖，风格对齐 shadcn |
| 图标 | lucide-react | |
| 本地库 | SQLite + FTS5（sqlx） | **向量检索未实现**，`sqlite-vec` 只在 deferred 设计里 |
| 模型接入 | `ModelProvider` adapter + capability | OpenAI 兼容 / Anthropic / 本地 Ollama |
| 包管理 | pnpm 10（`packageManager` 固定） | 不要提交 `package-lock.json` |

新增核心依赖前先确认必要性并在 PR 里说明；不要为了单个小功能引入重型库。

## 4. 动手前必须读的文档

| 场景 | 文档 |
| --- | --- |
| 全景 / 任何改动 | `docs/README.md`（索引，含状态分区） |
| 架构原则 | `docs/architecture.md` |
| Rust / Tauri 后端 | `docs/backend_architecture.md`、`docs/state_machines.md` |
| React 前端 | `docs/frontend_architecture.md`、`docs/ui_components_inventory.md` |
| IPC 契约 | `docs/api_contracts.md`（类型真源是 `src/types/api.ts`） |
| 数据模型 / 迁移 | `docs/database_schema.sql`、`docs/database_migrations.md`、`src-tauri/migrations/` |
| 备份 / 恢复 / 删除 | `docs/backup_and_restore.md` |
| 安全与隐私 | `docs/security_and_privacy_policies.md`、`docs/security_privacy_compliance.md` |
| 外部调用 / 模型 | `docs/external_api_resilience.md` |
| 测试 | `docs/testing_strategy.md` |
| 打包 / CI / 发布 | `docs/devops_and_ci.md`、`docs/operational_readiness.md` |
| 冻结的磁盘标识 | `docs/storage_abi.md` |
| 基础架构取舍 | `docs/adr/`（先看是否已有决策） |

`docs/deferred/` 下的同步与插件设计**未实现**，只能当设计意图读，不能当行为依据。

## 5. 常用命令

```bash
# 依赖与前端质量门
pnpm install --frozen-lockfile
pnpm run lint          # eslint --max-warnings=0
pnpm run typecheck     # tsc --noEmit
pnpm test              # vitest
pnpm run build         # tsc + vite build
pnpm run test:e2e      # 先 build 再 playwright（需 pnpm run test:e2e:install）

# 桌面应用（WSL/Linux 需要 webkit2gtk-4.1 等系统库，见 README）
pnpm tauri dev
pnpm tauri build --debug --bundles deb

# Rust 质量门（CI 用 1.88）
cd src-tauri
cargo +1.88.0 fmt --all -- --check
cargo +1.88.0 clippy --locked --all-targets -- -D warnings
cargo +1.88.0 test --locked -- --test-threads=1   # 必须单线程
```

- 单线程是硬要求：restore interruption fixtures 共享进程级恢复状态，并行会互相干扰。
- `scripts/*.ps1`（`pnpm run readiness:*`、`test:alpha-feedback-contract`、`build:cli`）是 **Windows + PowerShell** 发布门禁，Linux 上跑不了，不要因为跑不过就改逻辑。
- 提交前至少跑：`pnpm run lint && pnpm run typecheck && pnpm test`，改 Rust 再跑 clippy + test。

## 6. 测试要求

新增实现要配对应测试或 fixture：

- 状态流转、迁移、错误映射、隐私策略测试（Rust）；
- 前端 loading / empty / failed / 边界状态测试；
- 错误码与脱敏断言：错误消息里不得出现路径、正文、token；
- 修 bug 时先补能复现的测试，再修。

**不要谎报结果**：没跑过的命令不要写成"已验证"；构建或测试失败必须读完报错再动手。

## 7. 变更纪律

- 改契约 / schema / 状态机 / 安全策略 / 发布流程时，同一提交内同步文档。
- 重大架构取舍要加或更新 `docs/adr/`；ADR 记录"为什么"，主文档记录"现在怎么做"。
- 不创建 `v2` / `new` / `final` / `updated` 之类副本；就地更新主文档。
- 未实现的设计放 `docs/deferred/` 并登记到 `docs/deferred/README.md`。
- 代码与文档冲突时，先解决冲突再继续写代码。
- 遇到文档未定义且涉及基础架构的决策，先问，不要自行猜测。

## 8. UI 与代码质量

- 复杂 CSS（z-index、绝对定位）要有系统级方案；同一个 bug 修两次引入新问题就推翻重写，回到最简结构。
- 对接外部 API 不臆想参数，严格校验数据（`Array.isArray(x) && x.length > 0` 而不是 `if (x)`）。
- 后端状态机必须覆盖 `failed` 并给出用户可读的 `failure_reason`。
- TypeScript 类型严谨，异常分支优先处理。
- 避免 `unsafe`；确有必要时写 SAFETY 注释说明不变量。
