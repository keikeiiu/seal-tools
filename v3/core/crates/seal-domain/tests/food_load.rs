//! `FoodLoadModeTests.cs` translated — the 17 v2 cases, with the `[Theory]` rows folded into loops.
//!
//! The board version changes BEHAVIOUR here, not only a printed line, and the expensive failure is
//! the permissive one: an old board silently ignores a drag, so the feeder reports a successful
//! reload while loading no food. These cases guard that direction.

use seal_domain::tools::pet::food_load::{
    ALL, DEFAULT, DRAG, FIRMWARE_DRAG_LEVEL, RIGHT_CLICK, board_can_drag, complaint, is_drag,
};

/// `TheDefaultIsTheModeThatWorksOnEveryBoard` (`FoodLoadModeTests.cs:14-21`): the default is the mode
/// that works on a board that has never been reflashed.
#[test]
fn the_default_is_the_mode_that_works_on_every_board() {
    assert_eq!(DEFAULT, RIGHT_CLICK);
    assert!(!is_drag(Some(DEFAULT)));
    assert!(is_drag(Some(DRAG)));
}

/// `TheValueIsReadLoosely` (`:23-27`) — trimmed and case-insensitive.
#[test]
fn the_value_is_read_loosely() {
    for mode in ["drag", "DRAG", "  drag  "] {
        assert!(is_drag(Some(mode)), "mode {mode:?}");
    }
}

/// `AnythingElseIsNotADrag` (`:29-36`) — including the near misses a hand edit produces.
#[test]
fn anything_else_is_not_a_drag() {
    for mode in [
        None,
        Some(""),
        Some("right_click"),
        Some("rightclick"),
        Some("dragn"),
        Some("drags"),
    ] {
        assert!(!is_drag(mode), "mode {mode:?}");
    }
}

/// `ThePickerOffersEveryMode` (`:38-44`) — the Pet tab builds its dropdown from `ALL`.
#[test]
fn the_picker_offers_every_mode() {
    assert!(ALL.contains(&RIGHT_CLICK));
    assert!(ALL.contains(&DRAG));
    assert_eq!(ALL, [RIGHT_CLICK, DRAG], "v2's picker order");
}

/// `ABoardAtTheDragLevelCanDrag` (`:46-52`) — the drag level itself, the current level, and newer.
#[test]
fn a_board_at_the_drag_level_can_drag() {
    for level in [
        FIRMWARE_DRAG_LEVEL,
        FIRMWARE_DRAG_LEVEL,
        FIRMWARE_DRAG_LEVEL + 5,
    ] {
        assert!(board_can_drag(Some(level)), "level {level}");
    }
}

/// `ABoardBelowItCannot` (`:54-59`).
#[test]
fn a_board_below_it_cannot() {
    assert!(!board_can_drag(Some(FIRMWARE_DRAG_LEVEL - 1)));
    assert!(!board_can_drag(Some(0)));
}

/// `ASilentBoardIsADefiniteNoAndNotAnUnknown` (`:61-69`): the sketch that predates version reporting
/// also predates the drag commands, so silence is an answer.
#[test]
fn a_silent_board_is_a_definite_no_and_not_an_unknown() {
    assert!(!board_can_drag(None));
    let message = complaint(Some(DRAG), None).unwrap();
    assert!(message.contains("did not report"), "{message}");
}

/// `RightClickIsNeverAComplaintWhateverTheBoardSays` (`:71-79`): right-click is the fallback that any
/// board can do, so it must never block a start.
#[test]
fn right_click_is_never_a_complaint_whatever_the_board_says() {
    for level in [Some(FIRMWARE_DRAG_LEVEL), Some(1), None] {
        assert_eq!(complaint(Some(RIGHT_CLICK), level), None, "level {level:?}");
    }
    assert_eq!(complaint(None, None), None);
}

/// `DragOnACapableBoardIsNotAComplaint` (`:81-86`).
#[test]
fn drag_on_a_capable_board_is_not_a_complaint() {
    assert_eq!(complaint(Some(DRAG), Some(FIRMWARE_DRAG_LEVEL)), None);
    assert_eq!(complaint(Some(DRAG), Some(FIRMWARE_DRAG_LEVEL + 5)), None);
}

/// `DragOnAnOldBoardSaysWhichBoardAndWhatToDoAboutIt` (`:88-98`): it names the level it saw, so "wrong
/// board" and "wrong setting" are distinguishable from the message alone.
#[test]
fn drag_on_an_old_board_says_which_board_and_what_to_do_about_it() {
    let message = complaint(Some(DRAG), Some(FIRMWARE_DRAG_LEVEL - 1)).unwrap();

    assert!(message.contains("reflash"), "{message}");
    assert!(
        message.contains(&(FIRMWARE_DRAG_LEVEL - 1).to_string()),
        "the message must name the level it saw: {message}"
    );
}

/// The two messages verbatim, because they are the ones the player acts on.
#[test]
fn the_complaint_texts_are_v2s() {
    assert_eq!(
        complaint(Some(DRAG), Some(1)).unwrap(),
        "Food load is set to drag but the board reports firmware 1, which has no drag commands — \
         reflash the board, or set the Pet tab's food load back to right-click."
    );
    assert_eq!(
        complaint(Some(DRAG), None).unwrap(),
        "Food load is set to drag but the board did not report a firmware version, so it predates the \
         drag commands — reflash the board, or set the Pet tab's food load back to right-click."
    );
}
