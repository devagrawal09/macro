//! Pages read, built and run by the document programs service, so a published
//! page builds and runs exactly as a stored HTML document does, under the
//! same limits.

use agent_code_mode::domain::{
    DocumentBuild, DocumentProgramError, DocumentPrograms, RunProgram, UserExecutionReceipt,
};
use entity_access::domain::models::{EntityAccessReceipt, ViewAccessLevel};
use macro_user_id::user_id::MacroUserIdStr;
use tokio_util::sync::CancellationToken;

use crate::domain::ports::Pages;

impl Pages for DocumentPrograms {
    async fn source(
        &self,
        document: &EntityAccessReceipt<ViewAccessLevel>,
    ) -> Result<String, DocumentProgramError> {
        DocumentPrograms::source(self, document).await
    }

    async fn build(&self, html: &str) -> Result<DocumentBuild, DocumentProgramError> {
        self.build_source(html).await
    }

    async fn run(
        &self,
        user: &MacroUserIdStr<'static>,
        html: &str,
        request: RunProgram,
        cancel: CancellationToken,
    ) -> Result<UserExecutionReceipt, DocumentProgramError> {
        self.run_source(user, html, request, cancel).await
    }
}
