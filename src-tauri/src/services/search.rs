use crate::domain::search::SearchResult;
use crate::errors::AppResult;
use crate::repositories::search::SearchRepository;
use crate::state::AppState;
use sqlx::SqlitePool;

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
}
