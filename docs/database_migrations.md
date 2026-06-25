# Link World 数据库迁移规范

状态: Active baseline
适用范围: SQLite / sqlx migrations / Local Edition

## 1. Purpose

数据库迁移是桌面 Local-first 产品的高风险区域。用户数据在本地，迁移失败会直接影响信任。本文档规定迁移命名、执行、测试、回滚和兼容策略。

## 2. Migration Principles

- 永远假设用户数据库不可丢。
- migration 必须幂等地从上一版本升级。
- destructive migration 必须先备份。
- 迁移失败必须停止启动并展示恢复信息。
- 不允许静默删除用户数据。
- FTS/vector/cache 可重建，核心表不可随意重写。

## 3. Versioning

Migration 文件命名：

```text
src-tauri/migrations/
├── 0001_initial_schema.sql
├── 0002_ai_display_hints.sql
├── 0003_model_provider_api_family.sql
└── ...
```

Rules:

- 编号递增，不重排。
- 已发布 migration 不修改，只新增。
- 每个 migration 顶部写目的、风险、回滚说明。
- schema version 写入本地设置或 sqlx migration metadata。

## 4. Allowed Changes

Low risk:

- add nullable column.
- add table.
- add index.
- add enum-like value when app handles unknown values.

Medium risk:

- add NOT NULL column with default.
- rebuild derived index.
- backfill derived data.

High risk:

- rename column/table.
- split table.
- change primary key.
- delete column.
- rewrite source snapshots or parsed documents.

High-risk migration requires:

- backup.
- fixture test.
- explicit release note.
- rollback plan.

## 5. Startup Migration Guard

Production startup must not let SQLx mutate an existing user database before a recoverable restore point exists. `AppState` therefore initializes storage in this order:

1. Initialize the object-store path and connect SQLite without running migrations.
2. Inspect `_sqlx_migrations`; reject incomplete, unknown, or checksum-mismatched versions before any schema write.
3. If the database is fresh and contains no user tables, run pending migrations without creating an empty backup.
4. If an existing user schema has pending migrations, create and fully verify a normal restore point containing the SQLite `VACUUM INTO` snapshot and object store.
5. Write `migration/guard.prepared.json`, then rename it to `guard.running.json` immediately before running SQLx migrations.
6. Run migrations, `quick_check`, and `foreign_key_check`; on success remove the guard and write a bounded, redacted `last-result.json`.

Crash convergence is fail-closed:

- `guard.prepared.json` means the verified restore point exists and migration has not started; startup re-verifies it before proceeding.
- `guard.running.json` plus pending migrations means a previous attempt may have partially executed; automatic retry is blocked and the error retains the verified backup ID.
- `guard.running.json` with no pending migration means schema commit completed but cleanup was interrupted; startup validates integrity and completes the guard.
- Guard/result files are limited to 64 KiB and contain identifiers, versions, status, and timestamps only. They do not expose content, credential values, or absolute paths.

The verified restore point is deliberately retained after success. Automatic restore is not attempted because migration rollback semantics are schema-specific; recovery requires an explicit user action through the startup recovery UI.

## 6. Transaction Policy

- Each migration should run in a transaction when SQLite allows it.
- Long backfills should be chunked and resumable if they can be expensive.
- External network calls are forbidden during migration.
- Migration must not call model providers.

## 7. FTS and Vector Migration

FTS/vector rules:

- Treat `knowledge_fts`, `vec_chunks`, `vector_chunks_meta` as rebuildable.
- If schema changes affect parsed text, enqueue reindex jobs.
- Do not rely on FTS content as source data.
- sqlite-vec migrations must be feature-flag aware.

## 8. Data Compatibility

Unknown values:

- App should handle unknown lifecycle/job/evaluator statuses defensively.
- Unknown plugin permission should be denied by default.
- Unknown privacy level should be treated as most restrictive.

Old data:

- Old `ai_analysis` remains visible.
- Migration `0002_ai_display_hints.sql` only adds nullable `ai_analysis.display_hints_json`; it does not backfill or rewrite existing analysis rows.
- Migration `0003_model_provider_api_family.sql` adds a required `model_provider_configs.api_family` with the backward-compatible `openai_chat_completions` default; existing provider rows continue using their previous behavior.
- Analysis schema version 1 with `display_hints_json IS NULL` must deserialize as `displayHints: undefined` and use deterministic Markdown AST layout inference.
- Newly generated general analyses use analysis schema version 2; the nested display hint has its own independent schema version 1.
- Old `evaluation_runs` remain visible.
- New prompt schema does not overwrite old output.

## 9. Migration Test Matrix

Every migration must be tested against:

- empty DB.
- previous release DB.
- DB with 1000 objects.
- DB with failed jobs.
- DB with sensitive object.
- DB with parsed documents.
- DB with AI traces.
- previous release DB with schema version 1 AI analyses and no display hint column.
- upgraded DB containing both legacy analyses without display hints and schema version 2 analyses with valid or invalid hints.
- DB with evaluation artifacts.
- DB with tombstones.

### 9.1 当前自动化基线

历史 fixture 在测试运行时由正式 `sqlx::migrate!` 源截断生成，并保留真实 `_sqlx_migrations` version/checksum；仓库不保存易发生版本漂移的 SQLite 二进制 fixture。

- 0001 fixture：1000 个对象，并覆盖 sensitive/secret、snapshot、parsed document、legacy AI analysis、AI trace、Evaluation artifact、失败 job、FTS、provider、tombstone 和 local setting。
- 0001 → latest：验证所有核心/派生行数、隐私级别、FTS 可检索性、外键、`display_hints_json IS NULL` 兼容和 provider 默认 API family。
- 0002 → latest：验证已有 display hints 原样保留，历史 provider identity 不改写。
- 0003 → latest：验证重复启动幂等，非默认 `anthropic_messages` 不回退。
- future version：注入未知 migration 999 后必须返回 `DbMigration`，用户对象和 migration metadata 不被重写。

自动化 fixture 只证明 schema/data compatibility，不等同于发布包升级回归。仍需：

- 每次新增 migration 时把上一已发布版本加入矩阵。
- Windows 安装包从上一 Alpha 原地升级。
- 普通应用启动的 restore-point guard 已实现并覆盖 fresh DB、existing v1 DB、running 中断阻断和 migration 已提交后的收敛；启动 recovery UI 已有组件测试，真实安装升级场景仍需回归。

## 10. Failure Handling

On migration failure:

- stop app startup before normal use.
- expose a restricted startup recovery UI before opening normal Library state or background services.
- show user-readable error.
- surface the verified backup ID without exposing an absolute path.
- write redacted log.
- never attempt repeated destructive migration automatically.

## 11. Rollback Policy

Rollback is not guaranteed for every schema change. Instead:

- patch releases should avoid schema changes.
- minor releases can add columns/tables.
- major releases can require explicit migration confirmation.
- destructive changes require backup and export path.

## 12. Developer Checklist

Before adding a migration:

- Is the change required by API/schema docs?
- Is it additive?
- Does it affect syncable fields?
- Does it affect deletion purge?
- Does it affect FTS/vector rebuild?
- Does it require backfill?
- Are fixtures updated?
- Is release note required?

### Credential reference compatibility

- 历史 `memory:model_provider:*` 引用没有可恢复的持久 secret，升级后按“未配置凭据”处理，不阻止配置列表加载。
- 用户下一次保存 API Key 时写入 `keyring:model-provider:<config-id>`；迁移不得尝试从日志、配置表或环境中推断旧明文。
- 历史 provider row id 原样保留；新配置使用 UUID，不做破坏性主键重写。
