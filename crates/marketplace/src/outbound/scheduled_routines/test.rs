//! The adapter over the real scheduler service and Postgres: what an install
//! records must match what the update path reads back.

use std::sync::Arc;

use bots::outbound::pg_bots_repo::PgBotsRepo;
use entity_access::domain::models::{EntityAccessReceipt, EntityType, OwnerAccessLevel};
use entity_access::domain::service::EntityAccessServiceImpl;
use entity_access::outbound::PgAccessRepository;
use entity_registry::OwnerGrantPolicy;
use entity_registry_db_utils::OwnedEntityRegistrar;
use macro_db_migrator::MACRO_DB_MIGRATIONS;
use macro_user_id::user_id::MacroUserIdStr;
use scheduled_action::domain::code_upgrade::NoCodeUpgrades;
use scheduled_action::domain::models::{InProgressExecution, ScheduledAction};
use scheduled_action::domain::ports::{
    CodeOutcome, CodeRun, CodeUnavailable, RoutineCode, ScheduledActionExecutor,
    ScheduledActionService, StepProblems,
};
use scheduled_action::domain::service::ScheduledActionServiceImpl;
use scheduled_action::domain::templates::fill::FilledRoutine;
use scheduled_action::outbound::pg_scheduled_action_repo::PgScheduledActionRepo;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{ScheduledRoutines, task_hash};
use crate::domain::models::RoutineUpdate;
use crate::domain::ports::Routines;

const BEN: &str = "macro|ben@example.com";

fn ben() -> MacroUserIdStr<'static> {
    MacroUserIdStr::parse_from_str(BEN).expect("valid user id")
}

#[test]
fn a_task_hashes_alike_however_its_keys_are_stored_and_whatever_sdk_it_was_checked_on() {
    let saved = json!({
        "steps": [{ "type": "agent", "instructions": "Summarize" }],
        "sdk": "0.2.2+18cc1c19c35c",
    });
    let read_back = json!({
        "sdk": "0.2.3+aa00bb11cc22",
        "steps": [{ "instructions": "Summarize", "type": "agent" }],
    });
    let edited = json!({
        "steps": [{ "type": "agent", "instructions": "Summarize briefly" }],
    });

    assert_eq!(task_hash(&saved), task_hash(&read_back));
    assert_ne!(task_hash(&saved), task_hash(&edited));
}

/// Every code step checks, against a fixed SDK, as on a host that runs code.
struct Checker;

impl RoutineCode for Checker {
    fn sdk(&self) -> Option<routine_program::SdkFingerprint> {
        Some("0.3.0+test".to_owned().try_into().expect("a fingerprint"))
    }

    async fn check_program(
        &self,
        _: &routine_program::RoutineProgram,
    ) -> Result<Vec<StepProblems>, CodeUnavailable> {
        Ok(Vec::new())
    }

    async fn owner_time_zone(&self, _: &MacroUserIdStr<'static>) -> Option<String> {
        None
    }

    async fn run(&self, _: &MacroUserIdStr<'static>, _: CodeRun<'_>) -> CodeOutcome {
        unreachable!("saving never runs code")
    }

    async fn reference(
        &self,
        _: &MacroUserIdStr<'static>,
        _: &str,
    ) -> Result<String, CodeUnavailable> {
        unreachable!("saving reads no reference")
    }

    fn declarations(&self, _: &[String]) -> String {
        String::new()
    }

    async fn rewrite(
        &self,
        _: &MacroUserIdStr<'static>,
        _: &routine_program::CodeStep,
        _: &str,
        _: &[String],
    ) -> Result<String, String> {
        unreachable!("saving never rewrites code")
    }
}

struct NeverExecutor;

impl ScheduledActionExecutor for NeverExecutor {
    async fn execute_action(&self, _: ScheduledAction) -> anyhow::Result<InProgressExecution> {
        unreachable!("these tests never run a routine")
    }
}

async fn insert_user(pool: &PgPool, id: &str) {
    let macro_user_id = uuid::Uuid::now_v7();
    sqlx::query!(
        r#"INSERT INTO macro_user (id, username, email, stripe_customer_id) VALUES ($1, $2, $2, $2)"#,
        macro_user_id,
        id,
    )
    .execute(pool)
    .await
    .expect("macro_user should insert");
    sqlx::query!(
        r#"INSERT INTO "User" (id, email, macro_user_id) VALUES ($1, $1, $2)"#,
        id,
        macro_user_id,
    )
    .execute(pool)
    .await
    .expect("user should insert");
}

/// The scheduler's service over Postgres, as the harness builds it, with
/// the marketplace's adapter over it.
async fn routines(
    pool: &PgPool,
) -> (
    Arc<impl ScheduledActionService>,
    ScheduledRoutines<
        impl ScheduledActionService,
        impl entity_access::domain::ports::EntityAccessService,
    >,
) {
    insert_user(pool, BEN).await;
    let repo = Arc::new(PgScheduledActionRepo::new(
        pool.clone(),
        OwnedEntityRegistrar::new(OwnerGrantPolicy::new(PgBotsRepo::new(pool.clone()))),
    ));
    let access = Arc::new(EntityAccessServiceImpl::new(PgAccessRepository::new(
        pool.clone(),
    )));
    let (dispatch, mut dispatched) = tokio::sync::mpsc::channel(32);
    tokio::spawn(async move { while dispatched.recv().await.is_some() {} });
    let actions = Arc::new(
        ScheduledActionServiceImpl::new(
            repo,
            Arc::new(NeverExecutor),
            dispatch,
            Arc::clone(&access),
        )
        .with_code(Arc::new(Checker), Arc::new(NoCodeUpgrades)),
    );
    let adapter = ScheduledRoutines::new(Arc::clone(&actions), access);
    (actions, adapter)
}

/// A routine filled in for Ben: code steps with knobs and a sentence, an Ask
/// AI step with a model and mentions, on a schedule.
fn digest() -> FilledRoutine {
    let mention = |id: &str, name: &str| {
        format!(
            r#"<m-document-mention>{{"documentId":"{id}","documentName":"{name}","blockName":"md"}}</m-document-mention>"#
        )
    };
    let instructions = format!(
        r#"Tell <m-user-mention>{{"userId":"{BEN}","email":"ben@example.com"}}</m-user-mention> what changed in {} and {}"#,
        mention("board", "Board"),
        mention("notes", "Notes"),
    );

    FilledRoutine {
        name: "Weekly digest".to_owned(),
        description: "Sums up the week".to_owned(),
        trigger: serde_json::from_value(json!({
            "type": "cron",
            "schedule": "0 0 9 * * MON",
            "timezone": "America/New_York",
        }))
        .expect("a trigger"),
        task: json!({
            "steps": [
                {
                    "type": "code",
                    "title": "Read the week's tasks",
                    "summary": "Lists tasks changed in the last days.",
                    "source": "export async function main({ knobs }) { return [knobs.days, knobs.ratio, knobs.doneOnly, knobs.priority, knobs.urgent]; }",
                    "knobs": [
                        { "type": "number", "name": "days", "label": "Days back", "value": 7.0, "min": 1.0, "max": 30.0 },
                        { "type": "number", "name": "ratio", "label": "Share", "value": 0.25 },
                        { "type": "boolean", "name": "doneOnly", "label": "Only done", "value": false },
                        { "type": "choice", "name": "priority", "label": "Priority", "value": "high", "options": ["low", "high"] },
                        { "type": "question", "name": "urgent", "label": "it's urgent", "value": "Is this urgent?" },
                    ],
                    "sentence": "Read {days} days of tasks when {?urgent}",
                },
                {
                    "type": "code",
                    "title": "Label them",
                    "source": "export async function main({ knobs, steps }) { return [knobs.label, steps]; }",
                    "knobs": [{ "type": "text", "name": "label", "label": "Label", "value": "weekly ✓ \"quoted\"" }],
                },
                { "type": "agent", "model": "claude-sonnet-5-5", "instructions": instructions },
            ],
        }),
    }
}

fn owner(id: uuid::Uuid) -> EntityAccessReceipt<OwnerAccessLevel> {
    EntityAccessReceipt::dangerously_assert_authenticated_user(
        ben(),
        &id.to_string(),
        EntityType::ScheduledAction,
    )
}

async fn stored_task(actions: &impl ScheduledActionService, id: uuid::Uuid) -> Value {
    actions
        .get_action(&owner(id))
        .await
        .expect("the routine reads back")
        .task
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_installed_routine_hashes_alike_when_read_back_and_after_a_no_op_update(pool: PgPool) {
    let (actions, adapter) = routines(&pool).await;
    let routine = digest();

    let copy = adapter
        .create(&ben(), &routine, false)
        .await
        .expect("the routine is created");
    let read_back = stored_task(actions.as_ref(), copy.routine_id).await;
    // The update path reads it back and compares, as a minor version does.
    let unchanged = adapter
        .update(&ben(), copy.routine_id, &routine, Some(&copy.task_hash))
        .await
        .expect("the routine updates");
    let after_update = stored_task(actions.as_ref(), copy.routine_id).await;
    let RoutineUpdate::Updated(updated_hash) = unchanged else {
        panic!("expected the unedited routine to update, got {unchanged:?}");
    };
    let again = adapter
        .update(&ben(), copy.routine_id, &routine, Some(&updated_hash))
        .await
        .expect("the routine updates again");

    // Saving stamped the SDK and named the steps: what was sent differs from
    // what is stored, so the install hashes what the service returned.
    assert_ne!(task_hash(&routine.task), copy.task_hash);
    assert!(read_back.get("sdk").is_some());
    assert_eq!(read_back["steps"][0]["id"], json!("read_the_week_s_tasks"));
    assert_eq!(task_hash(&read_back), copy.task_hash);
    assert_eq!(updated_hash, copy.task_hash);
    assert_eq!(task_hash(&after_update), copy.task_hash);
    assert!(matches!(again, RoutineUpdate::Updated(_)));
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn an_edited_routine_reads_as_edited(pool: PgPool) {
    let (_, adapter) = routines(&pool).await;
    let copy = adapter
        .create(&ben(), &digest(), false)
        .await
        .expect("the routine is created");
    let mut edited = digest();
    edited.task["steps"][2]["instructions"] = json!("My own words");
    adapter
        .update(&ben(), copy.routine_id, &edited, None)
        .await
        .expect("the person edits it");

    let update = adapter
        .update(&ben(), copy.routine_id, &digest(), Some(&copy.task_hash))
        .await
        .expect("the update reads it");

    assert_eq!(update, RoutineUpdate::Edited);
}

#[sqlx::test(migrator = "MACRO_DB_MIGRATIONS")]
async fn a_deleted_routine_reads_as_gone(pool: PgPool) {
    let (actions, adapter) = routines(&pool).await;
    let copy = adapter
        .create(&ben(), &digest(), false)
        .await
        .expect("the routine is created");
    actions
        .delete_action(owner(copy.routine_id))
        .await
        .expect("the person deletes it");

    let update = adapter
        .update(&ben(), copy.routine_id, &digest(), Some(&copy.task_hash))
        .await
        .expect("a missing routine is not an error");
    let never_existed = adapter
        .update(
            &ben(),
            uuid::Uuid::now_v7(),
            &digest(),
            Some(&copy.task_hash),
        )
        .await
        .expect("a missing routine is not an error");

    assert_eq!(update, RoutineUpdate::Gone);
    assert_eq!(never_existed, RoutineUpdate::Gone);
}
