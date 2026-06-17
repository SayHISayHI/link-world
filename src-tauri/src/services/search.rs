use crate::domain::search::{RebuildSearchIndexResponse, ReindexObjectResponse, SearchResult};
use crate::errors::AppResult;
use crate::repositories::search::SearchRepository;
use crate::state::AppState;
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SearchService {
    repository: SearchRepository,
}

impl SearchService {
    pub fn from_state(state: &AppState) -> AppResult<Self> {
        Ok(Self::new(state.database()?.pool().clone()))
    }

    pub fn new(pool: SqlitePool) -> Self {
        Self {
            repository: SearchRepository::new(pool),
        }
    }

    pub async fn search_hybrid(
        &self,
        query: &str,
        limit: Option<i64>,
    ) -> AppResult<Vec<SearchResult>> {
        self.repository.search_hybrid(query, limit).await
    }

    pub async fn rebuild_search_index(&self) -> AppResult<RebuildSearchIndexResponse> {
        let job_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let indexed_objects = self
            .repository
            .rebuild_index_with_job(&job_id, &now)
            .await?;

        Ok(RebuildSearchIndexResponse {
            job_id,
            indexed_objects,
        })
    }

    pub async fn reindex_object(&self, object_id: &str) -> AppResult<ReindexObjectResponse> {
        let job_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let indexed = self
            .repository
            .reindex_object_with_job(object_id, &job_id, &now)
            .await?;

        Ok(ReindexObjectResponse {
            job_id,
            object_id: object_id.to_string(),
            indexed,
        })
    }
}
