//! How the pet feeder puts a food stack into the boarding window: right-click, or a drag.
//!
//! THIS IS THE ONE PLACE A BOARD VERSION CHANGES WHAT THE TOOL DOES rather than merely being printed,
//! and getting the "can this board drag?" answer wrong in the permissive direction is the expensive
//! mistake: the drag letters are ignored by an old board, the button is never pressed, and the stack
//! is never picked up — which looks exactly like a mis-aimed drag, so the feeder would report
//! successful reloads while loading no food at all (`FoodLoadModeTests.cs`'s own warning).
//!
//! THE GAME RULE BEHIND THE SETTING: a right-click drops the stack into the EARLIEST empty food box in
//! the boarder, and every row's boxes form one queue ordered top-down — so a stack meant for row 3
//! lands in row 1's box whenever row 1 has run dry. A drag names the box. Both are kept because a drag
//! needs firmware level 2 while a right-click works on every board ever flashed, so the setting is per
//! machine and the player can switch the day they reflash (`FoodLoadMode.cs:5-14`).
//!
//! Ported from `v2/SealTools.Core/FoodLoadMode.cs` with `FoodLoadModeTests.cs` as the oracle; the
//! firmware level numbers are the two constants the rule needs from `FirmwareVersion.cs`
//! (`DragLevel = 2`, `Current = 2`). The rest of the firmware protocol (the `V` query and its
//! messages) belongs to `seal-board`, which is why only the level constant is here.

/// Right-click the bag stack; the game chooses the box. Works on any firmware.
pub const RIGHT_CLICK: &str = "right_click";

/// Press on the bag stack, drag to the row's own food box, release. Needs firmware 2.
pub const DRAG: &str = "drag";

/// Every value the setting accepts, in the order the picker shows them (`FoodLoadMode.cs:24`). The Pet
/// tab builds its dropdown from this list, so anything missing here is unreachable in the UI.
pub const ALL: [&str; 2] = [RIGHT_CLICK, DRAG];

/// v2's `PetConfig.FoodLoadMode` initialiser (`AppConfig.cs:536`). Not a style preference: a
/// right-click works on a board that has never been reflashed, and a silent no-op is the failure this
/// setting must not default into.
pub const DEFAULT: &str = RIGHT_CLICK;

/// `FirmwareVersion.DragLevel` (`FirmwareVersion.cs:29`). The level at which the sketch grew the
/// `L`/`l` drag commands. `Current` is deliberately not here: the board crate owns the protocol, and
/// nothing in this rule needs to know the newest level.
pub const FIRMWARE_DRAG_LEVEL: i32 = 2;

/// Whether the configured value asks for a drag (`FoodLoadMode.cs:27-28`).
///
/// Read LOOSELY — trimmed and case-insensitive — because it comes out of a hand-editable YAML file. It
/// is NOT prefix matching: `dragn` and `drags` are not a drag, and neither is `rightclick`.
pub fn is_drag(mode: Option<&str>) -> bool {
    mode.map(str::trim)
        .is_some_and(|mode| mode.eq_ignore_ascii_case(DRAG))
}

/// Whether a board at `firmware_level` can do a drag (`FoodLoadMode.cs:36`).
///
/// `None` means the board never answered — which is a DEFINITE NO, not an unknown: the sketch that
/// predates version reporting also predates `L`/`l`. Treating it as "maybe" is what would let an
/// un-reflashed board be trusted with a drag.
pub fn board_can_drag(firmware_level: Option<i32>) -> bool {
    firmware_level.is_some_and(|level| level >= FIRMWARE_DRAG_LEVEL)
}

/// What to do about the setting and the board disagreeing, or `None` when they agree
/// (`FoodLoadMode.cs:41-52`).
///
/// A sentence for the card, because "the drag did nothing" and "the board is too old to drag" look
/// identical from outside. The reported-level form names the level it saw, so "wrong board" and "wrong
/// setting" can be told apart from the message alone — the player cannot see the board.
pub fn complaint(mode: Option<&str>, firmware_level: Option<i32>) -> Option<String> {
    if !is_drag(mode) {
        return None;
    }
    if board_can_drag(firmware_level) {
        return None;
    }

    Some(match firmware_level {
        Some(level) => format!(
            "Food load is set to drag but the board reports firmware {level}, which has no drag \
             commands — reflash the board, or set the Pet tab's food load back to right-click."
        ),
        None => "Food load is set to drag but the board did not report a firmware version, so it \
                 predates the drag commands — reflash the board, or set the Pet tab's food load back \
                 to right-click."
            .to_owned(),
    })
}
