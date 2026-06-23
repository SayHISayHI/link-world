# Link World 数据库迁移规范

状态: Draft  
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

## 5. Backup Strategy

Before high-risk migration:

1. Checkpoint WAL.
2. Create SQLite backup file.
3. Record app version and schema version.
4. Verify backup file exists and is readable.
5. Run migration.
6. If migration fails, keep original and backup untouched.

Backup path must be visible in diagnostics.

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

## 10. Failure Handling

On migration failure:

- stop app startup before normal use.
- show user-readable error.
- show backup path if created.
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
