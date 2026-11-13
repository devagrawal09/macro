//! The [`Routines`] port over the scheduler's service: a published routine
//! is read, and an install's copy created and updated, exactly as the
//! person would through the routines API, under its checks.

#[cfg(test)]
mod test;

use std::sync::Arc;

use agent_code_mode::domain::SourceHash;
use entity_access::domain::models::{
    AccessError, EditAccessLevel, EntityAccessReceipt, EntityType, OwnerAccessLevel,
};
use entity_access::domain::ports::EntityAccessService;
use macro_user_id::user_id::MacroUserIdStr;
use model_owner::CreationPrincipal;
use scheduled_action::domain::code_check::CodeCheckError;
use scheduled_action::domain::models::{
    ActionConfiguration, ActionConfigurationUpdate, ActionKind, ActionPolicyError,
    OwnerNotUserError, RunVisibility,
};
use scheduled_action::domain::ports::ScheduledActionService;
use scheduled_action::domain::target_validation::TargetValidationError;
use scheduled_action::domain::templates::{
    RoutineSnapshot, TemplateInputError, TemplateVariable, fill::FilledRoutine,
};
use serde_json::Value;
use uuid::Uuid;

use crate::domain::models::{
    MarketplaceError, RoutineCopy, RoutineUpdate, TaskHash, canonical_json,
};
use crate::domain::ports::Routines;

/// Routines saved through the scheduler's service, authorized through
/// entity access as the person whose routine it is.
pub struct ScheduledRoutines<S, A> {
    actions: Arc<S>,
    access: Arc<A>,
}

impl<S, A> ScheduledRoutines<S, A> {
    /// Bind the scheduler's service and entity access.
    pub fn new(actions: Arc<S>, access: Arc<A>) -> Self {
        Self { actions, access }
    }
}

/// The scheduler's service failed.
#[derive(Debug, thiserror::Error)]
#[error("routine service failed: {0:#}")]
struct RoutineServiceError(anyhow::Error);

/// A scheduler error as the marketplace answers it: a missing routine, a
/// routine its checks refuse, or an internal failure.
fn routine_error(error: anyhow::Error) -> MarketplaceError {
    if matches!(
        error.downcast_ref::<ActionPolicyError>(),
        Some(ActionPolicyError::NotFound)
    ) {
        return MarketplaceError::NotFound;
    }

    let rejected = error.downcast_ref::<ActionPolicyError>().is_some()
        || error.downcast_ref::<TargetValidationError>().is_some()
        || error.downcast_ref::<TemplateInputError>().is_some()
        || error.downcast_ref::<OwnerNotUserError>().is_some()
        || matches!(
            error.downcast_ref::<CodeCheckError>(),
            Some(CodeCheckError::DoesNotCheck { .. })
        );
    if rejected {
        return MarketplaceError::RoutineRejected(error.to_string());
    }

    MarketplaceError::Internal(rootcause::Report::new(RoutineServiceError(error)).into_dynamic())
}

/// The hash of a routine's task, which changes only when someone changes
/// the task. The SDK version stamped on a code check is left out: the
/// scheduler stamps it again by itself.
pub fn task_hash(task: &Value) -> TaskHash {
    let mut task = task.clone();
    if let Value::Object(fields) = &mut task {
        fields.remove("sdk");
    }

    TaskHash(SourceHash::of(&canonical_json(&task)).as_ref().to_owned())
}

impl<S, A> Routines for ScheduledRoutines<S, A>
where
    S: ScheduledActionService,
    A: EntityAccessService,
{
    #[tracing::instrument(err, skip_all)]
    async fn snapshot(
        &self,
        routine: &EntityAccessReceipt<OwnerAccessLevel>,
        description: &str,
        variables: Vec<TemplateVariable>,
    ) -> Result<RoutineSnapshot, MarketplaceError> {
        let action = self
            .actions
            .get_action(routine)
            .await
            .map_err(routine_error)?;
        if action.owner_user().is_err() {
            return Err(MarketplaceError::Invalid(
                "only a routine a person owns can be published".to_owned(),
            ));
        }

        RoutineSnapshot::of(&action, description, variables)
            .map_err(|error| MarketplaceError::Invalid(error.to_string()))
    }

    #[tracing::instrument(err, skip_all)]
    async fn create(
        &self,
        user: &MacroUserIdStr<'static>,
        routine: &FilledRoutine,
        enabled: bool,
    ) -> Result<RoutineCopy, MarketplaceError> {
        let created = self
            .actions
            .create_action(
                &CreationPrincipal::User(user.clone()),
                ActionConfiguration {
                    name: routine.name.clone(),
                    trigger: routine.trigger.clone(),
                    kind: ActionKind::Agent,
                    task: routine.task.clone(),
                    enabled,
                    run_visibility: RunVisibility::default(),
                    description: routine.description.clone(),
                },
            )
            .await
            .map_err(routine_error)?;
        let routine_id = created
            .id
            .ok_or_else(|| routine_error(anyhow::anyhow!("a created routine has no id")))?;

        Ok(RoutineCopy {
            routine_id,
            task_hash: task_hash(&created.task),
        })
    }

    #[tracing::instrument(err, skip_all, fields(%routine_id))]
    async fn update(
        &self,
        user: &MacroUserIdStr<'static>,
        routine_id: Uuid,
        routine: &FilledRoutine,
        unedited: Option<&TaskHash>,
    ) -> Result<RoutineUpdate, MarketplaceError> {
        let receipt = match self
            .access
            .generate_entity_access_receipt::<EditAccessLevel>(
                user,
                None,
                &routine_id.to_string(),
                EntityType::ScheduledAction,
            )
            .await
        {
            Ok(receipt) => receipt,
            Err(
                AccessError::NotFound(_)
                | AccessError::Unauthorized
                | AccessError::UnauthorizedWithMessage(_),
            ) => return Ok(RoutineUpdate::Gone),
            Err(error) => return Err(routine_error(anyhow::anyhow!("{error:?}"))),
        };

        let current = match self.actions.get_action(&receipt).await {
            Ok(current) => current,
            Err(error) => {
                return match routine_error(error) {
                    MarketplaceError::NotFound => Ok(RoutineUpdate::Gone),
                    other => Err(other),
                };
            }
        };
        if unedited.is_some_and(|expected| task_hash(&current.task) != *expected) {
            return Ok(RoutineUpdate::Edited);
        }

        // An update stores a new configuration revision, so a run already
        // started keeps the configuration it started with.
        let updated = self
            .actions
            .update_action(
                receipt,
                ActionConfigurationUpdate {
                    name: routine.name.clone(),
                    trigger: routine.trigger.clone(),
                    kind: ActionKind::Agent,
                    task: routine.task.clone(),
                    enabled: None,
                    run_visibility: None,
                    description: Some(routine.description.clone()),
                },
            )
            .await
            .map_err(routine_error)?;

        Ok(RoutineUpdate::Updated(task_hash(&updated.task)))
    }
}
