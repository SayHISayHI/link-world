use crate::domain::knowledge::KnowledgeObject;
use crate::domain::organization::{
    Collection, CreateCollectionInput, CreateSmartViewInput, LibraryFilters, LibraryNavigation,
    LibraryPage, LibraryQuery, LibraryViewKind, LibraryViewRef, NavigationItem, NewTagSuggestion,
    ObjectOrganization, SmartViewRule, Tag, TagSuggestion, UpdateCollectionInput, LOCAL_USER_ID,
};
use crate::errors::{AppError, AppResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

const DEFAULT_LIMIT: i64 = 30;
const MAX_LIMIT: i64 = 100;
const MAX_NAME_CHARS: usize = 80;
const MAX_DESCRIPTION_CHARS: usize = 280;

#[derive(Debug, Clone)]
pub struct OrganizationRepository {
    pool: SqlitePool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryCursor {
    updated_at: String,
    id: String,
}

#[derive(Debug, Clone)]
pub(crate) enum ResolvedView {
    All,
    Inbox,
    NeedsAttention,
    Collection(String),
    Tag(String),
    Smart(SmartViewRule),
    RecentlyAdded,
}

impl OrganizationRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_objects(
        &self,
        query: LibraryQuery,
    ) -> AppResult<LibraryPage<KnowledgeObject>> {
        validate_filters(&query.filters)?;
        let resolved_view = self.resolve_view(&query.view).await?;
        let cursor = query.cursor.as_deref().map(parse_cursor).transpose()?;
        let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                objects.id,
                objects.user_id,
                objects.object_type,
                objects.title,
                objects.canonical_url,
                objects.source_platform,
                objects.author,
                objects.privacy_level,
                objects.lifecycle_status,
                objects.failure_reason,
                objects.captured_at,
                objects.updated_at
            FROM knowledge_objects AS objects
            WHERE objects.lifecycle_status != 'deleted'
            "#,
        );
        append_view_predicate(&mut builder, &resolved_view);
        append_filter_predicates(&mut builder, &query.filters);
        if let Some(cursor) = &cursor {
            builder
                .push(" AND (objects.updated_at < ")
                .push_bind(cursor.updated_at.clone())
                .push(" OR (objects.updated_at = ")
                .push_bind(cursor.updated_at.clone())
                .push(" AND objects.id < ")
                .push_bind(cursor.id.clone())
                .push("))");
        }
        builder
            .push(" ORDER BY objects.updated_at DESC, objects.id DESC LIMIT ")
            .push_bind(limit + 1);

        let mut rows = builder.build().fetch_all(&self.pool).await?;
        let has_more = rows.len() as i64 > limit;
        if has_more {
            rows.pop();
        }
        let next_cursor = if has_more {
            rows.last()
                .map(|row| {
                    serialize_cursor(&LibraryCursor {
                        updated_at: row.get("updated_at"),
                        id: row.get("id"),
                    })
                })
                .transpose()?
        } else {
            None
        };

        Ok(LibraryPage {
            items: rows.into_iter().map(knowledge_object_from_row).collect(),
            next_cursor,
        })
    }

    pub async fn get_navigation(&self) -> AppResult<LibraryNavigation> {
        let all = self.count_view(&ResolvedView::All).await?;
        let inbox = self.count_view(&ResolvedView::Inbox).await?;
        let needs_attention = self.count_view(&ResolvedView::NeedsAttention).await?;

        let collection_rows = sqlx::query(
            r#"
            SELECT
                collections.id,
                collections.name,
                collections.collection_type,
                collections.icon_key,
                collections.color_token,
                COUNT(collection_objects.object_id) AS object_count
            FROM collections
            LEFT JOIN collection_objects
              ON collection_objects.collection_id = collections.id
            WHERE collections.user_id = ?1
              AND collections.archived_at IS NULL
              AND collections.collection_type = 'manual'
            GROUP BY collections.id
            ORDER BY collections.is_pinned DESC, collections.sort_order, collections.name
            "#,
        )
        .bind(LOCAL_USER_ID)
        .fetch_all(&self.pool)
        .await?;

        let topic_rows = sqlx::query(
            r#"
            SELECT tags.id, tags.name, tags.color_token, COUNT(object_tags.object_id) AS object_count
            FROM tags
            INNER JOIN object_tags ON object_tags.tag_id = tags.id
            INNER JOIN knowledge_objects AS objects ON objects.id = object_tags.object_id
            WHERE tags.archived_at IS NULL
              AND objects.lifecycle_status != 'deleted'
            GROUP BY tags.id
            ORDER BY object_count DESC, tags.name
            LIMIT 50
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let smart_rows = sqlx::query(
            r#"
            SELECT id, name, icon_key, color_token, query_json, revision
            FROM collections
            WHERE user_id = ?1
              AND archived_at IS NULL
              AND collection_type = 'smart'
            ORDER BY is_pinned DESC, sort_order, name
            "#,
        )
        .bind(LOCAL_USER_ID)
        .fetch_all(&self.pool)
        .await?;

        let mut smart_views = vec![
            self.builtin_smart_item("high_value_untested", "High Value, Untested")
                .await?,
            self.builtin_smart_item("needs_verification", "Needs Verification")
                .await?,
            self.builtin_smart_item("recently_added", "Recently Added")
                .await?,
            self.builtin_smart_item("ai_missing", "AI Analysis Missing")
                .await?,
        ];
        for row in smart_rows {
            let id: String = row.get("id");
            let rule = parse_smart_rule(row.get::<Option<String>, _>("query_json"))?;
            smart_views.push(NavigationItem {
                count: self.count_view(&ResolvedView::Smart(rule)).await?,
                id,
                label: row.get("name"),
                kind: LibraryViewKind::Smart,
                icon_key: row.get("icon_key"),
                color_token: row.get("color_token"),
                collection_type: Some("smart".to_string()),
            });
        }

        Ok(LibraryNavigation {
            system_views: vec![
                navigation_item("inbox", "Inbox", inbox, LibraryViewKind::System, "inbox"),
                navigation_item("all", "All", all, LibraryViewKind::System, "library"),
                navigation_item(
                    "needs_attention",
                    "Needs Attention",
                    needs_attention,
                    LibraryViewKind::System,
                    "alert-circle",
                ),
            ],
            collections: collection_rows
                .into_iter()
                .map(|row| NavigationItem {
                    id: row.get("id"),
                    label: row.get("name"),
                    count: row.get("object_count"),
                    kind: LibraryViewKind::Collection,
                    icon_key: row
                        .get::<Option<String>, _>("icon_key")
                        .or_else(|| Some("folder".to_string())),
                    color_token: row.get("color_token"),
                    collection_type: Some(row.get("collection_type")),
                })
                .collect(),
            topics: topic_rows
                .into_iter()
                .map(|row| NavigationItem {
                    id: row.get("id"),
                    label: row.get("name"),
                    count: row.get("object_count"),
                    kind: LibraryViewKind::Tag,
                    icon_key: Some("tag".to_string()),
                    color_token: row.get("color_token"),
                    collection_type: None,
                })
                .collect(),
            smart_views,
        })
    }

    pub async fn create_collection(&self, input: CreateCollectionInput) -> AppResult<Collection> {
        let name = validate_name(&input.name)?;
        let normalized_name = normalize_name(&name);
        validate_description(input.description.as_deref())?;
        ensure_collection_name_available(&self.pool, &normalized_name, None).await?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO collections (
                id, user_id, name, normalized_name, description, collection_type,
                icon_key, color_token, sort_order, is_pinned, revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 'manual', ?6, ?7, 0, 0, 1, ?8, ?8)
            "#,
        )
        .bind(&id)
        .bind(LOCAL_USER_ID)
        .bind(&name)
        .bind(&normalized_name)
        .bind(normalize_optional_text(input.description))
        .bind(normalize_icon(input.icon_key))
        .bind(normalize_color(input.color_token))
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.get_collection(&id).await
    }

    pub async fn create_smart_view(&self, input: CreateSmartViewInput) -> AppResult<Collection> {
        validate_smart_rule(&input.rule)?;
        let name = validate_name(&input.name)?;
        let normalized_name = normalize_name(&name);
        validate_description(input.description.as_deref())?;
        ensure_collection_name_available(&self.pool, &normalized_name, None).await?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let query_json = serde_json::to_string(&input.rule)
            .map_err(|error| AppError::Database(error.to_string()))?;
        sqlx::query(
            r#"
            INSERT INTO collections (
                id, user_id, name, normalized_name, description, collection_type,
                icon_key, query_json, sort_order, is_pinned, revision, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 'smart', 'filter', ?6, 0, 0, 1, ?7, ?7)
            "#,
        )
        .bind(&id)
        .bind(LOCAL_USER_ID)
        .bind(&name)
        .bind(&normalized_name)
        .bind(normalize_optional_text(input.description))
        .bind(query_json)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.get_collection(&id).await
    }

    pub async fn update_collection(&self, input: UpdateCollectionInput) -> AppResult<Collection> {
        let existing = self.get_collection(&input.collection_id).await?;
        let name = input
            .name
            .as_deref()
            .map(validate_name)
            .transpose()?
            .unwrap_or(existing.name.clone());
        let normalized_name = normalize_name(&name);
        validate_description(input.description.as_deref())?;
        ensure_collection_name_available(&self.pool, &normalized_name, Some(&input.collection_id))
            .await?;
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            r#"
            UPDATE collections
            SET name = ?2,
                normalized_name = ?3,
                description = COALESCE(?4, description),
                icon_key = COALESCE(?5, icon_key),
                color_token = COALESCE(?6, color_token),
                is_pinned = COALESCE(?7, is_pinned),
                sort_order = COALESCE(?8, sort_order),
                revision = revision + 1,
                updated_at = ?9
            WHERE id = ?1
              AND user_id = ?10
              AND archived_at IS NULL
              AND revision = ?11
            "#,
        )
        .bind(&input.collection_id)
        .bind(name)
        .bind(normalized_name)
        .bind(normalize_optional_text(input.description))
        .bind(normalize_icon(input.icon_key))
        .bind(normalize_color(input.color_token))
        .bind(input.is_pinned.map(i64::from))
        .bind(input.sort_order)
        .bind(now)
        .bind(LOCAL_USER_ID)
        .bind(input.expected_revision)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            return Err(AppError::PolicyDenied(
                "organization.collection_revision_conflict".to_string(),
            ));
        }
        self.get_collection(&input.collection_id).await
    }

    pub async fn archive_collection(&self, collection_id: &str) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            r#"
            UPDATE collections
            SET archived_at = ?2, revision = revision + 1, updated_at = ?2
            WHERE id = ?1 AND user_id = ?3 AND archived_at IS NULL
            "#,
        )
        .bind(collection_id)
        .bind(now)
        .bind(LOCAL_USER_ID)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            return Err(AppError::ObjectNotFound);
        }
        Ok(())
    }

    pub async fn add_object_to_collection(
        &self,
        object_id: &str,
        collection_id: &str,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        ensure_object_exists(&mut tx, object_id).await?;
        ensure_manual_collection_exists(&mut tx, collection_id).await?;
        sqlx::query(
            r#"
            INSERT INTO collection_objects (
                collection_id, object_id, sort_order, added_at, membership_source
            ) VALUES (?1, ?2, 0, ?3, 'user')
            ON CONFLICT(collection_id, object_id) DO NOTHING
            "#,
        )
        .bind(collection_id)
        .bind(object_id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        mark_filed(&mut tx, object_id, &now).await?;
        insert_organization_audit(
            &mut tx,
            "organization.collection.add_object",
            object_id,
            Some(collection_id),
            &now,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn remove_object_from_collection(
        &self,
        object_id: &str,
        collection_id: &str,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM collection_objects WHERE collection_id = ?1 AND object_id = ?2")
            .bind(collection_id)
            .bind(object_id)
            .execute(&mut *tx)
            .await?;
        insert_organization_audit(
            &mut tx,
            "organization.collection.remove_object",
            object_id,
            Some(collection_id),
            &now,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn mark_object_triaged(&self, object_id: &str, filed: bool) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            r#"
            UPDATE knowledge_objects
            SET triage_status = ?2,
                triaged_at = ?3,
                updated_at = updated_at
            WHERE id = ?1 AND lifecycle_status != 'deleted'
            "#,
        )
        .bind(object_id)
        .bind(if filed { "filed" } else { "inbox" })
        .bind(filed.then_some(now))
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            return Err(AppError::ObjectNotFound);
        }
        Ok(())
    }

    pub async fn add_user_tag(&self, object_id: &str, name: &str) -> AppResult<Tag> {
        let name = validate_name(name)?;
        let normalized_name = normalize_name(&name);
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        ensure_object_exists(&mut tx, object_id).await?;
        let tag = find_or_create_tag(&mut tx, &name, &normalized_name, "user", &now).await?;
        sqlx::query(
            r#"
            INSERT INTO object_tags (
                object_id, tag_id, assignment_source, created_at, updated_at
            ) VALUES (?1, ?2, 'user', ?3, ?3)
            ON CONFLICT(object_id, tag_id) DO UPDATE SET
                assignment_source = 'user',
                analysis_id = NULL,
                confidence = NULL,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(object_id)
        .bind(&tag.id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        mark_filed(&mut tx, object_id, &now).await?;
        insert_organization_audit(
            &mut tx,
            "organization.tag.add",
            object_id,
            Some(&tag.id),
            &now,
        )
        .await?;
        tx.commit().await?;
        Ok(tag)
    }

    pub async fn remove_object_tag(&self, object_id: &str, tag_id: &str) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM object_tags WHERE object_id = ?1 AND tag_id = ?2")
            .bind(object_id)
            .bind(tag_id)
            .execute(&mut *tx)
            .await?;
        insert_organization_audit(
            &mut tx,
            "organization.tag.remove",
            object_id,
            Some(tag_id),
            &now,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn accept_tag_suggestion(&self, suggestion_id: &str) -> AppResult<Tag> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            SELECT object_id, analysis_id, name, normalized_name, confidence
            FROM tag_suggestions
            WHERE id = ?1 AND status = 'pending'
            "#,
        )
        .bind(suggestion_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::ObjectNotFound)?;
        let object_id: String = row.get("object_id");
        let analysis_id: String = row.get("analysis_id");
        let name: String = row.get("name");
        let normalized_name: String = row.get("normalized_name");
        let confidence: Option<f64> = row.get("confidence");
        let tag =
            find_or_create_tag(&mut tx, &name, &normalized_name, "ai_generated", &now).await?;
        sqlx::query(
            r#"
            INSERT INTO object_tags (
                object_id, tag_id, assignment_source, analysis_id, confidence, created_at, updated_at
            ) VALUES (?1, ?2, 'ai_accepted', ?3, ?4, ?5, ?5)
            ON CONFLICT(object_id, tag_id) DO UPDATE SET
                assignment_source = CASE
                    WHEN object_tags.assignment_source = 'user' THEN 'user'
                    ELSE 'ai_accepted'
                END,
                analysis_id = CASE
                    WHEN object_tags.assignment_source = 'user' THEN object_tags.analysis_id
                    ELSE excluded.analysis_id
                END,
                confidence = CASE
                    WHEN object_tags.assignment_source = 'user' THEN object_tags.confidence
                    ELSE excluded.confidence
                END,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&object_id)
        .bind(&tag.id)
        .bind(&analysis_id)
        .bind(confidence)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE tag_suggestions SET status = 'accepted', decided_at = ?2 WHERE id = ?1",
        )
        .bind(suggestion_id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        mark_filed(&mut tx, &object_id, &now).await?;
        insert_organization_audit(
            &mut tx,
            "organization.tag_suggestion.accept",
            &object_id,
            Some(&tag.id),
            &now,
        )
        .await?;
        tx.commit().await?;
        Ok(tag)
    }

    pub async fn reject_tag_suggestion(&self, suggestion_id: &str) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let row = sqlx::query(
            r#"
            UPDATE tag_suggestions
            SET status = 'rejected', decided_at = ?2
            WHERE id = ?1 AND status = 'pending'
            RETURNING object_id
            "#,
        )
        .bind(suggestion_id)
        .bind(&now)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AppError::ObjectNotFound)?;
        let _: String = row.get("object_id");
        Ok(())
    }

    pub async fn get_object_organization(&self, object_id: &str) -> AppResult<ObjectOrganization> {
        let triage_status = sqlx::query_scalar(
            "SELECT triage_status FROM knowledge_objects WHERE id = ?1 AND lifecycle_status != 'deleted'",
        )
        .bind(object_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AppError::ObjectNotFound)?;
        let tags = sqlx::query(
            r#"
            SELECT tags.id, tags.name, tags.normalized_name, tags.source, tags.color_token
            FROM tags
            INNER JOIN object_tags ON object_tags.tag_id = tags.id
            WHERE object_tags.object_id = ?1 AND tags.archived_at IS NULL
            ORDER BY tags.name
            "#,
        )
        .bind(object_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(tag_from_row)
        .collect();
        let collections = sqlx::query(
            r#"
            SELECT
                collections.id, collections.name, collections.description,
                collections.collection_type, collections.icon_key, collections.color_token,
                collections.query_json, collections.sort_order, collections.is_pinned,
                collections.revision
            FROM collections
            INNER JOIN collection_objects ON collection_objects.collection_id = collections.id
            WHERE collection_objects.object_id = ?1
              AND collections.archived_at IS NULL
              AND collections.collection_type = 'manual'
            ORDER BY collections.is_pinned DESC, collections.sort_order, collections.name
            "#,
        )
        .bind(object_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(collection_from_row)
        .collect();
        let tag_suggestions = sqlx::query(
            r#"
            SELECT
                id, object_id, analysis_id, name, normalized_name,
                confidence, rationale, status, created_at
            FROM tag_suggestions
            WHERE object_id = ?1 AND status = 'pending'
            ORDER BY confidence DESC, created_at DESC, name
            "#,
        )
        .bind(object_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(tag_suggestion_from_row)
        .collect();
        Ok(ObjectOrganization {
            object_id: object_id.to_string(),
            triage_status,
            tags,
            collections,
            tag_suggestions,
        })
    }

    pub async fn persist_ai_tag_suggestions(
        tx: &mut Transaction<'_, Sqlite>,
        object_id: &str,
        analysis_id: &str,
        suggestions: &[NewTagSuggestion],
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE tag_suggestions
            SET status = 'superseded', decided_at = ?2
            WHERE object_id = ?1 AND status = 'pending'
            "#,
        )
        .bind(object_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut **tx)
        .await?;
        for suggestion in suggestions {
            if suggestion.object_id != object_id || suggestion.analysis_id != analysis_id {
                return Err(AppError::PolicyDenied(
                    "organization.suggestion_identity_mismatch".to_string(),
                ));
            }
            sqlx::query(
                r#"
                INSERT INTO tag_suggestions (
                    id, object_id, analysis_id, name, normalized_name,
                    confidence, rationale, status, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8)
                ON CONFLICT(analysis_id, normalized_name) DO NOTHING
                "#,
            )
            .bind(&suggestion.id)
            .bind(&suggestion.object_id)
            .bind(&suggestion.analysis_id)
            .bind(&suggestion.name)
            .bind(&suggestion.normalized_name)
            .bind(suggestion.confidence)
            .bind(&suggestion.rationale)
            .bind(&suggestion.created_at)
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }

    pub(crate) async fn resolve_view(&self, view: &LibraryViewRef) -> AppResult<ResolvedView> {
        match view.kind {
            LibraryViewKind::System => match view.id.as_str() {
                "all" => Ok(ResolvedView::All),
                "inbox" => Ok(ResolvedView::Inbox),
                "needs_attention" => Ok(ResolvedView::NeedsAttention),
                _ => Err(AppError::PolicyDenied(
                    "organization.view_invalid".to_string(),
                )),
            },
            LibraryViewKind::Collection => {
                let collection_type: Option<String> = sqlx::query_scalar(
                    "SELECT collection_type FROM collections WHERE id = ?1 AND user_id = ?2 AND archived_at IS NULL",
                )
                .bind(&view.id)
                .bind(LOCAL_USER_ID)
                .fetch_optional(&self.pool)
                .await?;
                match collection_type.as_deref() {
                    Some("manual") => Ok(ResolvedView::Collection(view.id.clone())),
                    Some("smart") => self.resolve_saved_smart_view(&view.id).await,
                    _ => Err(AppError::ObjectNotFound),
                }
            }
            LibraryViewKind::Tag => {
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM tags WHERE id = ?1 AND archived_at IS NULL)",
                )
                .bind(&view.id)
                .fetch_one(&self.pool)
                .await?;
                if exists {
                    Ok(ResolvedView::Tag(view.id.clone()))
                } else {
                    Err(AppError::ObjectNotFound)
                }
            }
            LibraryViewKind::Smart => match view.id.as_str() {
                "high_value_untested" => Ok(ResolvedView::Smart(SmartViewRule {
                    schema_version: 1,
                    object_types: vec![],
                    tag_ids: vec![],
                    minimum_quality: Some(0.7),
                    analysis_state: Some("present".to_string()),
                    evaluation_state: Some("missing".to_string()),
                })),
                "needs_verification" => Ok(ResolvedView::Smart(SmartViewRule {
                    schema_version: 1,
                    object_types: vec![],
                    tag_ids: vec![],
                    minimum_quality: None,
                    analysis_state: Some("present".to_string()),
                    evaluation_state: Some("missing".to_string()),
                })),
                "recently_added" => Ok(ResolvedView::RecentlyAdded),
                "ai_missing" => Ok(ResolvedView::Smart(SmartViewRule {
                    schema_version: 1,
                    object_types: vec![],
                    tag_ids: vec![],
                    minimum_quality: None,
                    analysis_state: Some("missing".to_string()),
                    evaluation_state: None,
                })),
                _ => self.resolve_saved_smart_view(&view.id).await,
            },
        }
    }

    async fn resolve_saved_smart_view(&self, id: &str) -> AppResult<ResolvedView> {
        let query_json: Option<String> = sqlx::query_scalar(
            "SELECT query_json FROM collections WHERE id = ?1 AND user_id = ?2 AND collection_type = 'smart' AND archived_at IS NULL",
        )
        .bind(id)
        .bind(LOCAL_USER_ID)
        .fetch_optional(&self.pool)
        .await?
        .flatten();
        Ok(ResolvedView::Smart(parse_smart_rule(query_json)?))
    }

    async fn count_view(&self, view: &ResolvedView) -> AppResult<i64> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            "SELECT COUNT(*) FROM knowledge_objects AS objects WHERE objects.lifecycle_status != 'deleted'",
        );
        append_view_predicate(&mut builder, view);
        Ok(builder.build_query_scalar().fetch_one(&self.pool).await?)
    }

    async fn builtin_smart_item(&self, id: &str, label: &str) -> AppResult<NavigationItem> {
        let resolved = self
            .resolve_view(&LibraryViewRef {
                kind: LibraryViewKind::Smart,
                id: id.to_string(),
            })
            .await?;
        Ok(NavigationItem {
            id: id.to_string(),
            label: label.to_string(),
            count: self.count_view(&resolved).await?,
            kind: LibraryViewKind::Smart,
            icon_key: Some("filter".to_string()),
            color_token: None,
            collection_type: Some("system_smart".to_string()),
        })
    }

    async fn get_collection(&self, id: &str) -> AppResult<Collection> {
        let row = sqlx::query(
            r#"
            SELECT
                id, name, description, collection_type, icon_key, color_token,
                query_json, sort_order, is_pinned, revision
            FROM collections
            WHERE id = ?1 AND user_id = ?2 AND archived_at IS NULL
            "#,
        )
        .bind(id)
        .bind(LOCAL_USER_ID)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AppError::ObjectNotFound)?;
        Ok(collection_from_row(row))
    }
}

pub(crate) fn append_view_predicate(builder: &mut QueryBuilder<'_, Sqlite>, view: &ResolvedView) {
    match view {
        ResolvedView::All => {}
        ResolvedView::Inbox => {
            builder.push(" AND objects.triage_status = 'inbox'");
        }
        ResolvedView::NeedsAttention => {
            builder.push(
                " AND (objects.lifecycle_status = 'failed' OR EXISTS (SELECT 1 FROM background_jobs AS attention_jobs WHERE attention_jobs.object_id = objects.id AND attention_jobs.status IN ('failed', 'blocked')))",
            );
        }
        ResolvedView::Collection(id) => {
            builder
                .push(" AND EXISTS (SELECT 1 FROM collection_objects WHERE collection_objects.object_id = objects.id AND collection_objects.collection_id = ")
                .push_bind(id.clone())
                .push(")");
        }
        ResolvedView::Tag(id) => {
            builder
                .push(" AND EXISTS (SELECT 1 FROM object_tags WHERE object_tags.object_id = objects.id AND object_tags.tag_id = ")
                .push_bind(id.clone())
                .push(")");
        }
        ResolvedView::RecentlyAdded => {
            builder.push(
                " AND objects.captured_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-7 days')",
            );
        }
        ResolvedView::Smart(rule) => append_smart_rule(builder, rule),
    }
}

fn append_smart_rule(builder: &mut QueryBuilder<'_, Sqlite>, rule: &SmartViewRule) {
    append_string_in(builder, "objects.object_type", &rule.object_types);
    for tag_id in &rule.tag_ids {
        builder
            .push(" AND EXISTS (SELECT 1 FROM object_tags AS smart_tags WHERE smart_tags.object_id = objects.id AND smart_tags.tag_id = ")
            .push_bind(tag_id.clone())
            .push(")");
    }
    if let Some(minimum_quality) = rule.minimum_quality {
        builder
            .push(" AND COALESCE((SELECT quality_score FROM ai_analysis WHERE object_id = objects.id ORDER BY created_at DESC, id DESC LIMIT 1), -1) >= ")
            .push_bind(minimum_quality);
    }
    match rule.analysis_state.as_deref() {
        Some("present") => {
            builder.push(" AND EXISTS (SELECT 1 FROM ai_analysis WHERE object_id = objects.id)")
        }
        Some("missing") => {
            builder.push(" AND NOT EXISTS (SELECT 1 FROM ai_analysis WHERE object_id = objects.id)")
        }
        _ => builder,
    };
    match rule.evaluation_state.as_deref() {
        Some("present") => builder.push(" AND EXISTS (SELECT 1 FROM evaluation_runs WHERE object_id = objects.id)"),
        Some("missing") => builder.push(" AND NOT EXISTS (SELECT 1 FROM evaluation_runs WHERE object_id = objects.id)"),
        Some("failed") => builder.push(" AND EXISTS (SELECT 1 FROM evaluation_runs WHERE object_id = objects.id AND status = 'failed')"),
        _ => builder,
    };
}

pub(crate) fn append_filter_predicates(
    builder: &mut QueryBuilder<'_, Sqlite>,
    filters: &LibraryFilters,
) {
    append_string_in(builder, "objects.object_type", &filters.object_types);
    append_string_in(
        builder,
        "objects.lifecycle_status",
        &filters.lifecycle_statuses,
    );
    append_string_in(builder, "objects.privacy_level", &filters.privacy_levels);
    for tag_id in &filters.tag_ids {
        builder
            .push(" AND EXISTS (SELECT 1 FROM object_tags AS filter_tags WHERE filter_tags.object_id = objects.id AND filter_tags.tag_id = ")
            .push_bind(tag_id.clone())
            .push(")");
    }
    if let Some(minimum) = filters.quality_min {
        builder
            .push(" AND COALESCE((SELECT quality_score FROM ai_analysis WHERE object_id = objects.id ORDER BY created_at DESC, id DESC LIMIT 1), -1) >= ")
            .push_bind(minimum);
    }
    if let Some(maximum) = filters.quality_max {
        builder
            .push(" AND COALESCE((SELECT quality_score FROM ai_analysis WHERE object_id = objects.id ORDER BY created_at DESC, id DESC LIMIT 1), 2) <= ")
            .push_bind(maximum);
    }
}

fn append_string_in(builder: &mut QueryBuilder<'_, Sqlite>, column: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    builder.push(" AND ").push(column).push(" IN (");
    let mut separated = builder.separated(", ");
    for value in values {
        separated.push_bind(value.clone());
    }
    separated.push_unseparated(")");
}

pub(crate) fn validate_filters(filters: &LibraryFilters) -> AppResult<()> {
    if filters.object_types.len() > 20
        || filters.lifecycle_statuses.len() > 20
        || filters.tag_ids.len() > 50
        || filters.privacy_levels.len() > 10
    {
        return Err(AppError::PolicyDenied(
            "organization.filters_too_large".to_string(),
        ));
    }
    if filters
        .quality_min
        .is_some_and(|value| !(0.0..=1.0).contains(&value))
        || filters
            .quality_max
            .is_some_and(|value| !(0.0..=1.0).contains(&value))
        || matches!((filters.quality_min, filters.quality_max), (Some(min), Some(max)) if min > max)
    {
        return Err(AppError::PolicyDenied(
            "organization.quality_filter_invalid".to_string(),
        ));
    }
    Ok(())
}

fn validate_smart_rule(rule: &SmartViewRule) -> AppResult<()> {
    if rule.schema_version != 1 || rule.object_types.len() > 20 || rule.tag_ids.len() > 50 {
        return Err(AppError::PolicyDenied(
            "organization.smart_rule_invalid".to_string(),
        ));
    }
    if rule
        .minimum_quality
        .is_some_and(|value| !(0.0..=1.0).contains(&value))
    {
        return Err(AppError::PolicyDenied(
            "organization.smart_rule_invalid".to_string(),
        ));
    }
    if rule
        .analysis_state
        .as_deref()
        .is_some_and(|value| !matches!(value, "present" | "missing"))
        || rule
            .evaluation_state
            .as_deref()
            .is_some_and(|value| !matches!(value, "present" | "missing" | "failed"))
    {
        return Err(AppError::PolicyDenied(
            "organization.smart_rule_invalid".to_string(),
        ));
    }
    Ok(())
}

fn parse_smart_rule(value: Option<String>) -> AppResult<SmartViewRule> {
    let value = value
        .ok_or_else(|| AppError::PolicyDenied("organization.smart_rule_missing".to_string()))?;
    let rule: SmartViewRule = serde_json::from_str(&value)
        .map_err(|_| AppError::PolicyDenied("organization.smart_rule_invalid".to_string()))?;
    validate_smart_rule(&rule)?;
    Ok(rule)
}

fn validate_name(value: &str) -> AppResult<String> {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.is_empty()
        || value.chars().count() > MAX_NAME_CHARS
        || value.chars().any(char::is_control)
    {
        return Err(AppError::PolicyDenied(
            "organization.name_invalid".to_string(),
        ));
    }
    Ok(value)
}

fn validate_description(value: Option<&str>) -> AppResult<()> {
    if value.is_some_and(|value| {
        value.chars().count() > MAX_DESCRIPTION_CHARS || value.chars().any(char::is_control)
    }) {
        return Err(AppError::PolicyDenied(
            "organization.description_invalid".to_string(),
        ));
    }
    Ok(())
}

pub fn normalize_name(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_icon(value: Option<String>) -> Option<String> {
    value.filter(|value| {
        matches!(
            value.as_str(),
            "folder" | "bookmark" | "code" | "lightbulb" | "filter"
        )
    })
}

fn normalize_color(value: Option<String>) -> Option<String> {
    value.filter(|value| {
        matches!(
            value.as_str(),
            "gray" | "green" | "blue" | "amber" | "red" | "violet"
        )
    })
}

async fn ensure_collection_name_available(
    pool: &SqlitePool,
    normalized_name: &str,
    excluding_id: Option<&str>,
) -> AppResult<()> {
    let duplicate: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM collections
            WHERE user_id = ?1 AND normalized_name = ?2 AND archived_at IS NULL
              AND (?3 IS NULL OR id != ?3)
        )
        "#,
    )
    .bind(LOCAL_USER_ID)
    .bind(normalized_name)
    .bind(excluding_id)
    .fetch_one(pool)
    .await?;
    if duplicate {
        Err(AppError::PolicyDenied(
            "organization.collection_name_exists".to_string(),
        ))
    } else {
        Ok(())
    }
}

async fn ensure_object_exists(tx: &mut Transaction<'_, Sqlite>, object_id: &str) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM knowledge_objects WHERE id = ?1 AND lifecycle_status != 'deleted')",
    )
    .bind(object_id)
    .fetch_one(&mut **tx)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::ObjectNotFound)
    }
}

async fn ensure_manual_collection_exists(
    tx: &mut Transaction<'_, Sqlite>,
    collection_id: &str,
) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM collections WHERE id = ?1 AND user_id = ?2 AND collection_type = 'manual' AND archived_at IS NULL)",
    )
    .bind(collection_id)
    .bind(LOCAL_USER_ID)
    .fetch_one(&mut **tx)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::ObjectNotFound)
    }
}

async fn mark_filed(tx: &mut Transaction<'_, Sqlite>, object_id: &str, now: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE knowledge_objects SET triage_status = 'filed', triaged_at = ?2 WHERE id = ?1",
    )
    .bind(object_id)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn find_or_create_tag(
    tx: &mut Transaction<'_, Sqlite>,
    name: &str,
    normalized_name: &str,
    source: &str,
    now: &str,
) -> AppResult<Tag> {
    if let Some(row) = sqlx::query(
        "SELECT id, name, normalized_name, source, color_token FROM tags WHERE normalized_name = ?1 AND archived_at IS NULL ORDER BY created_at, id LIMIT 1",
    )
    .bind(normalized_name)
    .fetch_optional(&mut **tx)
    .await?
    {
        return Ok(tag_from_row(row));
    }
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO tags (id, name, normalized_name, source, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?5)
        "#,
    )
    .bind(&id)
    .bind(name)
    .bind(normalized_name)
    .bind(source)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(Tag {
        id,
        name: name.to_string(),
        normalized_name: normalized_name.to_string(),
        source: source.to_string(),
        color_token: None,
    })
}

async fn insert_organization_audit(
    tx: &mut Transaction<'_, Sqlite>,
    action: &str,
    object_id: &str,
    related_id: Option<&str>,
    now: &str,
) -> AppResult<()> {
    let metadata = related_id
        .map(|value| serde_json::json!({ "relatedId": value }).to_string())
        .unwrap_or_else(|| "{}".to_string());
    sqlx::query(
        r#"
        INSERT INTO audit_logs (
            id, user_id, actor_type, actor_id, action, object_id, metadata_json, created_at
        ) VALUES (?1, ?2, 'local_user', ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(LOCAL_USER_ID)
    .bind(action)
    .bind(object_id)
    .bind(metadata)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn navigation_item(
    id: &str,
    label: &str,
    count: i64,
    kind: LibraryViewKind,
    icon_key: &str,
) -> NavigationItem {
    NavigationItem {
        id: id.to_string(),
        label: label.to_string(),
        count,
        kind,
        icon_key: Some(icon_key.to_string()),
        color_token: None,
        collection_type: None,
    }
}

fn collection_from_row(row: SqliteRow) -> Collection {
    Collection {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        collection_type: row.get("collection_type"),
        icon_key: row.get("icon_key"),
        color_token: row.get("color_token"),
        query_json: row.get("query_json"),
        sort_order: row.get("sort_order"),
        is_pinned: row.get::<i64, _>("is_pinned") != 0,
        revision: row.get("revision"),
    }
}

fn tag_from_row(row: SqliteRow) -> Tag {
    Tag {
        id: row.get("id"),
        name: row.get("name"),
        normalized_name: row.get("normalized_name"),
        source: row.get("source"),
        color_token: row.get("color_token"),
    }
}

fn tag_suggestion_from_row(row: SqliteRow) -> TagSuggestion {
    TagSuggestion {
        id: row.get("id"),
        object_id: row.get("object_id"),
        analysis_id: row.get("analysis_id"),
        name: row.get("name"),
        normalized_name: row.get("normalized_name"),
        confidence: row.get("confidence"),
        rationale: row.get("rationale"),
        status: row.get("status"),
        created_at: row.get("created_at"),
    }
}

fn knowledge_object_from_row(row: SqliteRow) -> KnowledgeObject {
    KnowledgeObject {
        id: row.get("id"),
        user_id: row.get("user_id"),
        object_type: row.get("object_type"),
        title: row.get("title"),
        canonical_url: row.get("canonical_url"),
        source_platform: row.get("source_platform"),
        author: row.get("author"),
        privacy_level: row.get("privacy_level"),
        lifecycle_status: row.get("lifecycle_status"),
        failure_reason: row.get("failure_reason"),
        captured_at: row.get("captured_at"),
        updated_at: row.get("updated_at"),
    }
}

fn parse_cursor(value: &str) -> AppResult<LibraryCursor> {
    serde_json::from_str(value)
        .map_err(|_| AppError::PolicyDenied("organization.cursor_invalid".to_string()))
}

fn serialize_cursor(cursor: &LibraryCursor) -> AppResult<String> {
    serde_json::to_string(cursor).map_err(|error| AppError::Database(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::Database;

    async fn seed_object(pool: &SqlitePool, id: &str, title: &str, triage: &str) {
        sqlx::query(
            r#"
            INSERT INTO knowledge_objects (
                id, user_id, object_type, title, privacy_level, lifecycle_status,
                triage_status, captured_at, updated_at
            ) VALUES (?1, 'local-user', 'article', ?2, 'personal', 'parsed', ?3, ?4, ?4)
            "#,
        )
        .bind(id)
        .bind(title)
        .bind(triage)
        .bind(format!(
            "2026-07-07T00:00:0{}Z",
            if id.ends_with('1') { 1 } else { 2 }
        ))
        .execute(pool)
        .await
        .expect("object should seed");
    }

    #[tokio::test]
    async fn collections_tags_and_cursor_queries_compose() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let repository = OrganizationRepository::new(database.pool().clone());
        seed_object(database.pool(), "object-1", "First", "inbox").await;
        seed_object(database.pool(), "object-2", "Second", "filed").await;

        let collection = repository
            .create_collection(CreateCollectionInput {
                name: "Research".to_string(),
                description: None,
                icon_key: None,
                color_token: None,
            })
            .await
            .expect("collection should create");
        repository
            .add_object_to_collection("object-1", &collection.id)
            .await
            .expect("membership should create");
        let tag = repository
            .add_user_tag("object-1", "Rust")
            .await
            .expect("tag should create");

        let collection_page = repository
            .list_objects(LibraryQuery {
                view: LibraryViewRef {
                    kind: LibraryViewKind::Collection,
                    id: collection.id,
                },
                ..LibraryQuery::default()
            })
            .await
            .expect("collection should list");
        assert_eq!(collection_page.items.len(), 1);
        assert_eq!(collection_page.items[0].id, "object-1");

        let tag_page = repository
            .list_objects(LibraryQuery {
                view: LibraryViewRef {
                    kind: LibraryViewKind::Tag,
                    id: tag.id,
                },
                limit: Some(1),
                ..LibraryQuery::default()
            })
            .await
            .expect("tag should list");
        assert_eq!(tag_page.items.len(), 1);

        let navigation = repository
            .get_navigation()
            .await
            .expect("navigation should load");
        assert!(navigation
            .collections
            .iter()
            .any(|item| item.label == "Research" && item.count == 1));
        assert!(navigation
            .topics
            .iter()
            .any(|item| item.label == "Rust" && item.count == 1));
    }

    #[tokio::test]
    async fn accepting_ai_suggestion_preserves_user_ownership() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let repository = OrganizationRepository::new(database.pool().clone());
        seed_object(database.pool(), "object-1", "First", "inbox").await;
        sqlx::query(
            "INSERT INTO ai_analysis (id, object_id, analysis_type, schema_version, summary, created_at) VALUES ('analysis-1', 'object-1', 'general_summary', 3, 'summary', '2026-07-07T00:00:03Z')",
        )
        .execute(database.pool())
        .await
        .expect("analysis should seed");
        sqlx::query(
            "INSERT INTO tag_suggestions (id, object_id, analysis_id, name, normalized_name, confidence, status) VALUES ('suggestion-1', 'object-1', 'analysis-1', 'Rust', 'rust', 0.9, 'pending')",
        )
        .execute(database.pool())
        .await
        .expect("suggestion should seed");

        let tag = repository
            .accept_tag_suggestion("suggestion-1")
            .await
            .expect("suggestion should accept");
        assert_eq!(tag.name, "Rust");
        let organization = repository
            .get_object_organization("object-1")
            .await
            .expect("organization should load");
        assert_eq!(organization.tags.len(), 1);
        assert!(organization.tag_suggestions.is_empty());
        assert_eq!(organization.triage_status, "filed");
    }
}
