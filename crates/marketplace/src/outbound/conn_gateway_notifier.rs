//! The [`VersionNotifier`] port over the connection gateway: each installer's
//! open clients receive a `marketplace_listing_version` message.

use std::sync::Arc;

use connection_gateway_client::client::ConnectionGatewayClient;
use macro_user_id::user_id::MacroUserIdStr;
use model_entity::EntityType;

use crate::domain::models::{LISTING_VERSION_MESSAGE_TYPE, VersionNotice};
use crate::domain::ports::VersionNotifier;

/// Most users one gateway request names. The gateway looks every one up at
/// once, so a large notification goes out as several requests in turn.
const USERS_PER_REQUEST: usize = 100;

/// Sends version notifications through the connection gateway.
pub struct ConnGatewayVersionNotifier {
    client: Arc<ConnectionGatewayClient>,
}

impl ConnGatewayVersionNotifier {
    /// Send through `client`.
    pub fn new(client: Arc<ConnectionGatewayClient>) -> Self {
        Self { client }
    }
}

/// Why a version notification wasn't fully sent.
#[derive(Debug, thiserror::Error)]
pub enum VersionNotifierErr {
    /// The message didn't serialize.
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    /// The gateway refused or didn't answer at least one request; the others
    /// were still sent.
    #[error("{failed} of {requests} connection gateway requests failed: {first}")]
    Gateway {
        /// Requests that failed.
        failed: usize,
        /// Requests sent.
        requests: usize,
        /// The first failure.
        first: String,
    },
}

impl VersionNotifier for ConnGatewayVersionNotifier {
    type Err = VersionNotifierErr;

    #[tracing::instrument(err, skip_all)]
    async fn notify(
        &self,
        version: VersionNotice,
        installers: &[MacroUserIdStr<'static>],
    ) -> Result<(), Self::Err> {
        let message = serde_json::to_value(version)?;

        // A failed request doesn't stop the next: those users still hear.
        let mut requests = 0;
        let mut failures = Vec::new();
        for users in installers.chunks(USERS_PER_REQUEST) {
            requests += 1;
            let entities = users
                .iter()
                .map(|user| EntityType::User.with_entity_str(user.as_ref()))
                .collect();
            let sent = self
                .client
                .batch_send_message(
                    LISTING_VERSION_MESSAGE_TYPE.to_owned(),
                    message.clone(),
                    entities,
                )
                .await;
            if let Err(error) = sent {
                failures.push(error);
            }
        }

        match failures.first() {
            None => Ok(()),
            Some(first) => Err(VersionNotifierErr::Gateway {
                failed: failures.len(),
                requests,
                first: format!("{first:#}"),
            }),
        }
    }
}
