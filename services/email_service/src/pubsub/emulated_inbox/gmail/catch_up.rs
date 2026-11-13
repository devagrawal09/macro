//! Catching up after Gmail forgot the history Macro's sync cursor points at.
//!
//! Inbox sync answers Gmail's 404 for an expired cursor by starting a
//! recovery backfill, so the harness also runs the backfill worker and
//! publishes the backfill outbox, as `pubsub_workers` does.

use super::*;
use crate::pubsub::backfill::worker::BackfillContext;
use email::domain::backfill_jobs::outbox::{drain_crm, drain_email_completion, drain_email_init};
use std::sync::Arc;

#[cfg(test)]
mod specs;

/// The production backfill worker context for the queue at `queue`.
pub(super) fn backfill_context(wiring: &InboxSyncWiring<'_>, queue: &str) -> BackfillContext {
    let db = wiring.db.clone();
    let services_url = wiring.services_url;
    crate::pubsub::backfill::worker::context(
        db.clone(),
        &sqs_worker::SQSWorker::new(wiring.sqs_sdk.clone(), queue.to_string(), 1, 1),
        wiring.sqs_client.clone(),
        Arc::new(contacts::domain::service::SqsContactsIngress {
            queue: contacts::outbound::ingress::SqsContactsQueue::new(
                wiring.sqs_sdk.clone(),
                format!("{services_url}/queue/contacts"),
            ),
        }),
        wiring.email_api.clone(),
        test_support::redis_client(wiring.redis.clone()),
        Arc::new(notification::domain::service::SqsNotificationIngress {
            queue: notification::outbound::queue::SqsQueue::new(
                wiring.sqs_sdk.clone(),
                format!("{services_url}/queue/notifications"),
            ),
        }),
        static_file_service_client::StaticFileServiceClient::new(
            "key".to_string(),
            services_url.to_string(),
        ),
        connection_gateway_client::client::ConnectionGatewayClient::new(
            "key".to_string(),
            services_url.to_string(),
        ),
        document_storage_service_client::DocumentStorageServiceClient::new(
            "key".to_string(),
            services_url.to_string(),
        ),
        Arc::new(system_properties::SystemPropertiesServiceImpl::new(
            system_properties::PgSystemPropertiesRepository::new(db.clone()),
        )),
        crm::domain::service::CrmServiceImpl::new(
            crm::outbound::companies_repo::CompaniesRepositoryImpl::new(db),
            crate::composition::workers::CrmMetadataResolver::new(Arc::new(
                unfurl::domain::service::UnfurlServiceImpl::new(
                    unfurl::outbound::ReqwestUnfurlFetcher::new().expect("unfurl fetcher"),
                ),
            )),
        ),
        wiring.broker.clone(),
        false,
    )
}

impl GmailInbox {
    /// Macro hears nothing from Gmail for longer than Gmail keeps a
    /// mailbox's history: the history its sync cursor points at is gone.
    pub fn is_away_longer_than_gmail_keeps_history(&self) {
        self.gmail.expire_history(OWNER).expect("expire history");
    }

    /// Mail arrives while Macro is away: Gmail's push for it never
    /// reaches Macro.
    pub fn receives_while_away(&self, mail: Mail) -> Received {
        self.receives(mail)
    }

    /// Publishes what the backfill outbox holds, as `backfill_outbox::run`
    /// does every few seconds.
    pub(super) async fn publish_backfill_outbox(&self) {
        let db = crate::outbound::backfill_jobs::outbox::PgBackfillOutbox(self.db.clone());
        let sqs = crate::outbound::backfill_jobs::worker::BackfillSqs(
            sqs_client::SQS::new(fake_sqs::client(&self.queues.sqs_url(), false))
                .email_backfill_queue(&self.backfill_queue),
        );
        drain_crm(&db, &sqs).await.expect("publish CRM backfill");
        drain_email_init(&db, &sqs)
            .await
            .expect("publish backfill init");
        drain_email_completion(&db, &sqs)
            .await
            .expect("publish backfill completion");
    }
}
