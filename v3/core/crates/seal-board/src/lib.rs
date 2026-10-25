//! The Arduino board side: the level-2 command grammar, the write/read transport seam, and the
//! closed-loop cursor placement that v2's `HidPointer`/`HidController` performed.
//!
//! Pure over a transport trait for now: the OS serial port is not opened here (that adapter is
//! blocked on the Windows toolchain), so a fake transport drives every test. The measured rules
//! that must survive: `D dx dy` counts are RAW HID counts and are never scaled; placement is a
//! closed loop over the cursor position with a step budget that reports the goal and where it
//! stopped; `V` is the only reply the sketch ever sends, and it is how a level-2 board is told from
//! a silent level-1 one; `SleepCheck` counts SECONDS (the launcher's `Task.Delay` counts
//! milliseconds — the eleven-minute-sleep incident).
