//! The effects layer: what a tool's `Decision` actually does to the machine.
//!
//! `seal-tools`' runner performs `Click(Point)` and `Key(Key)` through `Effects`; this module
//! implements that over the real hardware seams: board keys through the serial grammar, and a click
//! as a closed-loop cursor placement (`seal-board::placement`) followed by the firmware's left click.
//!
//! Two contract points, both load-bearing:
//!
//! - **A failed placement never clicks.** The effect error carries the placement's own sentence, so
//!   the card says where the cursor actually stopped rather than clicking blind.
//! - **The cursor space is the process's own.** The core is DPI-unaware on purpose, so `GetCursorPos`
//!   reads and the firmware's raw `D` counts are compared in the same (virtualized) space, and the
//!   goal is [`DisplayInfo::cursor_target`]'s LOGICAL point — the one place the division by scale
//!   happens. The physical point is for capture and the shell, never for this loop.
//!
//! Everything here is generic over the board transport, the cursor and the clock, so the whole layer
//! is testable with `FakeTransport`, a scripted cursor and a recording clock — no hardware.

use seal_board::clock::{Clock, RealClock};
use seal_board::command::BoardCommand;
use seal_board::placement::{Cursor, place};
use seal_board::transport::Transport;
use seal_domain::decision::Key;
use seal_domain::geometry::{DisplayInfo, Point};
use seal_tools::runner::{EffectError, Effects};

/// The production transport is `seal_board::serial::SerialTransport`; this layer only needs the
/// trait, which is what makes the tests hardware-free.
pub struct BoardEffects<T: Transport, C: Cursor, K: Clock> {
    transport: T,
    cursor: C,
    clock: K,
    display: DisplayInfo,
    /// True between a successful `hold_left` and its release. `Drop` releases it, because a drag
    /// abandoned by a cancelled run would otherwise leave the host's button down — the same rule the
    /// spacebar hold follows, at the same seam.
    held_left: bool,
}

impl<T: Transport, C: Cursor, K: Clock> BoardEffects<T, C, K> {
    pub fn new(transport: T, cursor: C, clock: K, display: DisplayInfo) -> Self {
        Self {
            transport,
            cursor,
            clock,
            display,
            held_left: false,
        }
    }

    /// The frame's display measurement, for the placement's logical goal.
    pub fn display(&self) -> &DisplayInfo {
        &self.display
    }

    /// The transport, so a test (or the supervisor's teardown) can inspect what was written.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    fn write(&mut self, command: BoardCommand) -> Result<(), EffectError> {
        self.transport
            .write_line(&command.encode())
            .map_err(|error| EffectError::new(error.message()))
    }

    /// Place the cursor at a client-relative PHYSICAL offset and DO NOT click.
    pub fn place_at(&mut self, at: Point) -> Result<(), EffectError> {
        let target = self.display.cursor_target(at.x, at.y).logical;
        let placement = place(
            &mut self.transport,
            &mut self.cursor,
            &self.clock,
            (target.x, target.y),
        );
        if !placement.ok {
            return Err(EffectError::new(placement.error.unwrap_or_else(|| {
                "the cursor could not be placed, so nothing was clicked".to_owned()
            })));
        }
        Ok(())
    }

    /// Place the cursor at a client-relative PHYSICAL offset, then click. The placement loop is the
    /// board crate's; a failed placement never clicks.
    pub fn click_at(&mut self, at: Point) -> Result<(), EffectError> {
        self.place_at(at)?;
        self.write(BoardCommand::LeftClick)
    }
}

impl<T: Transport, C: Cursor, K: Clock> Effects for BoardEffects<T, C, K> {
    fn click(&mut self, at: Point) -> Result<(), EffectError> {
        self.click_at(at)
    }

    fn key(&mut self, key: Key) -> Result<(), EffectError> {
        self.write(key_command(key, false))
    }

    fn key_fast(&mut self, key: Key) -> Result<(), EffectError> {
        self.write(key_command(key, true))
    }

    fn hold_space(&mut self) -> Result<(), EffectError> {
        self.write(BoardCommand::HoldSpace)
    }

    fn release_space(&mut self) -> Result<(), EffectError> {
        self.write(BoardCommand::ReleaseSpace)
    }

    fn enter(&mut self) -> Result<(), EffectError> {
        self.write(BoardCommand::Enter)
    }

    fn place(&mut self, at: Point) -> Result<(), EffectError> {
        self.place_at(at)
    }

    fn click_here(&mut self) -> Result<(), EffectError> {
        self.write(BoardCommand::LeftClick)
    }

    fn right_click_here(&mut self) -> Result<(), EffectError> {
        self.write(BoardCommand::RightClick)
    }

    fn move_by(&mut self, dx: i32, dy: i32) -> Result<(), EffectError> {
        self.write(BoardCommand::move_by(dx, dy))
    }

    fn hold_left(&mut self) -> Result<(), EffectError> {
        self.write(BoardCommand::HoldLeft)?;
        self.held_left = true;
        Ok(())
    }

    fn release_left(&mut self) -> Result<(), EffectError> {
        self.write(BoardCommand::ReleaseLeft)?;
        self.held_left = false;
        Ok(())
    }

    fn wheel(&mut self, notches: i32, down: bool) -> Result<(), EffectError> {
        let Ok(notches) = u16::try_from(notches) else {
            return Err(EffectError::new(
                "a wheel scroll needs a positive notch count",
            ));
        };
        let command = BoardCommand::wheel(!down, notches)
            .map_err(|error| EffectError::new(error.message()))?;
        self.write(command)
    }
}

fn key_command(key: Key, fast: bool) -> BoardCommand {
    match key {
        Key::Char(character) => BoardCommand::Key { character, fast },
        Key::Function(number) => BoardCommand::FunctionKey { number, fast },
    }
}

impl<T: Transport, C: Cursor, K: Clock> Drop for BoardEffects<T, C, K> {
    fn drop(&mut self) {
        if self.held_left {
            // Best effort: `Drop` cannot report, and the firmware is the backstop. The explicit
            // `release_left` is where a failure could still become a player message.
            let _ = self
                .transport
                .write_line(BoardCommand::ReleaseLeft.encode().as_str());
            self.held_left = false;
        }
    }
}

/// The production cursor: `GetCursorPos` in this (DPI-unaware) process's space.
#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
pub struct WinCursor;

#[cfg(windows)]
impl Cursor for WinCursor {
    fn position(&mut self) -> Option<(i32, i32)> {
        seal_win::window::cursor_position().map(|point| (point.x, point.y))
    }
}

/// The effects a real tool gets at run start: a serial transport opened by the caller, the real
/// cursor read, the real clock, and the display measurement captured when the run began.
#[cfg(windows)]
pub fn system_effects(
    transport: seal_board::serial::SerialTransport,
    display: DisplayInfo,
) -> BoardEffects<seal_board::serial::SerialTransport, WinCursor, RealClock> {
    BoardEffects::new(transport, WinCursor, RealClock, display)
}

#[cfg(test)]
mod tests {
    use super::*;
    use seal_board::transport::FakeTransport;
    use seal_domain::geometry::{Rect, Size};

    /// A clock that does not sleep but records the waits, so a placement test is instant. The
    /// counter is shared so a test can still read it after the effects own the clock.
    #[derive(Default, Clone)]
    struct StepClock {
        waited: std::rc::Rc<std::cell::Cell<f64>>,
    }

    impl Clock for StepClock {
        fn sleep_seconds(&self, seconds: f64) {
            self.waited.set(self.waited.get() + seconds);
        }
    }

    /// Records the drag's two writes in shared storage so the `Drop` release can be asserted after
    /// the effects object is gone.
    #[derive(Default)]
    struct RecordingEffects {
        writes: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }

    impl RecordingEffects {
        fn record_hold_and_release(&mut self) {
            let mut effects = BoardEffects::new(
                RecorderTransport {
                    writes: self.writes.clone(),
                },
                ScriptedCursor::new(vec![Some((100, 50))]),
                StepClock::default(),
                display(),
            );
            effects.hold_left().expect("holds");
            // Dropping releases; that is the property under test.
        }

        fn writes(&self) -> Vec<String> {
            self.writes.borrow().clone()
        }
    }

    struct RecorderTransport {
        writes: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }

    impl Transport for RecorderTransport {
        fn write_line(&mut self, wire: &str) -> Result<(), seal_board::transport::TransportError> {
            self.writes.borrow_mut().push(wire.to_owned());
            Ok(())
        }

        fn read_line(
            &mut self,
            _deadline: seal_board::transport::Deadline,
        ) -> seal_board::transport::LineRead {
            seal_board::transport::LineRead::Silent
        }
    }

    /// A cursor whose positions are scripted; the last one repeats, like a real cursor that stopped.
    struct ScriptedCursor {
        positions: Vec<Option<(i32, i32)>>,
        index: usize,
    }

    impl ScriptedCursor {
        fn new(positions: Vec<Option<(i32, i32)>>) -> Self {
            Self {
                positions,
                index: 0,
            }
        }
    }

    impl Cursor for ScriptedCursor {
        fn position(&mut self) -> Option<(i32, i32)> {
            let position = self
                .positions
                .get(self.index.min(self.positions.len().saturating_sub(1)))
                .copied()
                .flatten();
            self.index += 1;
            position
        }
    }

    fn display() -> DisplayInfo {
        // 1:1 scale, so logical and physical coincide and the target arithmetic is transparent.
        DisplayInfo::from_rects(
            Rect::new(100, 40, 820, 640),
            Rect::new(100, 50, 800, 600),
            Rect::new(100, 50, 800, 600),
            Size::new(1920, 1080),
        )
    }

    fn effects<T: Transport, C: Cursor, K: Clock>(
        transport: T,
        cursor: C,
        clock: K,
    ) -> BoardEffects<T, C, K> {
        BoardEffects::new(transport, cursor, clock, display())
    }

    #[test]
    fn keys_are_exactly_one_grammar_line() {
        let mut fx = effects(
            FakeTransport::new(),
            ScriptedCursor::new(vec![Some((100, 50))]),
            StepClock::default(),
        );

        fx.key(Key::Char('1')).expect("a character key");
        fx.key(Key::Function(12)).expect("a function key");

        assert_eq!(fx.transport().writes(), vec!["K 1\n", "F 12\n"]);
    }

    #[test]
    fn a_click_moves_the_cursor_with_the_closed_loop_then_clicks() {
        let clock = StepClock::default();
        let waits = clock.waited.clone();
        let mut fx = effects(
            FakeTransport::new(),
            ScriptedCursor::new(vec![Some((0, 0)), Some((110, 70))]),
            clock,
        );

        // Client-relative (10, 20) at 1:1 scale is logical screen (110, 70).
        fx.click(Point::new(10, 20)).expect("places then clicks");

        assert_eq!(fx.transport().writes(), vec!["D 110 70\n", "C\n"]);
        assert!(waits.get() > 0.0, "the loop waits for the cursor");
    }

    #[test]
    fn a_click_on_an_already_placed_cursor_writes_only_the_click() {
        let mut fx = effects(
            FakeTransport::new(),
            ScriptedCursor::new(vec![Some((110, 70))]),
            StepClock::default(),
        );

        fx.click(Point::new(10, 20)).expect("already there");

        assert_eq!(fx.transport().writes(), vec!["C\n"]);
    }

    #[test]
    fn an_unreadable_cursor_is_an_error_and_never_a_click() {
        let mut fx = effects(
            FakeTransport::new(),
            ScriptedCursor::new(vec![None]),
            StepClock::default(),
        );

        let error = fx.click(Point::new(10, 20)).expect_err("no read, no click");
        assert!(
            error.message().to_lowercase().contains("cursor"),
            "the sentence names the cursor: {}",
            error.message()
        );
        assert!(
            fx.transport().writes().is_empty()
                || !fx.transport().writes().iter().any(|w| w == "C\n"),
            "a failed placement must not click"
        );
    }

    #[test]
    fn enter_is_the_firmwares_own_command_not_a_printable_key() {
        let mut fx = effects(
            FakeTransport::new(),
            ScriptedCursor::new(vec![Some((100, 50))]),
            StepClock::default(),
        );

        fx.enter().expect("Enter is a plain write");

        assert_eq!(
            fx.transport().writes(),
            vec!["E\n"],
            "`E`, never `K E`: the firmware's Enter is what a dialog accepts"
        );
    }

    #[test]
    fn place_moves_without_clicking_and_click_here_writes_only_the_click() {
        let mut fx = effects(
            FakeTransport::new(),
            ScriptedCursor::new(vec![Some((0, 0)), Some((110, 70))]),
            StepClock::default(),
        );

        fx.place(Point::new(10, 20)).expect("a bare placement");
        assert_eq!(
            fx.transport().writes(),
            vec!["D 110 70\n"],
            "place never clicks"
        );

        fx.click_here().expect("a bare click");
        fx.right_click_here().expect("a bare right click");
        fx.wheel(3, true).expect("three notches down");
        fx.wheel(2, false).expect("two notches up");
        fx.move_by(-40, 512).expect("a raw HID move");
        fx.hold_left().expect("hold the button");
        fx.release_left().expect("release the button");
        assert_eq!(
            fx.transport().writes(),
            vec![
                "D 110 70\n",
                "C\n",
                "R\n",
                "Z 3\n",
                "Q 2\n",
                "D -40 512\n",
                "L\n",
                "l\n"
            ],
            "bare clicks write C/R; a down scroll is Z, an up scroll Q; a raw move keeps its counts; \
             the drag is L then l"
        );
    }

    #[test]
    fn dropping_while_the_button_is_held_releases_it() {
        let mut fx = effects(
            FakeTransport::new(),
            ScriptedCursor::new(vec![Some((100, 50))]),
            StepClock::default(),
        );

        fx.hold_left().expect("holds");
        assert_eq!(fx.transport().writes(), vec!["L\n"]);
        drop(fx);

        // The guard lives on the transport, so the assertion reads a second instance's log: the drop
        // above moved the transport into the effects that were dropped. This test proves the write
        // happened by exercising the same path with a shared recorder instead.
        let mut recorder = RecordingEffects::default();
        recorder.record_hold_and_release();
        assert_eq!(recorder.writes(), vec!["L\n", "l\n"]);
    }

    #[test]
    fn a_failed_write_becomes_the_effects_sentence() {
        let mut transport = FakeTransport::new();
        transport.fail_writes("the board would not take the move");
        let mut fx = effects(
            transport,
            ScriptedCursor::new(vec![Some((100, 50))]),
            StepClock::default(),
        );

        let error = fx.key(Key::Char('1')).expect_err("the write failed");
        assert_eq!(error.message(), "the board would not take the move");
    }
}
