//! Quest authoring rules: the Save gate and the picker's rename/duplicate/delete logic, moved out of
//! `MainWindow.xaml.cs` and `Core/QuestAction.cs`.
//!
//! `v2/SealTools.Tests/QuestConfigTests.cs` is the oracle for the reference integrity, and the
//! launcher's Quest-tab Save handler for the refusal message. Every failure mode here is SILENT on
//! screen in v2: a dangling entry makes the run skip a step it cannot plan, and a rename that leaves
//! entries behind is only noticed later by a Save that refuses.

use seal_domain::authoring::{
    PresetAction, PresetError, PresetNoun, check_saveable, dangling_sequence_entries,
    duplicate_flow, duplicate_sequence, quest_step_complaint, remove_flow, rename_flow,
    rename_sequence,
};
use seal_domain::config::{
    QuestConfig, QuestPreset, QuestSequence, QuestSequenceEntry, QuestStep, quest_action,
};

fn quest_with_flows(names: &[&str]) -> QuestConfig {
    let mut quest = QuestConfig::default();
    for name in names {
        quest
            .presets
            .insert((*name).to_owned(), QuestPreset::default());
    }
    quest.active = names
        .first()
        .map(|name| (*name).to_owned())
        .unwrap_or_default();
    quest
}

fn sequence_of(flows: &[&str]) -> QuestSequence {
    QuestSequence {
        entries: flows
            .iter()
            .map(|flow| QuestSequenceEntry {
                preset: (*flow).to_owned(),
                times: 1,
            })
            .collect(),
        ..QuestSequence::default()
    }
}

// ── reference integrity (`QuestConfig.DanglingSequenceEntries`) ───────────────────────────────────

#[test]
fn a_sequence_that_names_real_flows_is_not_dangling() {
    let mut quest = quest_with_flows(&["a", "b"]);
    quest
        .sequences
        .insert("run".to_owned(), sequence_of(&["a", "b"]));

    assert!(dangling_sequence_entries(&quest).is_empty());
    assert!(check_saveable(&quest).is_ok());
}

#[test]
fn an_entry_whose_flow_is_gone_is_reported_with_its_one_based_position() {
    let mut quest = quest_with_flows(&["a", "b"]);
    quest
        .sequences
        .insert("run".to_owned(), sequence_of(&["a", "deleted"]));

    let dangling = dangling_sequence_entries(&quest);

    assert_eq!(dangling.len(), 1);
    assert_eq!(dangling[0].sequence, "run");
    // 1-based, because this number goes straight into a message the player reads next to the rows.
    assert_eq!(dangling[0].entry, 2);
    assert_eq!(dangling[0].flow, "deleted");
}

#[test]
fn every_dangling_entry_is_reported_not_just_the_first() {
    let mut quest = quest_with_flows(&["a"]);
    quest
        .sequences
        .insert("run".to_owned(), sequence_of(&["gone1", "a", "gone2"]));

    let dangling = dangling_sequence_entries(&quest);
    let found: Vec<(usize, &str)> = dangling
        .iter()
        .map(|entry| (entry.entry, entry.flow.as_str()))
        .collect();
    assert_eq!(found, vec![(1, "gone1"), (3, "gone2")]);
}

/// A blank entry is not a flow named "": the run looks that name up and finds nothing, so it is
/// refused for the same reason a deleted one is.
#[test]
fn an_empty_flow_name_is_dangling_too() {
    let mut quest = quest_with_flows(&["a"]);
    quest.sequences.insert("run".to_owned(), sequence_of(&[""]));

    assert_eq!(dangling_sequence_entries(&quest).len(), 1);
}

/// The check agrees with the runner, which trims the name before looking it up — a padded name that
/// runs must not stop every Save on the tab.
#[test]
fn a_padded_name_the_runner_would_trim_is_not_dangling() {
    let mut quest = quest_with_flows(&["a"]);
    quest
        .sequences
        .insert("run".to_owned(), sequence_of(&[" a "]));

    assert!(dangling_sequence_entries(&quest).is_empty());
}

/// The check covers EVERY sequence, not the active one: Save writes the whole quest block, so a
/// dangling entry in a sequence the player is not looking at is still about to be saved.
#[test]
fn an_inactive_sequence_is_checked_too() {
    let mut quest = quest_with_flows(&["a"]);
    quest
        .sequences
        .insert("active".to_owned(), sequence_of(&["a"]));
    quest
        .sequences
        .insert("other".to_owned(), sequence_of(&["gone"]));
    quest.active_sequence = "active".to_owned();

    let dangling = dangling_sequence_entries(&quest);
    assert_eq!(dangling.len(), 1);
    assert_eq!(dangling[0].sequence, "other");
}

/// The order is fixed so the same defect reads the same way twice — the message lists every hit, and
/// a message whose lines move between saves is one the player cannot compare. Sorted ignore-case,
/// which is why "Zeta" sorts after "alpha" rather than before it.
#[test]
fn the_report_is_ordered_by_sequence_ignore_case_then_position() {
    let mut quest = quest_with_flows(&[]);
    quest
        .sequences
        .insert("Zeta".to_owned(), sequence_of(&["missing"]));
    quest
        .sequences
        .insert("alpha".to_owned(), sequence_of(&["missing", "missing"]));

    let found: Vec<(String, usize)> = dangling_sequence_entries(&quest)
        .into_iter()
        .map(|entry| (entry.sequence, entry.entry))
        .collect();
    assert_eq!(
        found,
        vec![
            ("alpha".to_owned(), 1),
            ("alpha".to_owned(), 2),
            ("Zeta".to_owned(), 1)
        ]
    );
}

/// The Save refusal, word for word from the launcher's Quest Save handler: it lists every hit, says
/// which of the two defects it is, and tells the player how to get out of it.
#[test]
fn the_save_gate_refuses_with_every_hit_and_an_instruction() {
    let mut quest = quest_with_flows(&[]);
    quest
        .sequences
        .insert("nightly".to_owned(), sequence_of(&["deleted"]));
    quest
        .sequences
        .insert("empty".to_owned(), sequence_of(&[""]));

    let err = check_saveable(&quest).unwrap_err();
    assert_eq!(
        err.message(),
        "\"empty\" entry 1 has no flow chosen\n\
         \"nightly\" entry 1 names the flow \"deleted\", which does not exist\n\
         Pick an existing flow for each entry, or remove it, then Save again."
    );
}

// ── the step rule (`QuestAction.Complaint`) ──────────────────────────────────────────────────────

#[test]
fn every_sendable_step_has_no_complaint() {
    for action in [
        quest_action::CLICK,
        quest_action::RIGHT_CLICK,
        quest_action::ENTER,
        quest_action::WAIT,
    ] {
        let step = QuestStep {
            action: action.to_owned(),
            ..QuestStep::default()
        };
        assert_eq!(quest_step_complaint(&step), None, "action {action}");
    }

    let key = QuestStep {
        action: quest_action::KEY.to_owned(),
        value: Some("5".to_owned()),
        ..QuestStep::default()
    };
    assert_eq!(quest_step_complaint(&key), None);
}

/// A `key` step with no character, or with two, is a step that does nothing — and in a flow replayed
/// 500 times, a step that does nothing is indistinguishable from a quest that behaves differently
/// today. The launcher refuses such a flow before the run starts, and so does this.
#[test]
fn an_unsendable_key_step_says_exactly_what_is_wrong() {
    let missing = QuestStep {
        action: quest_action::KEY.to_owned(),
        value: None,
        ..QuestStep::default()
    };
    assert_eq!(
        quest_step_complaint(&missing).as_deref(),
        Some("a key step with no character would send nothing at all.")
    );

    let two_chars = QuestStep {
        action: quest_action::KEY.to_owned(),
        value: Some("ab".to_owned()),
        ..QuestStep::default()
    };
    let complaint = quest_step_complaint(&two_chars).unwrap();
    assert!(complaint.contains("ONE character"), "got {complaint:?}");
    assert!(complaint.contains("is 2"), "got {complaint:?}");

    let unprintable = QuestStep {
        action: quest_action::KEY.to_owned(),
        value: Some("é".to_owned()),
        ..QuestStep::default()
    };
    assert_eq!(
        quest_step_complaint(&unprintable).as_deref(),
        Some("\"é\" is not a printable character the board can press.")
    );
}

#[test]
fn an_unknown_action_is_refused_by_naming_the_ones_that_exist() {
    let step = QuestStep {
        action: "Click".to_owned(), // the values are lowercase; v2's check is exact
        ..QuestStep::default()
    };
    let complaint = quest_step_complaint(&step).unwrap();
    assert_eq!(
        complaint,
        "\"Click\" is not a step this tool can send — pick one of \
         click, right_click, key, enter, wait."
    );
}

// ── rename / re-point ────────────────────────────────────────────────────────────────────────────

fn preset_with_steps() -> QuestPreset {
    QuestPreset {
        initial_wait_seconds: 3.0,
        loops: 2,
        steps: vec![QuestStep {
            action: quest_action::KEY.to_owned(),
            value: Some("1".to_owned()),
            ..QuestStep::default()
        }],
    }
}

/// A rename keeps the same flow under a new name, so the entries that named it follow the name.
/// Without this they would be left pointing at a flow that no longer exists, and Save would refuse
/// the sequence the rename was meant to leave working.
#[test]
fn renaming_a_flow_moves_its_body_and_re_points_every_entry_that_named_it() {
    let mut quest = quest_with_flows(&["red", "blue"]);
    quest.presets.insert("red".to_owned(), preset_with_steps());
    quest.active = "red".to_owned();
    quest
        .sequences
        .insert("nightly".to_owned(), sequence_of(&["red", " blue "]));

    let repointed = rename_flow(&mut quest, "red", "crimson").unwrap();

    assert_eq!(repointed, 1);
    assert!(!quest.presets.contains_key("red"));
    assert_eq!(quest.presets["crimson"].initial_wait_seconds, 3.0);
    assert_eq!(
        quest.presets["crimson"].steps[0].value.as_deref(),
        Some("1")
    );
    // The active flow's target is a NAME, so it follows too, or Start would run another flow.
    assert_eq!(quest.active, "crimson");
    assert_eq!(quest.sequences["nightly"].entries[0].preset, "crimson");
    // The comparison trims, exactly like the runner: " blue " names blue and is left alone.
    assert_eq!(quest.sequences["nightly"].entries[1].preset, " blue ");
}

#[test]
fn renaming_a_flow_onto_an_existing_name_is_refused_without_moving_anything() {
    let mut quest = quest_with_flows(&["red", "blue"]);
    quest.active = "red".to_owned();

    let err = rename_flow(&mut quest, "red", "blue").unwrap_err();

    assert_eq!(
        err.message(),
        "There is already a flow called \"blue\" — pick another name."
    );
    assert!(quest.presets.contains_key("red"));
    assert!(quest.presets.contains_key("blue"));
    assert_eq!(quest.active, "red");
}

#[test]
fn renaming_a_flow_to_its_own_name_is_a_no_op_that_succeeds() {
    let mut quest = quest_with_flows(&["red"]);

    assert_eq!(rename_flow(&mut quest, "red", "red").unwrap(), 0);
    assert!(quest.presets.contains_key("red"));
}

#[test]
fn renaming_a_flow_that_is_not_there_says_which_picker_to_use() {
    let mut quest = quest_with_flows(&["red"]);

    let err = rename_flow(&mut quest, "gone", "other").unwrap_err();
    assert!(matches!(
        err,
        PresetError::Missing {
            noun: PresetNoun::Flow,
            action: PresetAction::Rename
        }
    ));
    assert_eq!(err.message(), "Pick a flow to rename.");
}

#[test]
fn renaming_a_sequence_moves_it_and_keeps_the_active_choice_aimed_at_it() {
    let mut quest = quest_with_flows(&["red"]);
    quest
        .sequences
        .insert("nightly".to_owned(), sequence_of(&["red"]));
    quest.active_sequence = "nightly".to_owned();

    rename_sequence(&mut quest, "nightly", "weekly").unwrap();

    assert!(!quest.sequences.contains_key("nightly"));
    assert_eq!(quest.sequences["weekly"].entries[0].preset, "red");
    assert_eq!(quest.active_sequence, "weekly");
}

#[test]
fn renaming_a_sequence_onto_an_existing_name_is_refused() {
    let mut quest = quest_with_flows(&["red"]);
    quest
        .sequences
        .insert("a".to_owned(), sequence_of(&["red"]));
    quest
        .sequences
        .insert("b".to_owned(), sequence_of(&["red"]));

    let err = rename_sequence(&mut quest, "a", "b").unwrap_err();
    assert_eq!(
        err.message(),
        "There is already a sequence called \"b\" — pick another name."
    );
}

// ── delete ───────────────────────────────────────────────────────────────────────────────────────

/// The last flow cannot be deleted: the Quest tab would have nothing to run, and v2 refused with this
/// exact line before the MessageBox.
#[test]
fn the_last_flow_cannot_be_deleted() {
    let mut quest = quest_with_flows(&["only"]);

    let err = remove_flow(&mut quest, "only").unwrap_err();
    assert_eq!(
        err.message(),
        "This is the only flow — create another before deleting it."
    );
    assert!(quest.presets.contains_key("only"));
}

#[test]
fn deleting_the_active_flow_leaves_the_picker_on_one_that_still_exists() {
    let mut quest = quest_with_flows(&["red", "blue"]);
    quest.active = "red".to_owned();

    remove_flow(&mut quest, "red").unwrap();

    assert!(!quest.presets.contains_key("red"));
    assert!(quest.presets.contains_key(&quest.active));
    assert_eq!(quest.active, "blue");
}

/// Deleting a flow a sequence still names is ALLOWED — the Save gate is where the player is stopped
/// and told, rather than the delete guessing a replacement the player did not choose.
#[test]
fn deleting_a_referenced_flow_puts_the_refusal_at_save_time() {
    let mut quest = quest_with_flows(&["red", "blue"]);
    quest
        .sequences
        .insert("nightly".to_owned(), sequence_of(&["red"]));

    remove_flow(&mut quest, "red").unwrap();

    let err = check_saveable(&quest).unwrap_err();
    assert!(err.message().contains("\"red\""), "{}", err.message());
    assert_eq!(dangling_sequence_entries(&quest)[0].flow, "red");
}

#[test]
fn deleting_a_flow_that_is_not_there_says_so() {
    let mut quest = quest_with_flows(&["red", "blue"]);

    assert_eq!(
        remove_flow(&mut quest, "gone").unwrap_err().message(),
        "Pick a flow to delete."
    );
}

// ── duplicate ───────────────────────────────────────────────────────────────────────────────────

#[test]
fn duplicating_a_flow_copies_its_body_under_a_generated_name_and_selects_it() {
    let mut quest = quest_with_flows(&["red"]);
    quest.presets.insert("red".to_owned(), preset_with_steps());

    let copy_name = duplicate_flow(&mut quest, "red").unwrap();

    assert_eq!(copy_name, "red (copy)");
    assert_eq!(quest.active, "red (copy)");
    assert_eq!(quest.presets.len(), 2);
    assert_eq!(
        quest.presets[&copy_name].steps[0].value.as_deref(),
        Some("1")
    );

    // A SECOND field on the copy, independent of the source: a shallow copy would edit both, and
    // nothing on screen would say so. Rust's Clone is deep, so this sentence is the whole port of
    // `PresetCopy.CopyQuestPreset`.
    quest.presets.get_mut(&copy_name).unwrap().loops = 9;
    assert_eq!(quest.presets["red"].loops, 2);
}

#[test]
fn duplicating_twice_generates_the_next_free_name() {
    let mut quest = quest_with_flows(&["red"]);

    assert_eq!(duplicate_flow(&mut quest, "red").unwrap(), "red (copy)");
    assert_eq!(duplicate_flow(&mut quest, "red").unwrap(), "red (copy 2)");
}

#[test]
fn a_duplicated_sequence_still_names_the_same_flows() {
    let mut quest = quest_with_flows(&["red"]);
    quest
        .sequences
        .insert("nightly".to_owned(), sequence_of(&["red"]));

    let copy_name = duplicate_sequence(&mut quest, "nightly").unwrap();

    assert_eq!(copy_name, "nightly (copy)");
    assert_eq!(quest.active_sequence, "nightly (copy)");
    // Duplicating a sequence is not a reason to duplicate every flow in it, and pointing the copy at
    // a flow of its own is one edit in its entry editor.
    assert_eq!(quest.sequences[&copy_name].entries[0].preset, "red");
}

#[test]
fn duplicating_something_that_is_not_there_says_which_picker_to_use() {
    let mut quest = quest_with_flows(&["red"]);

    assert_eq!(
        duplicate_flow(&mut quest, "gone").unwrap_err().message(),
        "Pick a flow to copy."
    );
    assert_eq!(
        duplicate_sequence(&mut quest, "gone")
            .unwrap_err()
            .message(),
        "Pick a sequence to copy."
    );
}
