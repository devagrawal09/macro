use chrono_tz::Tz;
use serde_json::{Value, json};

use super::*;
use crate::domain::templates::VariableFill;

fn snapshot(task: Value, trigger: Value, variables: Vec<TemplateVariable>) -> RoutineSnapshot {
    RoutineSnapshot {
        name: "Label invoices".to_owned(),
        description: String::new(),
        trigger: serde_json::from_value(trigger).unwrap(),
        task,
        variables,
    }
}

fn program(knob_type: &str, apps: Value) -> Value {
    json!({
        "steps": [
            {
                "type": "code",
                "id": "label",
                "title": "Label invoices",
                "source": "export async function main() {}",
                "knobs": [{ "type": knob_type, "name": "label", "label": "Label", "value": "money" }],
            },
            { "type": "agent", "instructions": "Summarize" },
        ],
        "mcp_servers": apps,
    })
}

fn cron() -> Value {
    json!({ "type": "cron", "schedule": "0 0 9 * * MON", "timezone": "Europe/London" })
}

fn zone() -> TemplateVariable {
    TemplateVariable::TimeZone {
        timezone: Tz::Europe__London,
        fill: VariableFill::Activator,
    }
}

#[test]
fn reads_variables_apps_triggers_and_knobs() {
    let routine = snapshot(
        program("text", json!([{ "app_slug": "linear", "server_name": "Linear" }])),
        json!({
            "type": "multiple",
            "triggers": [
                { "type": "cron", "schedule": "0 0 9 * * MON", "timezone": "Europe/London" },
                { "type": "once", "at": "2026-12-01T09:00:00Z" },
            ],
        }),
        vec![zone()],
    );

    let interface = routine.interface();

    assert_eq!(
        interface.variables,
        BTreeSet::from([("time_zone".to_owned(), "time_zone")])
    );
    assert_eq!(interface.apps, BTreeSet::from(["linear".to_owned()]));
    assert_eq!(interface.triggers, BTreeSet::from(["cron", "once"]));
    assert_eq!(
        interface.knobs,
        BTreeSet::from([(
            "label".to_owned(),
            "label".to_owned(),
            "text",
            "string".to_owned()
        )])
    );
}

#[test]
fn wording_and_values_leave_it_alone() {
    let before = snapshot(program("text", json!([])), cron(), vec![zone()]);
    let mut after = before.clone();
    after.name = "Label every invoice".to_owned();
    after.task["steps"][1]["instructions"] = json!("Summarize briefly");
    after.task["steps"][0]["knobs"][0]["value"] = json!("finance");

    assert_eq!(before.interface(), after.interface());
}

#[test]
fn a_new_app_variable_trigger_or_knob_type_changes_it() {
    let before = snapshot(program("text", json!([])), cron(), vec![zone()]);
    let with_app = snapshot(
        program("text", json!([{ "app_slug": "linear", "server_name": "Linear" }])),
        cron(),
        vec![zone()],
    );
    let with_variable = snapshot(
        program("text", json!([])),
        cron(),
        vec![
            zone(),
            TemplateVariable::Item {
                id: "board".to_owned(),
                block_name: "md".to_owned(),
                label: "Board".to_owned(),
                fill: VariableFill::Choice,
            },
        ],
    );
    let on_events = snapshot(
        program("text", json!([])),
        json!({ "type": "events", "filters": [{ "events": ["document.created"] }] }),
        vec![zone()],
    );
    let with_question = snapshot(program("question", json!([])), cron(), vec![zone()]);

    assert_ne!(before.interface(), with_app.interface());
    assert_ne!(before.interface(), with_variable.interface());
    assert_ne!(before.interface(), on_events.interface());
    assert_ne!(before.interface(), with_question.interface());
}
