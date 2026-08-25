use std::sync::{Arc, Mutex};

use macro_user_id::{cowlike::CowLike, user_id::MacroUserIdStr};
use model::document::response::DocumentResponseMetadata;

use super::*;
use crate::domain::ports::create::{DocumentBytesUpload, DocumentBytesUploadPort};
use crate::domain::response::{DocumentResponse, DocumentResponseMetadataWithContent};

#[derive(Default)]
struct FakeService {
    published: Mutex<Vec<(String, String)>>,
    cleaned: Mutex<Vec<String>>,
}

impl DocumentCreationService for FakeService {
    async fn create_document(
        &self,
        user_id: MacroUserIdStr<'static>,
        _args: CreateDocumentRepoArgs,
        _job_id: Option<String>,
    ) -> Result<CreateDocumentResponseData, DocumentError> {
        Ok(CreateDocumentResponseData {
            document_response: DocumentResponse {
                document_metadata: DocumentResponseMetadataWithContent::new(
                    DocumentResponseMetadata {
                        document_id: "document-id".into(),
                        document_version_id: 1,
                        owner: user_id,
                        document_name: "task".into(),
                        file_type: Some("md".into()),
                        sha: Some(EMPTY_SHA256.into()),
                        branched_from_id: None,
                        branched_from_version_id: None,
                        document_family_id: None,
                        document_bom: None,
                        modification_data: None,
                        created_at: None,
                        updated_at: None,
                        sub_type: Some(document_sub_type::DocumentSubType::Task),
                    },
                    DocumentContent::pending(),
                ),
                presigned_url: None,
            },
            content_type: "text/markdown".into(),
            file_type: Some("md".into()),
        })
    }
    async fn handle_task_properties(
        &self,
        _user_id: MacroUserIdStr<'static>,
        _document_id: &str,
        _request: &CreateTaskRequest,
    ) -> Result<(), DocumentError> {
        Ok(())
    }
    async fn mark_document_uploaded(&self, _document_id: &str) -> Result<(), DocumentError> {
        Ok(())
    }
    async fn set_document_content(
        &self,
        _document_id: &str,
        _content: DocumentContent,
    ) -> Result<(), DocumentError> {
        Ok(())
    }
    fn publish_task_created(&self, document_id: &str, project_id: &str) {
        self.published
            .lock()
            .unwrap()
            .push((document_id.into(), project_id.into()));
    }
    async fn cleanup_created_document(&self, document_id: &str) {
        self.cleaned.lock().unwrap().push(document_id.into());
    }
}

struct FakeMarkdown(bool);
impl MarkdownInitializationPort for FakeMarkdown {
    async fn initialize_existing_markdown(
        &self,
        _document_id: &str,
        _markdown: &str,
    ) -> Result<Vec<u8>, DocumentError> {
        if self.0 {
            Err(DocumentError::BadRequest("initialization failed".into()))
        } else {
            Ok(vec![1, 2, 3])
        }
    }
}
struct UnusedUpload;
impl DocumentBytesUploadPort for UnusedUpload {
    async fn upload_document_bytes(
        &self,
        _upload: DocumentBytesUpload,
    ) -> Result<(), DocumentError> {
        unreachable!()
    }
}

fn user() -> MacroUserIdStr<'static> {
    MacroUserIdStr::parse_from_str("macro|task@example.com")
        .unwrap()
        .into_owned()
}
fn task(project_id: Option<uuid::Uuid>) -> NewMarkdownTextDocument {
    NewMarkdownTextDocument {
        metadata: match project_id {
            Some(id) => NewDocumentMetadata::builder("task").project_id(id).build(),
            None => NewDocumentMetadata::new("task"),
        },
        markdown: "body".into(),
        subtype: MarkdownSubtype::Task {
            property_values: None,
            share_with_team: false,
            team_id: None,
        },
    }
}

#[tokio::test]
async fn publishes_task_only_after_finalization_succeeds() {
    let service = Arc::new(FakeService::default());
    let project_id = uuid::Uuid::new_v4();
    let creator = DocumentCreator::new(service.clone(), FakeMarkdown(false), UnusedUpload);
    creator
        .create_markdown_text(user(), task(Some(project_id)))
        .await
        .unwrap();
    assert_eq!(
        service.published.lock().unwrap().as_slice(),
        &[("document-id".into(), project_id.to_string())]
    );
    assert!(service.cleaned.lock().unwrap().is_empty());
}

#[tokio::test]
async fn finalization_failure_cleans_up_and_suppresses_event() {
    let service = Arc::new(FakeService::default());
    let creator = DocumentCreator::new(service.clone(), FakeMarkdown(true), UnusedUpload);
    assert!(
        creator
            .create_markdown_text(user(), task(Some(uuid::Uuid::new_v4())))
            .await
            .is_err()
    );
    assert!(service.published.lock().unwrap().is_empty());
    assert_eq!(service.cleaned.lock().unwrap().as_slice(), &["document-id"]);
}

#[tokio::test]
async fn non_task_and_projectless_task_do_not_publish() {
    let service = Arc::new(FakeService::default());
    let creator = DocumentCreator::new(service.clone(), FakeMarkdown(false), UnusedUpload);
    creator
        .create_markdown_text(
            user(),
            NewMarkdownTextDocument::empty_note(NewDocumentMetadata::new("note")),
        )
        .await
        .unwrap();
    creator
        .create_markdown_text(user(), task(None))
        .await
        .unwrap();
    assert!(service.published.lock().unwrap().is_empty());
}
