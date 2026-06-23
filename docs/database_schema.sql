-- Link World 基础数据库模型定义 (SQLite)
-- 包含核心实体表、解析正文表、AI Trace、Evaluation Engine、全文搜索表 (FTS5)
-- 以及向量检索表 (sqlite-vec)。
--
-- 注意：实际项目中这些 SQL 会被拆分到 src-tauri/migrations/ 下。

PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;

-- 1. 核心知识对象表
CREATE TABLE IF NOT EXISTS knowledge_objects (
    id TEXT PRIMARY KEY,               -- UUID
    user_id TEXT NOT NULL,             -- Local Edition 也保留 user_id，便于后续同步和多 profile
    object_type TEXT NOT NULL,         -- 'article', 'prompt', 'github_repo', etc.
    title TEXT,
    canonical_url TEXT,
    source_platform TEXT,
    author TEXT,
    privacy_level TEXT NOT NULL CHECK (
        privacy_level IN ('public', 'personal', 'sensitive', 'secret')
    ),
    lifecycle_status TEXT NOT NULL CHECK (
        lifecycle_status IN (
            'captured', 'parsed', 'enriched', 'evaluated',
            'triaged', 'archived', 'deleted', 'failed'
        )
    ),
    failure_reason TEXT,               -- lifecycle_status = 'failed' 时记录可展示错误
    captured_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_knowledge_objects_user_status
    ON knowledge_objects(user_id, lifecycle_status, updated_at);

CREATE INDEX IF NOT EXISTS idx_knowledge_objects_type
    ON knowledge_objects(object_type);

-- 2. 原始快照表
-- source_snapshots 只保存来源快照和指针，不作为解析后正文的唯一来源。
CREATE TABLE IF NOT EXISTS source_snapshots (
    id TEXT PRIMARY KEY,
    object_id TEXT NOT NULL,
    snapshot_type TEXT NOT NULL,       -- 'html', 'markdown', 'text', 'pdf_blob', 'json', 'screenshot'
    storage_uri TEXT NOT NULL,         -- 本地文件路径指针，如 'local://objects/abc.html'
    content_hash TEXT NOT NULL,        -- SHA-256，用于防篡改和去重
    parser_id TEXT,
    parser_version TEXT,
    captured_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_source_snapshots_object
    ON source_snapshots(object_id, captured_at);

-- 3. 解析后正文表
-- 这是详情页、FTS、chunk、AI input 的正文 source of truth。
CREATE TABLE IF NOT EXISTS parsed_documents (
    id TEXT PRIMARY KEY,
    object_id TEXT NOT NULL,
    source_snapshot_id TEXT,
    title TEXT,
    text_content TEXT NOT NULL,
    markdown_content TEXT,
    language TEXT,
    word_count INTEGER,
    content_hash TEXT NOT NULL,
    parser_id TEXT NOT NULL,
    parser_version TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    FOREIGN KEY (source_snapshot_id) REFERENCES source_snapshots(id) ON DELETE SET NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_parsed_documents_object_hash
    ON parsed_documents(object_id, content_hash);

CREATE INDEX IF NOT EXISTS idx_parsed_documents_object
    ON parsed_documents(object_id, created_at);

-- 4. AI 分析与摘要表
CREATE TABLE IF NOT EXISTS ai_analysis (
    id TEXT PRIMARY KEY,
    object_id TEXT NOT NULL,
    parsed_document_id TEXT,
    analysis_type TEXT NOT NULL,       -- 'general_summary', 'repo_analysis', 'prompt_analysis', etc.
    schema_version INTEGER NOT NULL DEFAULT 1,
    summary TEXT,
    category TEXT,
    tags_json TEXT,                    -- JSON 字符串数组
    key_points_json TEXT,
    claims_json TEXT,
    action_items_json TEXT,
    risks_json TEXT,
    quality_score REAL,
    confidence REAL,
    display_hints_json TEXT,            -- nullable AIDisplayHintsV1 sidecar; added by migration 0002
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    FOREIGN KEY (parsed_document_id) REFERENCES parsed_documents(id) ON DELETE SET NULL
);

-- 新分析使用 schema_version = 2，并可写入 display_hints_json。
-- 旧 schema_version = 1 记录保持 NULL，不执行数据回填。

CREATE INDEX IF NOT EXISTS idx_ai_analysis_object_created
    ON ai_analysis(object_id, created_at);

-- 5. AI Trace 表
-- 任何 AI 输出都必须可追踪到模型、prompt、输入快照、成本和版本。
CREATE TABLE IF NOT EXISTS ai_traces (
    id TEXT PRIMARY KEY,
    analysis_id TEXT,
    object_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    capability TEXT NOT NULL CHECK (
        capability IN ('chat', 'embedding', 'rerank', 'vision')
    ),
    prompt_template_id TEXT,
    prompt_template_version TEXT,
    input_snapshot_id TEXT,
    input_parsed_document_id TEXT,
    input_hash TEXT,
    output_hash TEXT,
    prompt_tokens INTEGER,
    completion_tokens INTEGER,
    estimated_cost_usd REAL,
    latency_ms INTEGER,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (analysis_id) REFERENCES ai_analysis(id) ON DELETE CASCADE,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    FOREIGN KEY (input_snapshot_id) REFERENCES source_snapshots(id) ON DELETE SET NULL,
    FOREIGN KEY (input_parsed_document_id) REFERENCES parsed_documents(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_ai_traces_object
    ON ai_traces(object_id, created_at);

-- 6. 评估验证表 (Evaluation Engine 的核心产物)
CREATE TABLE IF NOT EXISTS evaluation_runs (
    id TEXT PRIMARY KEY,
    object_id TEXT NOT NULL,
    evaluator_type TEXT NOT NULL,      -- 'prompt_evaluator', 'repo_evaluator', etc.
    evaluator_version TEXT NOT NULL,
    status TEXT NOT NULL CHECK (
        status IN ('planned', 'running', 'passed', 'failed', 'skipped', 'blocked')
    ),
    plan_json TEXT,                    -- EvaluationPlan
    input_json TEXT,
    output_json TEXT,
    dimensions_json TEXT,              -- novelty, utility, actionability, etc.
    evidence_json TEXT,
    limitations_json TEXT,
    next_actions_json TEXT,
    score REAL,
    verdict TEXT NOT NULL CHECK (
        verdict IN ('high_value', 'useful', 'situational', 'low_value', 'unsafe', 'unknown')
    ),
    failure_reason TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_evaluation_runs_object_created
    ON evaluation_runs(object_id, created_at);

-- 7. 评估产物表
CREATE TABLE IF NOT EXISTS evaluation_artifacts (
    id TEXT PRIMARY KEY,
    evaluation_run_id TEXT NOT NULL,
    artifact_type TEXT NOT NULL,       -- 'log', 'screenshot', 'diff', 'test_output', 'generated_prompt', 'report'
    storage_uri TEXT NOT NULL,
    content_hash TEXT,
    metadata_json TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (evaluation_run_id) REFERENCES evaluation_runs(id) ON DELETE CASCADE
);

-- 8. 标签与集合表
CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('user', 'ai_generated', 'imported'))
);

CREATE TABLE IF NOT EXISTS object_tags (
    object_id TEXT NOT NULL,
    tag_id TEXT NOT NULL,
    PRIMARY KEY (object_id, tag_id),
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS collections (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    name TEXT NOT NULL,
    collection_type TEXT NOT NULL DEFAULT 'manual', -- 'manual', 'smart', 'system'
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS collection_objects (
    collection_id TEXT NOT NULL,
    object_id TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    added_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (collection_id, object_id),
    FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE
);

-- 9. 全文索引表 (FTS5)
-- FTS 是派生索引，不是正文 source of truth。
-- 写入 parsed_documents 和 ai_analysis 后，由应用层或触发器同步维护。
CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_fts USING fts5(
    object_id UNINDEXED,
    parsed_document_id UNINDEXED,
    title,
    author,
    content,
    ai_summary,
    tokenize='unicode61'
);

-- 10. 向量索引表 (sqlite-vec)
-- sqlite-vec 的 vec0 表通过 rowid 关联普通 metadata 表。
-- 插入时先创建 vector_chunks_meta，获得 chunk_id 后：
-- INSERT INTO vec_chunks(rowid, embedding) VALUES (:chunk_id, :embedding);
CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(
    embedding float[1536]
);

CREATE TABLE IF NOT EXISTS vector_chunks_meta (
    chunk_id INTEGER PRIMARY KEY,
    object_id TEXT NOT NULL,
    parsed_document_id TEXT,
    chunk_index INTEGER NOT NULL,
    chunk_text TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    embedding_provider TEXT,
    embedding_model TEXT,
    embedding_dimensions INTEGER NOT NULL DEFAULT 1536,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    FOREIGN KEY (parsed_document_id) REFERENCES parsed_documents(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_vector_chunks_meta_object
    ON vector_chunks_meta(object_id, chunk_index);

-- 11. 对象关系表
-- 支撑“相关旧收藏”、知识图谱、重复内容、引用关系。
CREATE TABLE IF NOT EXISTS object_relations (
    id TEXT PRIMARY KEY,
    from_object_id TEXT NOT NULL,
    to_object_id TEXT NOT NULL,
    relation_type TEXT NOT NULL,       -- 'related', 'duplicate', 'supports', 'contradicts', 'references', 'derived_from'
    source TEXT NOT NULL,              -- 'user', 'ai', 'parser', 'import'
    confidence REAL,
    metadata_json TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (from_object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    FOREIGN KEY (to_object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_object_relations_unique
    ON object_relations(from_object_id, to_object_id, relation_type, source);

-- 12. 后台任务表
CREATE TABLE IF NOT EXISTS background_jobs (
    id TEXT PRIMARY KEY,
    job_type TEXT NOT NULL,
    status TEXT NOT NULL CHECK (
        status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled', 'blocked')
    ),
    object_id TEXT,
    payload_json TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 3,
    next_run_at DATETIME,
    locked_at DATETIME,
    locked_by TEXT,
    last_error TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_background_jobs_status_next
    ON background_jobs(status, next_run_at);

CREATE INDEX IF NOT EXISTS idx_background_jobs_object
    ON background_jobs(object_id, created_at);

-- 13. 领域事件表
-- Local Edition 可作为持久化 outbox；Cloud Edition 可映射到事件总线。
CREATE TABLE IF NOT EXISTS domain_events (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    event_version INTEGER NOT NULL DEFAULT 1,
    user_id TEXT NOT NULL,
    object_id TEXT,
    causation_id TEXT,
    correlation_id TEXT,
    payload_json TEXT NOT NULL,
    occurred_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    processed_at DATETIME
);

CREATE INDEX IF NOT EXISTS idx_domain_events_unprocessed
    ON domain_events(processed_at, occurred_at);

CREATE INDEX IF NOT EXISTS idx_domain_events_object
    ON domain_events(object_id, occurred_at);

-- 14. 插件 manifest 与权限
CREATE TABLE IF NOT EXISTS plugin_manifests (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    plugin_kind TEXT NOT NULL CHECK (
        plugin_kind IN ('connector', 'parser', 'evaluator', 'model_provider', 'sync_provider', 'exporter')
    ),
    manifest_json TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 0,
    installed_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS plugin_permissions (
    id TEXT PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    permission_kind TEXT NOT NULL,
    scope TEXT,
    required INTEGER NOT NULL DEFAULT 0,
    granted INTEGER NOT NULL DEFAULT 0,
    granted_at DATETIME,
    FOREIGN KEY (plugin_id) REFERENCES plugin_manifests(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_plugin_permissions_plugin
    ON plugin_permissions(plugin_id);

-- 15. 模型配置
-- api_key 不进入此表；这里只保存 keychain/secret store 的引用。
CREATE TABLE IF NOT EXISTS model_provider_configs (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    chat_base_url TEXT,
    embeddings_base_url TEXT,
    default_chat_model TEXT,
    default_embedding_model TEXT,
    capabilities_json TEXT NOT NULL,
    secret_ref TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 16. 删除 tombstone
CREATE TABLE IF NOT EXISTS deletion_tombstones (
    id TEXT PRIMARY KEY,
    object_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    deletion_mode TEXT NOT NULL CHECK (
        deletion_mode IN ('soft_delete', 'purge', 'export_then_delete')
    ),
    purge_status TEXT NOT NULL CHECK (
        purge_status IN ('pending', 'running', 'completed', 'failed')
    ),
    reason TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME
);

CREATE INDEX IF NOT EXISTS idx_deletion_tombstones_status
    ON deletion_tombstones(purge_status, created_at);

-- 17. 本地设置
CREATE TABLE IF NOT EXISTS local_settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 18. 审计日志
CREATE TABLE IF NOT EXISTS audit_logs (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    actor_type TEXT NOT NULL,          -- 'user', 'system', 'plugin', 'agent'
    actor_id TEXT,
    action TEXT NOT NULL,
    object_id TEXT,
    metadata_json TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_user_created
    ON audit_logs(user_id, created_at);
