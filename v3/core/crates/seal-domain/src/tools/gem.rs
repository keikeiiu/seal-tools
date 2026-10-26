//! The gem composer's decision half: the route table, the two move sets, the empty-result-box
//! discriminator, and the start / combine / settle / advance turn sequence.
//!
//! Ported from `v2/SealTools.Core/GemRoutes.cs`, `GemColorAnalyzer.cs` (its pure arithmetic) and
//! `SealTools.GemComposer/GemComposer.cs`. The composer reads one thing — the result box — and the
//! rest is a fixed cycle of cursor moves and clicks, which is exactly what is testable here.
//!
//! # The two move sets, chosen by `gem.move_mode`
//!
//! - `"tuned"` — the raw dx/dy counts in `gem.movements`: one hand-tuned `D dx dy` per route, per-PC.
//! - `"arduino"` — place the cursor on the route's destination POINT closed-loop, the same mechanism
//!   as the calibrator's Test Click. Self-correcting, and it reuses the points already calibrated.
//!
//! # What the domain does NOT own
//!
//! Capturing the result box, comparing it with the saved empty crop (`DiffFraction`, which needs
//! pixels) and the foreground check are the reader side's; what crosses the boundary is
//! [`EmptyRead`] — the fraction when the crop was compared, the colour composition when only the
//! signature can be used, or "no reference". The discriminator itself is pure and ported here.
//!
//! One naming quirk is kept from v2 because both halves are user-visible: the arduino route keys are
//! `slot3_n` / `slot3_g` / `slot3_dg` while `gem.movements` stores the tuned counts as `slot3_to_n`
//! etc.

use crate::config::{ColorComposition, MovementsConfig};
use crate::decision::StopReason;
use crate::geometry::Point;
use crate::tools::grade_cycles::GradeCycles;
use std::collections::BTreeMap;

/// One composer route: `key` matches the arduino set's route id and the calibrator's row key;
/// `to` is a calibrated point name (see [`GemRoutes::resolve`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Route {
    pub key: &'static str,
    pub from: &'static str,
    pub to: &'static str,
    pub label: &'static str,
}

/// The composer's routes, and where each one ends. The shared definition of the arduino move set.
pub struct GemRoutes;

impl GemRoutes {
    pub const ALL: [Route; 11] = [
        Route {
            key: "radio_N",
            from: "N",
            to: "Register",
            label: "N → Register",
        },
        Route {
            key: "radio_G",
            from: "G",
            to: "Register",
            label: "G → Register",
        },
        Route {
            key: "radio_DG",
            from: "DG",
            to: "Register",
            label: "DG → Register",
        },
        Route {
            key: "register_combine",
            from: "Register",
            to: "Combine",
            label: "Register → Combine",
        },
        Route {
            key: "combine_register",
            from: "Combine",
            to: "Register",
            label: "Combine → Register",
        },
        Route {
            key: "register_slot1",
            from: "Register",
            to: "Resource1",
            label: "Register → Resource1",
        },
        Route {
            key: "slot1_slot2",
            from: "Resource1",
            to: "Resource2",
            label: "Resource1 → Resource2",
        },
        Route {
            key: "slot2_slot3",
            from: "Resource2",
            to: "Resource3",
            label: "Resource2 → Resource3",
        },
        Route {
            key: "slot3_n",
            from: "Resource3",
            to: "N",
            label: "Resource3 → N",
        },
        Route {
            key: "slot3_g",
            from: "Resource3",
            to: "G",
            label: "Resource3 → G",
        },
        Route {
            key: "slot3_dg",
            from: "Resource3",
            to: "DG",
            label: "Resource3 → DG",
        },
    ];

    /// The calibrated point a route ends on (the arduino set's whole definition), or `None` when the
    /// key isn't a known route.
    pub fn destination(key: &str) -> Option<&'static str> {
        Self::ALL
            .iter()
            .find(|route| route.key == key)
            .map(|route| route.to)
    }

    /// Resolves a calibrated point name to its client-relative PHYSICAL offset: the
    /// grade/Register/Combine entries in `gem.grade_positions`, and `Resource1..3` in
    /// `gem.resource_gems` (1-based in the name, 0-based in the list). `None` when that point hasn't
    /// been calibrated.
    pub fn resolve(
        grade_positions: &BTreeMap<String, Vec<i32>>,
        resource_gems: &[Vec<i32>],
        name: &str,
    ) -> Option<Point> {
        if let Some(rest) = name.strip_prefix("Resource") {
            let index: i32 = rest.parse().ok()?;
            if index < 1 || resource_gems.len() < index as usize {
                return None;
            }
            let resource = &resource_gems[index as usize - 1];
            return (resource.len() >= 2).then(|| Point {
                x: resource[0],
                y: resource[1],
            });
        }

        grade_positions.get(name).and_then(|position| {
            (position.len() >= 2).then(|| Point {
                x: position[0],
                y: position[1],
            })
        })
    }
}

/// Which move set the composer uses between calibrated points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveMode {
    /// `"arduino"`: place the cursor on the destination point, closed-loop.
    Arduino,
    /// `"tuned"`: send the hand-tuned `gem.movements` counts.
    Tuned,
}

impl MoveMode {
    /// v2's comparison is `== "arduino"`, so anything else is the tuned set.
    pub fn parse(text: &str) -> Self {
        if text == "arduino" {
            Self::Arduino
        } else {
            Self::Tuned
        }
    }
}

/// What the composer does when the result box comes back empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmptyMode {
    Stop,
    AdvanceGrade,
    AdvanceGradeClear,
}

impl EmptyMode {
    /// v2 compares `== "stop"` and `== "advance_grade_clear"`, so an unknown value behaves as plain
    /// `advance_grade`.
    pub fn parse(text: &str) -> Self {
        match text {
            "stop" => Self::Stop,
            "advance_grade_clear" => Self::AdvanceGradeClear,
            _ => Self::AdvanceGrade,
        }
    }
}

/// What the reader saw in the result box after a combine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmptyRead {
    /// The saved empty crop exists and the live crop differs from it on this fraction of pixels.
    /// The PRIMARY signal — colour- AND shape-blind, so any gem reads the same.
    DiffFraction(f64),
    /// The crop is missing; the box's colour composition was analysed instead. The weaker test: a
    /// gem colour close to the empty slot's can hide in it.
    Composition(ColorComposition),
    /// Neither reference is available. The composer must answer "not empty", so it never advances a
    /// grade on a missing reference.
    NoReference,
}

/// v2 `GemColorAnalyzer.Analyze`: the six-feature colour fingerprint of a box. `colored_gap_min` is
/// the channel max-min gap above which a pixel counts as distinctly coloured.
///
/// Measurement arithmetic, not a decision: the gem run uses it when only the colour signature can be
/// consulted, and `calibration.save` samples the same six features from the calibrated result box.
/// Ported verbatim from v2's analyzer; keep the attribution if it ever moves again.
pub fn analyze_composition(
    rgba: &[u8],
    width: usize,
    height: usize,
    colored_gap_min: i32,
) -> ColorComposition {
    let pixels = width.saturating_mul(height);
    if pixels == 0 || rgba.len() < pixels * 4 {
        return ColorComposition::default();
    }
    let mut sum_v = 0.0;
    let mut sum_sat = 0.0;
    let mut sum_r = 0.0;
    let mut sum_g = 0.0;
    let mut sum_b = 0.0;
    let mut colored = 0usize;
    for pixel in rgba[..pixels * 4].as_chunks::<4>().0 {
        let (r, g, b) = (
            i32::from(pixel[0]),
            i32::from(pixel[1]),
            i32::from(pixel[2]),
        );
        let value = r.max(g).max(b);
        let minimum = r.min(g).min(b);
        let saturation = if value == 0 {
            0
        } else {
            (value - minimum) * 255 / value
        };
        sum_v += f64::from(value) / 255.0;
        sum_sat += f64::from(saturation) / 255.0;
        sum_r += f64::from(r) / 255.0;
        sum_g += f64::from(g) / 255.0;
        sum_b += f64::from(b) / 255.0;
        if value - minimum >= colored_gap_min {
            colored += 1;
        }
    }
    let count = pixels as f64;
    ColorComposition {
        mean_v: sum_v / count,
        mean_sat: sum_sat / count,
        colored_fraction: colored as f64 / count,
        mean_r: sum_r / count,
        mean_g: sum_g / count,
        mean_b: sum_b / count,
    }
}
/// Distance between two colour compositions: the EUCLIDEAN norm over the six features, not their
/// mean.
///
/// The mean was measured to be useless here (2026-09-10): a genuinely empty box and the same box
/// holding a gem differ by only 0.10 as a mean of six absolute differences, so a box full of gem read
/// as "empty" and the composer advanced the grade. The same pair is 0.29 as a norm, which the
/// calibrated threshold (0.18) separates cleanly.
pub fn composition_distance(a: &ColorComposition, b: &ColorComposition) -> f64 {
    let dv = a.mean_v - b.mean_v;
    let ds = a.mean_sat - b.mean_sat;
    let df = a.colored_fraction - b.colored_fraction;
    let dr = a.mean_r - b.mean_r;
    let dg = a.mean_g - b.mean_g;
    let db = a.mean_b - b.mean_b;
    (dv * dv + ds * ds + df * df + dr * dr + dg * dg + db * db).sqrt()
}

/// True when the frame is close enough to the sampled EMPTY reference. A `None` reference means
/// empty-detection hasn't been calibrated yet, so we conservatively say NOT empty.
pub fn composition_is_empty(
    frame: &ColorComposition,
    empty_reference: Option<&ColorComposition>,
    empty_distance: f64,
) -> bool {
    match empty_reference {
        Some(reference) => composition_distance(frame, reference) <= empty_distance,
        None => false,
    }
}

/// `IsResultBoxEmpty`, from the reader's evidence: the crop comparison when there is one, the colour
/// signature only as the fallback, and "not empty" when neither exists.
pub fn result_box_is_empty(
    read: &EmptyRead,
    empty_signature: Option<&ColorComposition>,
    empty_distance: f64,
) -> bool {
    match read {
        EmptyRead::DiffFraction(fraction) => *fraction <= empty_distance,
        EmptyRead::Composition(frame) => {
            composition_is_empty(frame, empty_signature, empty_distance)
        }
        EmptyRead::NoReference => false,
    }
}

/// The gem block the decision half needs, resolved from the merged config.
///
/// Field sources: `gem` in the merged config — `grades`, `start_grade`, `move_mode`, `empty_mode`,
/// `empty_distance`, `empty_streak` (portable half in `local.yaml`'s `settings:`); `grade_positions`,
/// `resource_gems`, `movements`, `empty_signature` (machine half in `local.yaml`).
#[derive(Debug, Clone, PartialEq)]
pub struct GemSettings {
    pub grades: Vec<String>,
    pub start_grade: String,
    pub grade_positions: BTreeMap<String, Vec<i32>>,
    pub movements: MovementsConfig,
    pub move_mode: MoveMode,
    pub resource_gems: Vec<Vec<i32>>,
    pub empty_mode: EmptyMode,
    pub empty_distance: f64,
    pub empty_signature: Option<ColorComposition>,
    /// Consecutive empty reads required before the `empty_mode` action fires.
    pub empty_streak: i32,
}

/// One effect in a turn, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum GemStep {
    /// Move the cursor to this client-relative physical offset, closed-loop.
    Place(Point),
    /// Send the tuned HID counts (`D dx dy`).
    Move {
        dx: i32,
        dy: i32,
    },
    ClickLeft,
    ClickRight,
    Wait(f64),
}

/// One turn's ordered effects, and whether the run ends after them.
#[derive(Debug, Clone, PartialEq)]
pub struct GemTurn {
    pub steps: Vec<GemStep>,
    /// `Some` when the run ends after these steps (v2's last-grade completion, whose message the
    /// player sees).
    pub stop: Option<StopReason>,
}

impl GemTurn {
    fn new(steps: Vec<GemStep>) -> Self {
        Self { steps, stop: None }
    }
}

/// The state of one compose run: the current grade, the cycle count, the empty streak, and the
/// per-grade tally.
#[derive(Debug, Clone, PartialEq)]
pub struct GemRun {
    settings: GemSettings,
    grade_index: usize,
    cycle: u32,
    empty_streak: i32,
    tally: GradeCycles,
}

impl GemRun {
    /// Resolves the start grade and settles the state. The one refusal here is an empty grade list:
    /// v2 indexed `grades[gidx]` before its window even opened, so the domain refuses up front
    /// instead of reproducing an out-of-bounds index.
    pub fn new(settings: GemSettings) -> Result<Self, StopReason> {
        if settings.grades.is_empty() {
            return Err(StopReason::error(
                "No gem grades are configured — the composer has nothing to work through.",
            ));
        }
        let grade_index = settings
            .grades
            .iter()
            .position(|grade| grade == &settings.start_grade)
            .unwrap_or(0);

        Ok(Self {
            settings,
            grade_index,
            cycle: 0,
            empty_streak: 0,
            tally: GradeCycles::new(),
        })
    }

    /// The grade the composer is working on.
    pub fn grade(&self) -> &str {
        &self.settings.grades[self.grade_index]
    }

    /// Cycles this run has started.
    pub fn cycle(&self) -> u32 {
        self.cycle
    }

    /// The per-grade tally, for the card and the log.
    pub fn tally(&self) -> &GradeCycles {
        &self.tally
    }

    /// Whether the loop should read the result box at all.
    ///
    /// v2's gate is `empty_mode != "stop" && empty_signature != null` — so an empty signature turns
    /// auto-advance off EVEN when the saved crop (the primary signal) exists. That looks like a bug
    /// and is ported as-is: it is the condition that shipped, and a composer that suddenly starts
    /// advancing on machines that never sampled a signature would be the stricter-behaviour change
    /// this port avoids.
    pub fn empty_check_enabled(&self) -> bool {
        self.settings.empty_mode != EmptyMode::Stop && self.settings.empty_signature.is_some()
    }

    /// Selects the grade and moves to Register (v2's `SelectGradeAndRegister`): the single click on
    /// the grade button both activates the game window and presses it, so there is no separate
    /// focus click.
    pub fn select_grade_turn(&self) -> Result<GemTurn, StopReason> {
        let grade = self.grade().to_owned();
        let mut steps = vec![
            GemStep::Place(self.grade_position(&grade)?),
            GemStep::Wait(0.3),
            GemStep::ClickLeft,
            GemStep::Wait(0.5),
        ];
        let what = format!("Move {grade} → Register");
        let tuned = self.radio_tuned(&grade);
        steps.push(self.route_step(&what, &format!("radio_{grade}"), tuned)?);
        steps.push(GemStep::Wait(0.3));
        steps.push(GemStep::ClickLeft);
        steps.push(GemStep::Wait(0.5));
        Ok(GemTurn::new(steps))
    }

    /// One combine cycle's first half: count the cycle and its grade, then move Register → Combine
    /// and click.
    ///
    /// The count happens BEFORE the combine, so the tally says where the work went even if the run
    /// dies mid-cycle.
    pub fn combine_turn(&mut self) -> Result<GemTurn, StopReason> {
        self.cycle += 1;
        let grade = self.grade().to_owned();
        self.tally.add(&grade);

        let tuned = Some(self.settings.movements.register_combine.as_slice());
        let mut steps =
            vec![self.route_step("Move Register → Combine", "register_combine", tuned)?];
        steps.push(GemStep::Wait(0.2));
        steps.push(GemStep::ClickLeft);
        steps.push(GemStep::Wait(0.8));
        Ok(GemTurn::new(steps))
    }

    /// The second half of a cycle, after the result box was read: move Combine → Register, then
    /// either the normal deregister/register click pair or the grade-advance flow.
    ///
    /// `empty` is the reader's verdict, or `None` when no read was performed (auto-check disabled).
    pub fn finish_turn(&mut self, empty: Option<bool>) -> Result<GemTurn, StopReason> {
        let tuned = Some(self.settings.movements.combine_register.as_slice());
        let mut steps =
            vec![self.route_step("Move Combine → Register", "combine_register", tuned)?];
        steps.push(GemStep::Wait(0.2));

        let advance = match empty {
            Some(is_empty) if self.empty_check_enabled() => {
                if is_empty {
                    self.empty_streak += 1;
                    if self.empty_streak >= self.settings.empty_streak {
                        self.empty_streak = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    self.empty_streak = 0;
                    false
                }
            }
            _ => false,
        };

        if advance {
            let next = self.advance_turn()?;
            steps.extend(next.steps);
            return Ok(GemTurn {
                steps,
                stop: next.stop,
            });
        }

        // Normal: deregister + register.
        steps.push(GemStep::ClickLeft);
        steps.push(GemStep::Wait(0.3));
        steps.push(GemStep::ClickLeft);
        steps.push(GemStep::Wait(0.5));
        Ok(GemTurn::new(steps))
    }

    /// F9: advance to the next grade BY HAND, wrapping back to the first at the end. v2 logs this as
    /// "left by hand" rather than "finished" — the grade did not run out, the player moved on — and
    /// clears no resources here, because the player chose the moment.
    pub fn advance_grade_by_hand(&mut self) -> Result<GemTurn, StopReason> {
        self.grade_index = (self.grade_index + 1) % self.settings.grades.len();
        self.select_grade_turn()
    }

    /// The advance flow: clear the resource slots in `advance_grade_clear` mode, stop if this was the
    /// last grade, else select the next grade and move it to Register.
    fn advance_turn(&mut self) -> Result<GemTurn, StopReason> {
        let mut steps = Vec::new();

        if self.settings.empty_mode == EmptyMode::AdvanceGradeClear {
            steps.extend(self.clear_resources_steps()?);
        }

        if self.grade_index + 1 >= self.settings.grades.len() {
            let grade = self.grade().to_owned();
            return Ok(GemTurn {
                steps,
                stop: Some(StopReason::done(format!(
                    "All grades done (last was {grade}) — composer stopped."
                ))),
            });
        }

        self.grade_index += 1;
        let next = self.grade().to_owned();

        if self.settings.empty_mode == EmptyMode::AdvanceGradeClear {
            // The cursor is at Resource3: move to the next grade and select it.
            let (route_key, tuned) = match next.as_str() {
                "N" => ("slot3_n", self.settings.movements.slot3_to_n.as_slice()),
                "G" => ("slot3_g", self.settings.movements.slot3_to_g.as_slice()),
                "DG" => ("slot3_dg", self.settings.movements.slot3_to_dg.as_slice()),
                other => {
                    return Err(StopReason::error(format!("No route Resource3 → {other}.")));
                }
            };
            steps.push(self.route_step(
                &format!("Move Resource3 → {next}"),
                route_key,
                Some(tuned),
            )?);
            steps.push(GemStep::Wait(0.2));
            steps.push(GemStep::ClickLeft);
            steps.push(GemStep::Wait(0.5));
        } else {
            // The cursor is at Register: re-select the grade ABSOLUTELY.
            steps.push(GemStep::Place(self.grade_position(&next)?));
            steps.push(GemStep::Wait(0.3));
            steps.push(GemStep::ClickLeft);
            steps.push(GemStep::Wait(0.5));
        }

        // Next grade → Register, and select.
        let what = format!("Move {next} → Register");
        let tuned = self.radio_tuned(&next);
        steps.push(self.route_step(&what, &format!("radio_{next}"), tuned)?);
        steps.push(GemStep::Wait(0.3));
        steps.push(GemStep::ClickLeft);
        steps.push(GemStep::Wait(0.5));

        Ok(GemTurn::new(steps))
    }

    /// Right-click the three resource slots to clear any stuck resource gems.
    fn clear_resources_steps(&self) -> Result<Vec<GemStep>, StopReason> {
        let movements = &self.settings.movements;
        let routes = [
            (
                "Move Register → Resource1",
                "register_slot1",
                movements.register_slot1.as_slice(),
            ),
            (
                "Move Resource1 → Resource2",
                "slot1_slot2",
                movements.slot1_slot2.as_slice(),
            ),
            (
                "Move Resource2 → Resource3",
                "slot2_slot3",
                movements.slot2_slot3.as_slice(),
            ),
        ];

        let mut steps = Vec::new();
        for (what, route_key, tuned) in routes {
            steps.push(self.route_step(what, route_key, Some(tuned))?);
            steps.push(GemStep::Wait(0.2));
            steps.push(GemStep::ClickRight);
            steps.push(GemStep::Wait(0.3));
        }
        Ok(steps)
    }

    /// One route move, honouring `gem.move_mode`. Both sets are one step, so a route reads the same
    /// either way.
    ///
    /// Public so a caller (and the tests) can ask for a single route without building a whole turn.
    pub fn route_step(
        &self,
        what: &str,
        route_key: &str,
        tuned: Option<&[i32]>,
    ) -> Result<GemStep, StopReason> {
        match self.settings.move_mode {
            MoveMode::Arduino => {
                let Some(destination) = GemRoutes::destination(route_key) else {
                    return Err(StopReason::error(format!(
                        "{what}: unknown route '{route_key}'."
                    )));
                };
                let point = GemRoutes::resolve(
                    &self.settings.grade_positions,
                    &self.settings.resource_gems,
                    destination,
                )
                .ok_or_else(|| {
                    StopReason::error(format!(
                        "{what}: '{destination}' isn't calibrated yet — open Calibrate Gem and save \
                         it."
                    ))
                })?;
                Ok(GemStep::Place(point))
            }
            MoveMode::Tuned => match tuned {
                Some(tuned) if tuned.len() >= 2 => Ok(GemStep::Move {
                    dx: tuned[0],
                    dy: tuned[1],
                }),
                _ => Err(StopReason::error(format!(
                    "{what} isn't saved yet — open Calibrate Gem and press Save Composer Moves."
                ))),
            },
        }
    }

    /// The tuned counts for a `radio_<grade>` route, or `None` when they were never saved.
    fn radio_tuned(&self, grade: &str) -> Option<&[i32]> {
        self.settings
            .movements
            .radio_to_register
            .get(grade)
            .map(Vec::as_slice)
    }

    /// A grade button's calibrated position (`gem.grade_positions`).
    fn grade_position(&self, grade: &str) -> Result<Point, StopReason> {
        match self.settings.grade_positions.get(grade) {
            Some(position) if position.len() >= 2 => Ok(Point {
                x: position[0],
                y: position[1],
            }),
            _ => Err(StopReason::error(format!(
                "Grade position '{grade}' isn't calibrated yet — open Calibrate Gem and save it."
            ))),
        }
    }
}

// ── failures the LOOP reports (the decision half cannot see a window or a board) ───────────

/// The game window was not found (or is minimized) when a placement was attempted.
pub fn window_missing() -> StopReason {
    StopReason::error("Game window not found (or minimized) — open and restore the game first.")
}

/// A cursor that can't be placed must NOT be clicked through: the click would land wherever the
/// pointer happens to be. This is the grade-button version of that refusal.
pub fn grade_button_placement_failed(grade: &str, detail: &str) -> StopReason {
    StopReason::error(format!(
        "Couldn't move the cursor onto the {grade} button — {detail}. Stopped instead of clicking \
         blind."
    ))
}

/// The closed-loop placement for a route's destination failed.
pub fn route_placement_failed(what: &str, point_name: &str, detail: &str) -> StopReason {
    StopReason::error(format!(
        "{what}: couldn't move the cursor to {point_name} — {detail}."
    ))
}
