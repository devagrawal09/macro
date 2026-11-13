use std::collections::BTreeMap;

use chrono_tz::Tz;
use macro_user_id::user_id::MacroUserIdStr;
use serde_json::json;

use super::*;

fn person() -> MacroUserIdStr<'static> {
    MacroUserIdStr::try_from("macro|ben@example.com".to_owned()).unwrap()
}

const ADA_MENTION: &str = r#"<m-user-mention>{"userId":"macro|ada@example.com","email":"ada@example.com","displayName":"Ada"}</m-user-mention>"#;
const BOARD_MENTION: &str = r#"<m-document-mention>{"documentId":"board","documentName":"Board","blockName":"md"}</m-document-mention>"#;
const PRIVATE_MENTION: &str = r#"<m-document-mention>{"documentId":"private","documentName":"Private notes","blockName":"md"}</m-document-mention>"#;

fn snapshot(task: Value, trigger: ActionTrigger) -> RoutineSnapshot {
    RoutineSnapshot {
        name: "Weekly digest".to_owned(),
        description: "Sums up the week".to_owned(),
        trigger,
        task,
        variables: vec![
            TemplateVariable::User {
                id: "macro|ada@example.com".to_owned(),
                label: "Ada".to_owned(),
                fill: VariableFill::Activator,
            },
            TemplateVariable::Item {
                id: "board".to_owned(),
                block_name: "md".to_owned(),
                label: "Board".to_owned(),
                fill: VariableFill::Shared,
            },
            TemplateVariable::Item {
                id: "private".to_owned(),
                block_name: "md".to_owned(),
                label: "Private notes".to_owned(),
                fill: VariableFill::Choice,
            },
            TemplateVariable::TimeZone {
                timezone: Tz::Europe__London,
                fill: VariableFill::Activator,
            },
        ],
    }
}

fn cron() -> ActionTrigger {
    serde_json::from_value(json!({
        "type": "cron",
        "schedule": "0 0 9 * * MON",
        "timezone": "Europe/London",
    }))
    .unwrap()
}

fn fills(private: Option<VariableFill>) -> RoutineFills {
    RoutineFills {
        timezone: Tz::America__New_York,
        variables: private
            .map(|fill| BTreeMap::from([("private".to_owned(), fill)]))
            .unwrap_or_default(),
    }
}

#[test]
fn mentions_fill_in_as_the_person_and_left_out_references_become_text() {
    let instructions = format!("Tell {ADA_MENTION} about {BOARD_MENTION} and {PRIVATE_MENTION}");
    let routine = snapshot(
        json!({
            "steps": [
                { "type": "code", "title": "Read", "source": format!("// {ADA_MENTION}") },
                { "type": "agent", "instructions": instructions },
            ],
        }),
        cron(),
    );

    let filled = routine
        .fill(&person(), &fills(Some(VariableFill::Choice)))
        .unwrap();

    assert_eq!(
        filled.task["steps"][1]["instructions"],
        format!(
            r#"Tell <m-user-mention>{{"userId":"macro|ben@example.com","email":"ben@example.com"}}</m-user-mention> about {BOARD_MENTION} and Private notes"#
        )
    );
    // Code reads its inputs from the trigger and is never filled in.
    assert_eq!(
        filled.task["steps"][0]["source"],
        format!("// {ADA_MENTION}")
    );
}

#[test]
fn a_kept_choice_stays_a_reference_in_an_agent_task() {
    let routine = snapshot(
        json!({ "prompt": BOARD_MENTION, "user_prompt": PRIVATE_MENTION }),
        cron(),
    );

    let filled = routine
        .fill(&person(), &fills(Some(VariableFill::Shared)))
        .unwrap();

    assert_eq!(filled.task["prompt"], BOARD_MENTION);
    assert_eq!(filled.task["user_prompt"], PRIVATE_MENTION);
}

#[test]
fn schedules_move_to_the_persons_time_zone() {
    let routine = snapshot(json!({ "prompt": "", "user_prompt": "Go" }), cron());

    let filled = routine
        .fill(&person(), &fills(Some(VariableFill::Choice)))
        .unwrap();

    assert_eq!(
        filled.trigger,
        serde_json::from_value::<ActionTrigger>(json!({
            "type": "cron",
            "schedule": "0 0 9 * * MON",
            "timezone": "America/New_York",
        }))
        .unwrap()
    );
}

#[test]
fn a_choice_without_a_fill_is_missing() {
    let routine = snapshot(json!({ "prompt": "", "user_prompt": "Go" }), cron());

    let missing = routine.fill(&person(), &fills(None)).unwrap_err();

    assert_eq!(
        missing
            .0
            .iter()
            .map(|variable| variable.key())
            .collect::<Vec<_>>(),
        vec!["private"]
    );
}

#[test]
fn newer_fills_take_precedence() {
    let stored = RoutineFills {
        timezone: Tz::Europe__London,
        variables: BTreeMap::from([
            ("board".to_owned(), VariableFill::Choice),
            ("private".to_owned(), VariableFill::Shared),
        ]),
    };

    let merged = stored.merged(fills(Some(VariableFill::Choice)));

    assert_eq!(merged.timezone, Tz::America__New_York);
    assert_eq!(
        merged.variables,
        BTreeMap::from([
            ("board".to_owned(), VariableFill::Choice),
            ("private".to_owned(), VariableFill::Choice),
        ])
    );
}
