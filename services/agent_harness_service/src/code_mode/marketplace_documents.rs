//! Creates the HTML documents marketplace copies make, through the documents
//! domain's in-process upload: the path `POST /documents/upload` takes.

use documents::domain::{
    create::upload::NewFileUpload, models::DocumentError, ports::create::DocumentUploadService,
};
use macro_user_id::user_id::MacroUserIdStr;
use marketplace::domain::{models::MarketplaceError, ports::Documents};
use model_owner::CreationPrincipal;
use uuid::Uuid;

#[cfg(test)]
mod test;

/// Name a copy gets when its listing's title has nothing a file name keeps.
const FALLBACK_NAME: &str = "Marketplace copy";

/// Uploads a copy as an `.html` file owned by the person making it.
pub struct UploadedDocuments<U> {
    uploads: U,
}

impl<U> UploadedDocuments<U> {
    /// Bind the documents upload service.
    pub fn new(uploads: U) -> Self {
        Self { uploads }
    }
}

/// `name` as a file name the upload takes: no directory separators or
/// control characters, trimmed, and `.html` on the end.
fn html_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|character| match character {
            '/' | '\\' => '-',
            character if character.is_control() => ' ',
            character => character,
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    let stem = if cleaned.is_empty() {
        FALLBACK_NAME
    } else {
        cleaned
    };
    format!("{stem}.html")
}

fn marketplace_error(error: DocumentError) -> MarketplaceError {
    match error {
        DocumentError::BadRequest(message) => MarketplaceError::Invalid(message),
        DocumentError::NameTooLong { max } => {
            MarketplaceError::Invalid(format!("a document name is at most {max} characters"))
        }
        error => MarketplaceError::Internal(rootcause::report!("{error}").into_dynamic()),
    }
}

impl<U> Documents for UploadedDocuments<U>
where
    U: DocumentUploadService + 'static,
{
    #[tracing::instrument(err, skip_all, name = "marketplace.create_html_document")]
    async fn create_html(
        &self,
        user: &MacroUserIdStr<'static>,
        name: &str,
        html: &str,
    ) -> Result<Uuid, MarketplaceError> {
        let created = self
            .uploads
            .upload_file(
                &CreationPrincipal::User(user.clone()),
                NewFileUpload {
                    file_name: html_file_name(name),
                    bytes: html.as_bytes().to_vec(),
                    project: None,
                },
            )
            .await
            .map_err(marketplace_error)?;

        Uuid::parse_str(created.document_id()).map_err(|error| {
            MarketplaceError::Internal(
                rootcause::report!("documents answered an invalid id: {error}").into_dynamic(),
            )
        })
    }
}
