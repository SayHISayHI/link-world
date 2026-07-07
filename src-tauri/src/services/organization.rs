use crate::domain::knowledge::KnowledgeObject;
use crate::domain::organization::{
    Collection, CreateCollectionInput, CreateSmartViewInput, LibraryNavigation, LibraryPage,
    LibraryQuery, ObjectOrganization, Tag, UpdateCollectionInput,
};
use crate::errors::AppResult;
use crate::repositories::organization::OrganizationRepository;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct OrganizationService {
    repository: OrganizationRepository,
}

impl OrganizationService {
    pub fn from_state(state: &AppState) -> AppResult<Self> {
        Ok(Self {
            repository: OrganizationRepository::new(state.database()?.pool().clone()),
        })
    }

    #[cfg(test)]
    pub fn new(repository: OrganizationRepository) -> Self {
        Self { repository }
    }

    pub async fn list_objects(
        &self,
        query: LibraryQuery,
    ) -> AppResult<LibraryPage<KnowledgeObject>> {
        self.repository.list_objects(query).await
    }

    pub async fn get_navigation(&self) -> AppResult<LibraryNavigation> {
        self.repository.get_navigation().await
    }

    pub async fn create_collection(&self, input: CreateCollectionInput) -> AppResult<Collection> {
        self.repository.create_collection(input).await
    }

    pub async fn create_smart_view(&self, input: CreateSmartViewInput) -> AppResult<Collection> {
        self.repository.create_smart_view(input).await
    }

    pub async fn update_collection(&self, input: UpdateCollectionInput) -> AppResult<Collection> {
        self.repository.update_collection(input).await
    }

    pub async fn archive_collection(&self, collection_id: &str) -> AppResult<()> {
        self.repository.archive_collection(collection_id).await
    }

    pub async fn add_object_to_collection(
        &self,
        object_id: &str,
        collection_id: &str,
    ) -> AppResult<()> {
        self.repository
            .add_object_to_collection(object_id, collection_id)
            .await
    }

    pub async fn remove_object_from_collection(
        &self,
        object_id: &str,
        collection_id: &str,
    ) -> AppResult<()> {
        self.repository
            .remove_object_from_collection(object_id, collection_id)
            .await
    }

    pub async fn mark_object_triaged(&self, object_id: &str, filed: bool) -> AppResult<()> {
        self.repository.mark_object_triaged(object_id, filed).await
    }

    pub async fn add_user_tag(&self, object_id: &str, name: &str) -> AppResult<Tag> {
        self.repository.add_user_tag(object_id, name).await
    }

    pub async fn remove_object_tag(&self, object_id: &str, tag_id: &str) -> AppResult<()> {
        self.repository.remove_object_tag(object_id, tag_id).await
    }

    pub async fn accept_tag_suggestion(&self, suggestion_id: &str) -> AppResult<Tag> {
        self.repository.accept_tag_suggestion(suggestion_id).await
    }

    pub async fn reject_tag_suggestion(&self, suggestion_id: &str) -> AppResult<()> {
        self.repository.reject_tag_suggestion(suggestion_id).await
    }

    pub async fn get_object_organization(&self, object_id: &str) -> AppResult<ObjectOrganization> {
        self.repository.get_object_organization(object_id).await
    }
}
