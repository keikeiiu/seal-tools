//! G20: `tool.start {..., "dry_run": true}` — the Buy/Sell dry run.
//!
//! A dry run validates the same plan and drives the same closed-loop cursor placement and wheel
//! scrolls a real pass would, but it NEVER clicks, presses Enter or sends a key. The wrapper records
//! every suppressed action and writes one `dry run:` line per action into the tool's log, so the card
//! and `logs/` show exactly what the pass would have done. A failed placement/scroll still fails the
//! run (the point is to validate the route, not to pretend the hardware is fine).
//!
//! The wrapper is generic over the real effects so the tests can use a fake transport and prove the
//! suppressed calls never reach it.

use std::sync::Arc;

use seal_domain::decision::Key;
use seal_domain::geometry::Point;
use seal_tools::runner::{EffectError, Effects};

use crate::logs::Logs;
use seal_tools::tool::ToolId;

/// Forward placements/scrolls (they are how a dry run proves the route), suppress every CLICK and
/// KEY (they are how a dry run would change the game). Every action is recorded for the log.
pub struct DryRunEffects<E> {
    inner: E,
    log: Arc<Logs>,
    tool: ToolId,
    actions: Vec<String>,
}

impl<E> DryRunEffects<E> {
    pub fn new(inner: E, log: Arc<Logs>, tool: ToolId) -> Self {
        Self {
            inner,
            log,
            tool,
            actions: Vec::new(),
        }
    }

    fn record(&mut self, action: String) {
        self.actions.push(action);
    }
}

impl<E: Effects> Effects for DryRunEffects<E> {
    fn place(&mut self, at: Point) -> Result<(), EffectError> {
        self.inner.place(at)?;
        self.record(format!("place ({}, {})", at.x, at.y));
        Ok(())
    }

    fn click(&mut self, at: Point) -> Result<(), EffectError> {
        // A dry run never moves-then-clicks: record the intended click and do not touch the game.
        self.record(format!("would click ({}, {})", at.x, at.y));
        Ok(())
    }

    fn key(&mut self, key: Key) -> Result<(), EffectError> {
        self.record(format!("would press {key:?}"));
        Ok(())
    }

    fn key_fast(&mut self, key: Key) -> Result<(), EffectError> {
        self.record(format!("would press {key:?} (fast)"));
        Ok(())
    }

    fn enter(&mut self) -> Result<(), EffectError> {
        self.record("would press Enter".to_owned());
        Ok(())
    }

    fn click_here(&mut self) -> Result<(), EffectError> {
        self.record("would click here".to_owned());
        Ok(())
    }

    fn right_click_here(&mut self) -> Result<(), EffectError> {
        self.record("would right-click here".to_owned());
        Ok(())
    }

    fn hold_space(&mut self) -> Result<(), EffectError> {
        self.record("would hold space".to_owned());
        Ok(())
    }

    fn release_space(&mut self) -> Result<(), EffectError> {
        self.record("would release space".to_owned());
        Ok(())
    }

    fn wheel(&mut self, notches: i32, down: bool) -> Result<(), EffectError> {
        // The scroll is part of the route being validated; it cannot buy or sell anything.
        self.inner.wheel(notches, down)?;
        self.record(format!(
            "wheel {notches} {}",
            if down { "down" } else { "up" }
        ));
        Ok(())
    }

    fn move_by(&mut self, dx: i32, dy: i32) -> Result<(), EffectError> {
        self.inner.move_by(dx, dy)?;
        self.record(format!("move by ({dx}, {dy})"));
        Ok(())
    }
}

impl<E> Drop for DryRunEffects<E> {
    fn drop(&mut self) {
        for action in std::mem::take(&mut self.actions) {
            self.log.tool_line(self.tool, &format!("dry run: {action}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Clone, Default)]
    struct Fake {
        calls: Rc<RefCell<Vec<&'static str>>>,
    }

    impl Effects for Fake {
        fn click(&mut self, _at: Point) -> Result<(), EffectError> {
            self.calls.borrow_mut().push("click");
            Ok(())
        }
        fn key(&mut self, _key: Key) -> Result<(), EffectError> {
            self.calls.borrow_mut().push("key");
            Ok(())
        }
        fn enter(&mut self) -> Result<(), EffectError> {
            self.calls.borrow_mut().push("enter");
            Ok(())
        }
        fn place(&mut self, _at: Point) -> Result<(), EffectError> {
            self.calls.borrow_mut().push("place");
            Ok(())
        }
        fn wheel(&mut self, _notches: i32, _down: bool) -> Result<(), EffectError> {
            self.calls.borrow_mut().push("wheel");
            Ok(())
        }
    }

    #[test]
    fn a_dry_run_forwards_placements_and_scrolls_but_suppresses_every_click_and_key() {
        let fake = Fake::default();
        let calls = fake.calls.clone();
        let mut effects = DryRunEffects::new(fake, Arc::new(Logs::disabled()), ToolId::Buy);
        effects.place(Point::new(1, 2)).unwrap();
        effects.wheel(3, true).unwrap();
        effects.click(Point::new(1, 2)).unwrap();
        effects.enter().unwrap();
        effects.key(Key::Char('E')).unwrap();
        assert_eq!(
            &*calls.borrow(),
            &["place", "wheel"],
            "placements/scrolls reach the hardware, clicks/keys do not"
        );
        drop(effects);
    }
}
