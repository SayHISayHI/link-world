use crate::domain::knowledge::{
    DeleteObjectMode, DeleteObjectResponse, KnowledgeObject, KnowledgeObjectDetail,
};
use crate::errors::AppResult;
use crate::repositories::knowledge_objects::KnowledgeObjectRepository;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct LibraryService {
    repository: KnowledgeObjectRepository,
}

impl LibraryService {
    pub fn from_state(state: &AppState) -> AppResult<Self> {
        Ok(Self {
            repository: KnowledgeObjectRepository::new(state.database()?.pool().clone()),
        })
    }

    pub async fn list_recent(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
        filter_type: Option<String>,
    ) -> AppResult<Vec<KnowledgeObject>> {
        self.repository
            .list_recent(limit, offset, filter_type)
            .await
    }

    pub async fn get_detail(&self, object_id: &str) -> AppResult<KnowledgeObjectDetail> {
        self.repository.get_detail(object_id).await
    }

    pub async fn delete_object(
        &self,
        object_id: &str,
        mode: DeleteObjectMode,
    ) -> AppResult<DeleteObjectResponse> {
        self.repository.delete_object(object_id, mode).await
    }
}
