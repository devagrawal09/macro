//! What a shared routine asks of the person who takes it: the parts of a
//! snapshot that change what they fill in, connect or rely on. Wording,
//! schedules and code can change under them; these can't.

use std::collections::BTreeSet;

use routine_program::{Knob, RoutineProgram, Step};

use super::{RoutineSnapshot, TemplateVariable};
use crate::domain::event_trigger::{ActionTrigger, RoutineTrigger};

#[cfg(test)]
mod test;

/// A routine's interface: two snapshots with the same one fill in, connect
/// and run alike for whoever took the older.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoutineInterface {
    /// Each variable's key and kind.
    pub variables: BTreeSet<(String, &'static str)>,
    /// The apps it requires, by slug.
    pub apps: BTreeSet<String>,
    /// The kinds of trigger that start it.
    pub triggers: BTreeSet<&'static str>,
    /// Each code step's knobs: the step, the knob's name, its kind and the
    /// type the code reads it as.
    pub knobs: BTreeSet<(String, String, &'static str, String)>,
}

impl RoutineSnapshot {
    /// What taking this routine asks of a person.
    pub fn interface(&self) -> RoutineInterface {
        let variables = self
            .variables
            .iter()
            .map(|variable| (variable.key().to_owned(), variable_kind(variable)))
            .collect();

        // A task that doesn't read can't be saved, so it never reaches here;
        // read as empty, it changes the interface of anything it is compared to.
        let program = RoutineProgram::from_task(&self.task).ok();
        let apps = program
            .iter()
            .flat_map(|program| &program.mcp_servers)
            .map(|app| app.app_slug.clone())
            .collect();
        let knobs = program
            .iter()
            .flat_map(|program| program.steps.iter().enumerate())
            .filter_map(|(index, step)| match step {
                Step::Code(code) => Some((index, code)),
                Step::Agent(_) => None,
            })
            .flat_map(|(index, code)| {
                let step = code
                    .id
                    .as_ref()
                    .map_or_else(|| index.to_string(), ToString::to_string);
                code.knobs.iter().map(move |knob| {
                    (
                        step.clone(),
                        knob.name().to_string(),
                        knob_kind(knob),
                        knob.ts_type(),
                    )
                })
            })
            .collect();

        RoutineInterface {
            variables,
            apps,
            triggers: trigger_kinds(&self.trigger),
            knobs,
        }
    }
}

fn knob_kind(knob: &Knob) -> &'static str {
    match knob {
        Knob::Text { .. } => "text",
        Knob::Number { .. } => "number",
        Knob::Boolean { .. } => "boolean",
        Knob::Choice { .. } => "choice",
        Knob::Question { .. } => "question",
    }
}

fn variable_kind(variable: &TemplateVariable) -> &'static str {
    match variable {
        TemplateVariable::User { .. } => "user",
        TemplateVariable::Item { .. } => "item",
        TemplateVariable::TimeZone { .. } => "time_zone",
    }
}

fn trigger_kinds(trigger: &ActionTrigger) -> BTreeSet<&'static str> {
    match trigger {
        ActionTrigger::Cron { .. } => BTreeSet::from(["cron"]),
        ActionTrigger::Events { .. } => BTreeSet::from(["events"]),
        ActionTrigger::Once { .. } => BTreeSet::from(["once"]),
        ActionTrigger::Multiple { triggers } => triggers
            .as_slice()
            .iter()
            .map(|item| match item {
                RoutineTrigger::Cron { .. } => "cron",
                RoutineTrigger::Events { .. } => "events",
                RoutineTrigger::Once { .. } => "once",
            })
            .collect(),
    }
}
