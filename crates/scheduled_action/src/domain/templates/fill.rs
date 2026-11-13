//! Filling in a shared routine for the person who takes it: the server's
//! form of the web app's template activation (`core/template.ts`). Mentions
//! of the author become the person, references they leave out become plain
//! text, and schedules move to their time zone.

use std::collections::{BTreeMap, HashMap};

use chrono_tz::Tz;
use lazy_regex::{Lazy, Regex, lazy_regex};
use macro_user_id::user_id::MacroUserIdStr;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use utoipa::ToSchema;

use super::{RoutineSnapshot, TemplateVariable, VariableFill};
use crate::domain::event_trigger::{ActionTrigger, RoutineTrigger, RoutineTriggers};

#[cfg(test)]
mod test;

/// A `<m-user-mention>` or `<m-document-mention>` tag and its JSON.
static MENTION: Lazy<Regex> = lazy_regex!(
    r"(?s)<m-user-mention>(.*?)</m-user-mention>|<m-document-mention>(.*?)</m-document-mention>"
);

/// What a person chose when taking a shared routine.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RoutineFills {
    /// Their time zone, which schedules filled in as them run in.
    #[schema(value_type = String)]
    pub timezone: Tz,
    /// How each variable is filled, by its key, in place of how its author
    /// marked it: `shared` keeps a reference, `choice` leaves it out and
    /// `activator` fills it in as them. A `choice` variable needs one.
    #[serde(default)]
    pub variables: BTreeMap<String, VariableFill>,
}

impl RoutineFills {
    /// These fills with `newer`'s time zone and variables taking precedence.
    pub fn merged(mut self, newer: RoutineFills) -> Self {
        self.timezone = newer.timezone;
        self.variables.extend(newer.variables);
        self
    }
}

/// The routine filled in for one person: what their own routine is saved as.
#[derive(Debug, Clone, PartialEq)]
pub struct FilledRoutine {
    pub name: String,
    pub description: String,
    pub trigger: ActionTrigger,
    pub task: Value,
}

/// Variables the author left for each person to choose that have no fill.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("choose what to do with {} of this routine's references", .0.len())]
pub struct MissingFills(pub Vec<TemplateVariable>);

impl RoutineSnapshot {
    /// The routine as `person`'s own, filled in with `fills`, or the
    /// variables still needing one.
    pub fn fill(
        &self,
        person: &MacroUserIdStr<'_>,
        fills: &RoutineFills,
    ) -> Result<FilledRoutine, MissingFills> {
        let missing: Vec<TemplateVariable> = self
            .variables
            .iter()
            .filter(|variable| {
                variable.fill() == VariableFill::Choice
                    && !fills.variables.contains_key(variable.key())
            })
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(MissingFills(missing));
        }

        // The author's marks, each replaced by the person's own fill.
        let effective: HashMap<&str, VariableFill> = self
            .variables
            .iter()
            .map(|variable| {
                let fill = fills
                    .variables
                    .get(variable.key())
                    .copied()
                    .unwrap_or(variable.fill());
                (variable.key(), fill)
            })
            .collect();
        let text = |text: &str| fill_text(text, &effective, person);

        Ok(FilledRoutine {
            name: self.name.clone(),
            description: self.description.clone(),
            trigger: fill_trigger(&self.trigger, &self.variables, &effective, fills.timezone),
            task: fill_task(&self.task, text),
        })
    }
}

/// The task with every text that can mention people or items passed through
/// `fill`: each Ask AI step's instructions, or an agent task's two prompts.
/// Code reads its inputs from the trigger, so it is never filled in.
fn fill_task(task: &Value, fill: impl Fn(&str) -> String) -> Value {
    let mut task = task.clone();
    let fill_field = |fields: &mut serde_json::Map<String, Value>, key: &str| {
        if let Some(Value::String(text)) = fields.get_mut(key) {
            *text = fill(text);
        }
    };

    if let Some(Value::Array(steps)) = task.get_mut("steps") {
        for step in steps {
            let Value::Object(fields) = step else {
                continue;
            };
            if fields.get("type").and_then(Value::as_str) == Some("agent") {
                fill_field(fields, "instructions");
            }
        }
    } else if let Value::Object(fields) = &mut task {
        fill_field(fields, "prompt");
        fill_field(fields, "user_prompt");
    }

    task
}

/// A mention's id and the words it reads as.
struct Mention {
    id: String,
    label: String,
    user: bool,
}

fn parse_mention(json: &str, user: bool) -> Option<Mention> {
    let data: Value = serde_json::from_str(json).ok()?;
    let field = |name: &str| {
        data.get(name)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };

    if user {
        let id = field("userId")?;
        let email = field("email");
        let email_name = email
            .as_deref()
            .and_then(|email| email.split('@').next())
            .map(str::to_owned);
        let label = field("displayName")
            .or(email_name)
            .unwrap_or_else(|| id.clone());
        return Some(Mention { id, label, user });
    }

    let id = field("documentId")?;
    let label = field("documentName").unwrap_or_else(|| "Untitled".to_owned());
    Some(Mention { id, label, user })
}

/// `text` as the person's routine reads it: their author's mentions name
/// them, and references they leave out become plain text.
fn fill_text(
    text: &str,
    fills: &HashMap<&str, VariableFill>,
    person: &MacroUserIdStr<'_>,
) -> String {
    MENTION
        .replace_all(text, |captures: &lazy_regex::Captures| {
            let tag = captures[0].to_owned();
            let (json, user) = match (captures.get(1), captures.get(2)) {
                (Some(json), _) => (json.as_str(), true),
                (_, Some(json)) => (json.as_str(), false),
                _ => return tag,
            };
            let Some(mention) = parse_mention(json, user) else {
                return tag;
            };

            let fill = fills
                .get(mention.id.as_str())
                .copied()
                .unwrap_or(VariableFill::Shared);
            match fill {
                VariableFill::Activator if mention.user => {
                    let data = json!({
                        "userId": person.as_ref(),
                        "email": person.email_str(),
                    });
                    format!("<m-user-mention>{data}</m-user-mention>")
                }
                VariableFill::Choice => mention.label,
                VariableFill::Activator | VariableFill::Shared => tag,
            }
        })
        .into_owned()
}

/// The trigger with its schedules in `timezone` when the time zone fills in
/// as the person.
fn fill_trigger(
    trigger: &ActionTrigger,
    variables: &[TemplateVariable],
    fills: &HashMap<&str, VariableFill>,
    timezone: Tz,
) -> ActionTrigger {
    let zone_fills_in = variables.iter().any(|variable| {
        matches!(variable, TemplateVariable::TimeZone { .. })
            && fills.get(variable.key()) == Some(&VariableFill::Activator)
    });
    if !zone_fills_in {
        return trigger.clone();
    }

    match trigger {
        ActionTrigger::Cron { schedule, .. } => ActionTrigger::Cron {
            schedule: schedule.clone(),
            timezone,
        },
        ActionTrigger::Multiple { triggers } => {
            let moved: Vec<RoutineTrigger> = triggers
                .as_slice()
                .iter()
                .map(|item| match item {
                    RoutineTrigger::Cron { schedule, .. } => RoutineTrigger::Cron {
                        schedule: schedule.clone(),
                        timezone,
                    },
                    other => other.clone(),
                })
                .collect();
            // Same count and kinds as triggers already accepted.
            RoutineTriggers::try_from(moved)
                .map(|triggers| ActionTrigger::Multiple { triggers })
                .unwrap_or_else(|_| trigger.clone())
        }
        ActionTrigger::Events { .. } | ActionTrigger::Once { .. } => trigger.clone(),
    }
}
