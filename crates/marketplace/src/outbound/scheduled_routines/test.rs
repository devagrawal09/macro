use serde_json::json;

use super::task_hash;

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
