ALTER TABLE knowledge_objects
    ADD COLUMN triage_status TEXT NOT NULL DEFAULT 'inbox'
    CHECK (triage_status IN ('inbox', 'filed'));

ALTER TABLE knowledge_objects ADD COLUMN triaged_at TEXT;

-- Preserve the old Inbox semantics for existing rows without making future AI
-- enrichment remove a newly captured object from Inbox.
UPDATE knowledge_objects
SET triage_status = CASE
        WHEN lifecycle_status IN ('captured', 'parsed') THEN 'inbox'
        ELSE 'filed'
    END,
    triaged_at = CASE
        WHEN lifecycle_status IN ('captured', 'parsed') THEN NULL
        ELSE updated_at
    END;

ALTER TABLE tags ADD COLUMN normalized_name TEXT;
ALTER TABLE tags ADD COLUMN color_token TEXT;
ALTER TABLE tags ADD COLUMN created_at TEXT;
ALTER TABLE tags ADD COLUMN updated_at TEXT;
ALTER TABLE tags ADD COLUMN archived_at TEXT;

UPDATE tags
SET normalized_name = lower(trim(name)),
    created_at = COALESCE(created_at, CURRENT_TIMESTAMP),
    updated_at = COALESCE(updated_at, CURRENT_TIMESTAMP);

ALTER TABLE object_tags
    ADD COLUMN assignment_source TEXT NOT NULL DEFAULT 'user'
    CHECK (assignment_source IN ('user', 'ai_accepted', 'imported', 'rule'));
ALTER TABLE object_tags ADD COLUMN analysis_id TEXT REFERENCES ai_analysis(id) ON DELETE SET NULL;
ALTER TABLE object_tags ADD COLUMN confidence REAL CHECK (confidence IS NULL OR (confidence >= 0 AND confidence <= 1));
ALTER TABLE object_tags ADD COLUMN created_at TEXT;
ALTER TABLE object_tags ADD COLUMN updated_at TEXT;

UPDATE object_tags
SET created_at = COALESCE(created_at, CURRENT_TIMESTAMP),
    updated_at = COALESCE(updated_at, CURRENT_TIMESTAMP);

ALTER TABLE collections ADD COLUMN normalized_name TEXT;
ALTER TABLE collections ADD COLUMN description TEXT;
ALTER TABLE collections ADD COLUMN icon_key TEXT;
ALTER TABLE collections ADD COLUMN color_token TEXT;
ALTER TABLE collections ADD COLUMN query_json TEXT;
ALTER TABLE collections ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;
ALTER TABLE collections ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0 CHECK (is_pinned IN (0, 1));
ALTER TABLE collections ADD COLUMN revision INTEGER NOT NULL DEFAULT 1;
ALTER TABLE collections ADD COLUMN archived_at TEXT;

UPDATE collections SET normalized_name = lower(trim(name));

ALTER TABLE collection_objects
    ADD COLUMN membership_source TEXT NOT NULL DEFAULT 'user'
    CHECK (membership_source IN ('user', 'ai_accepted', 'imported', 'rule'));

CREATE TABLE IF NOT EXISTS tag_suggestions (
    id TEXT PRIMARY KEY,
    object_id TEXT NOT NULL,
    analysis_id TEXT NOT NULL,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    confidence REAL,
    rationale TEXT,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'accepted', 'rejected', 'superseded')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    decided_at TEXT,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    FOREIGN KEY (analysis_id) REFERENCES ai_analysis(id) ON DELETE CASCADE,
    UNIQUE (analysis_id, normalized_name)
);

INSERT OR IGNORE INTO tag_suggestions (
    id, object_id, analysis_id, name, normalized_name, confidence,
    rationale, status, created_at
)
SELECT
    analysis.id || ':legacy:' || MIN(CAST(tags.key AS TEXT)),
    analysis.object_id,
    analysis.id,
    trim(tags.value),
    lower(trim(tags.value)),
    NULL,
    'Imported from a historical AI analysis.',
    'pending',
    analysis.created_at
FROM ai_analysis AS analysis
JOIN json_each(
    CASE WHEN json_valid(analysis.tags_json) THEN analysis.tags_json ELSE '[]' END
) AS tags
WHERE tags.type = 'text'
  AND trim(tags.value) != ''
  AND analysis.id = (
      SELECT latest.id FROM ai_analysis AS latest
      WHERE latest.object_id = analysis.object_id
      ORDER BY latest.created_at DESC, latest.id DESC LIMIT 1
  )
GROUP BY analysis.id, analysis.object_id, lower(trim(tags.value));

CREATE INDEX IF NOT EXISTS idx_knowledge_objects_triage_updated
    ON knowledge_objects(triage_status, updated_at DESC, id DESC)
    WHERE lifecycle_status != 'deleted';
CREATE INDEX IF NOT EXISTS idx_tags_active_normalized
    ON tags(normalized_name) WHERE archived_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_object_tags_tag_object
    ON object_tags(tag_id, object_id);
CREATE INDEX IF NOT EXISTS idx_collections_active_order
    ON collections(user_id, archived_at, is_pinned DESC, sort_order, name);
CREATE INDEX IF NOT EXISTS idx_collection_objects_object
    ON collection_objects(object_id, collection_id);
CREATE INDEX IF NOT EXISTS idx_tag_suggestions_object_status
    ON tag_suggestions(object_id, status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_tag_suggestions_analysis
    ON tag_suggestions(analysis_id);
