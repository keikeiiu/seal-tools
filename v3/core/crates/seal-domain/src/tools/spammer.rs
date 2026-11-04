//! The spammer's decision half: which key fires when, and what the opening looks like.
//!
//! Ported from `v2/SealTools.Spammer/SpammerOrder.cs` (already a pure decision half — the proven
//! pattern this crate generalises) and from the run loop in `SkillSpammer.cs`, which is where the
//! rules the v2 tests could NOT reach live: the opening stagger, the lead-in, preemption across
//! ticks, and a combo owning the tick until it drains.
//!
//! The spammer is a press-only tool. It reads nothing and has no OCR, no window and no guard: its
//! whole failure surface is "which key, and when", which is exactly what is testable here.
//!
//! # The input contract
//!
//! [`SpammerRotation`] is the ACTIVE preset, resolved by the caller from the merged config. Its
//! `keys` is an ORDERED list of `(key, cooldown)` pairs, not a map: the walk's "dictionary order" is
//! the order the config file lists a preset's keys in, and v2's `Dictionary` preserved that order.
//! A `BTreeMap` would sort it and change which filler fires first, so the order is explicit and the
//! config merge must not sort.
//!
//! What the core's reader/config side produces, field by field:
//!
//! - `keys` — `spammer.presets[<active>]` from `local.yaml`, in file order: the key name as the
//!   preset spells it (the leading `*` means a fast tap) and its cooldown in seconds.
//! - `priority` — `spammer.priority[<active>]`, in config order.
//! - `combos` — `spammer.combos[<active>]`, in config order.
//! - `startup` — `spammer.startup[<active>]`; absent means both zeroes.
//!
//! The board side receives [`BoardPress`] values: a key the firmware can press, plus whether it is
//! the fast-tap variant (`k`/`f` commands instead of `K`/`F`).

use crate::config::{SpammerCombo, SpammerStartup};
use crate::decision::{Key, StopReason};
use std::collections::{BTreeMap, VecDeque};

/// The active spammer preset, resolved from the merged config.
///
/// `keys` is ordered on purpose; see the module docs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpammerRotation {
    /// `(key, cooldown seconds)`, in the order the preset lists them.
    pub keys: Vec<(String, f64)>,
    /// Keys that outrank the rest, highest first. An empty list means "no order given".
    pub priority: Vec<String>,
    /// Key groups that must be cast together, in the order they are tried.
    pub combos: Vec<SpammerCombo>,
    /// How the run opens: a lead-in, then a gap between the opening presses.
    pub startup: SpammerStartup,
}

/// The rotation's rules, derived once: what to walk, what the combos are, and whether preemption is
/// on.
///
/// # The shortest usable cooldown (TOOL-F12)
///
/// The caller ticks every 20 ms (`SkillSpammer.Run`'s `SleepCheck(0.02)`), so a cooldown below one
/// tick is "due on every tick" rather than a rate. [`SpammerPlan::cooldown_problems`] refuses those at
/// START; the walk below also rotates where it starts looking, so even a key whose cooldown EQUALS one
/// tick cannot monopolise every tick and starve the rest of the order.
#[derive(Debug, Clone, PartialEq)]
pub struct SpammerPlan {
    cooldowns: BTreeMap<String, f64>,
    /// The combos that can actually run, in their listed order.
    pub combos: Vec<SpammerCombo>,
    /// The keys that may be pressed on their own, in walk order.
    pub singles: Vec<String>,
    /// Whether the order preempts: one single per tick, the rest delayed.
    pub preempt: bool,
    pub startup: SpammerStartup,
}

/// One tick of the run loop, in seconds: `SkillSpammer.Run`'s `SleepCheck(0.02)`. A cooldown shorter
/// than this cannot be honoured, because the loop is never awake more often (TOOL-F12).
pub const MIN_COOLDOWN_SECONDS: f64 = 0.02;

impl SpammerPlan {
    /// Derives the plan from a rotation. See [`runnable_combos`] and [`singles`] for the rules.
    // The rotation is taken by value so a caller can build it inline; this is a public constructor, and
    // borrowing it instead would change every call site for no behaviour change.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "public constructor takes the rotation by value so callers can build it inline; changing it to a reference would only rename callers"
    )]
    #[must_use]
    pub fn new(rotation: SpammerRotation) -> Self {
        let keys: Vec<String> = rotation.keys.iter().map(|(key, _)| key.clone()).collect();
        let cooldowns: BTreeMap<String, f64> = rotation.keys.iter().cloned().collect();
        let combos = runnable_combos(&keys, &rotation.combos);
        let singles = singles(&keys, &rotation.priority, &combos);
        let preempt = !rotation.priority.is_empty();

        Self {
            cooldowns,
            combos,
            singles,
            preempt,
            startup: rotation.startup,
        }
    }

    /// A key's cooldown in seconds, from the active preset.
    #[must_use]
    pub fn cooldown(&self, key: &str) -> Option<f64> {
        self.cooldowns.get(key).copied()
    }

    /// The cooldowns this run cannot use, each as a player line (TOOL-F12). A cooldown that is not a
    /// finite number, or shorter than one tick, makes the key due on EVERY tick — about fifty presses
    /// a second — and under a priority order the top key then starves every other key silently.
    #[must_use]
    pub fn cooldown_problems(&self) -> Vec<String> {
        self.cooldowns
            .iter()
            .filter(|(_, cooldown)| !(cooldown.is_finite() && **cooldown >= MIN_COOLDOWN_SECONDS))
            .map(|(key, cooldown)| {
                format!(
                    "Spammer key '{key}' has a cooldown of {cooldown} s — it must be at least \
                     {MIN_COOLDOWN_SECONDS} s (one tick), or that key fires every tick and the \
                     others never get a turn. Fix the preset on the Spammer tab."
                )
            })
            .collect()
    }
}

/// Resolves the toggle into a plan, or refuses with the message v2's `Run()` shows.
///
/// An empty active preset is the same refusal as a missing one: v2's `ActiveKeys` is an empty map in
/// both cases, and pressing some other rotation silently would be worse than pressing nothing. The
/// two refusals differ only in what the player has to do about it.
///
/// `presets_configured` is "the merged config has any spammer presets at all"; `active` is the name
/// the config points at. Both come from the merged `spammer` block.
///
/// # Errors
///
/// Returns [`StopReason::error`] naming which refusal applies when the rotation is missing or has no
/// keys: "no presets are configured" or "the active preset was not found".
pub fn plan_or_stop(
    presets_configured: bool,
    active: &str,
    rotation: Option<SpammerRotation>,
) -> Result<SpammerPlan, StopReason> {
    match rotation {
        Some(rotation) if !rotation.keys.is_empty() => Ok(SpammerPlan::new(rotation)),
        _ => Err(preset_refusal(presets_configured, active)),
    }
}

/// v2's two refusals, word for word.
fn preset_refusal(presets_configured: bool, active: &str) -> StopReason {
    if presets_configured {
        StopReason::error(format!(
            "The active preset '{active}' was not found — pick one in the Spammer tab."
        ))
    } else {
        StopReason::error("No spammer presets are configured — add one in the Spammer tab.")
    }
}

/// The keys to walk, highest precedence first: those the preset NAMES, in the order it gives them,
/// followed by every other key in config order.
///
/// A name that matches no key in the preset is SKIPPED — a typo in the order must not stop the
/// rotation, and must not add a phantom key either. A name repeated is taken once, at its first
/// position.
///
/// An empty `priority` returns the keys in config order, which is exactly what the loop did before
/// any of this existed. THE CALLER decides what that means, because an empty list cannot distinguish
/// "no rule was asked for" from "no key outranks another" — that is why [`SpammerPlan::preempt`] is
/// `priority`'s presence and not its contents.
#[must_use]
pub fn order_for(keys: &[String], priority: &[String]) -> Vec<String> {
    let mut order: Vec<String> = Vec::with_capacity(keys.len());
    for named in priority {
        if keys.contains(named) && !order.contains(named) {
            order.push(named.clone());
        }
    }
    for key in keys {
        if !order.contains(key) {
            order.push(key.clone());
        }
    }
    order
}

/// Each name respelled to the form the preset actually stores, when the two differ only by the
/// leading `*`; anything else comes back untouched.
///
/// This exists because the rows editor writes every key as `*name` — the fast tap — so saving a
/// preset that a hand-edited config spelled bare RENAMES its keys. `priority` and `combos` reference
/// keys BY NAME, so without this the editor prunes those references against names that had merely
/// changed shape: the ordering and the combos vanish on the first save, silently.
///
/// One way only, and only where it is unambiguous. A name matching NEITHER spelling is left alone,
/// so a genuine typo is still pruned by the normal rule rather than being rewritten into some key
/// that happens to exist.
#[must_use]
pub fn respell(names: &[String], preset_keys: &[String]) -> Vec<String> {
    names
        .iter()
        .map(|name| {
            if preset_keys.contains(name) {
                return name.clone();
            }
            let other = match name.strip_prefix('*') {
                Some(bare) => bare.to_owned(),
                None => format!("*{name}"),
            };
            if preset_keys.contains(&other) {
                other
            } else {
                name.clone()
            }
        })
        .collect()
}

/// The keys the loop may press ON THEIR OWN: the whole preset minus everything a combo owns, in
/// [`order_for`]'s precedence order.
///
/// The subtraction is what makes a combo a UNIT. Left in the walk, the second key of a pair would
/// fire on its own whenever the first was on cooldown — the effect never triggers, and nothing says
/// so.
#[must_use]
pub fn singles(keys: &[String], priority: &[String], combos: &[SpammerCombo]) -> Vec<String> {
    let mut owned: Vec<&str> = Vec::new();
    for combo in combos {
        for key in &combo.keys {
            if !owned.iter().any(|taken| *taken == key) {
                owned.push(key);
            }
        }
    }
    order_for(keys, priority)
        .into_iter()
        .filter(|key| !owned.iter().any(|taken| *taken == key))
        .collect()
}

/// The combos that can actually run, in the order given: at least two keys, and every one of them a
/// key this preset has.
///
/// Everything else is DROPPED rather than run, and that includes a combo naming a key the preset does
/// not have — a typo. Half a combo is worse than no combo: it fires the first skill, spends its
/// cooldown, and leaves the effect un-triggered with nothing to show for it.
#[must_use]
pub fn runnable_combos(keys: &[String], combos: &[SpammerCombo]) -> Vec<SpammerCombo> {
    combos
        .iter()
        .filter(|combo| combo.keys.len() >= 2 && combo.keys.iter().all(|key| keys.contains(key)))
        .cloned()
        .collect()
}

/// The state of one running spammer session: the cooldown clock, the combo being cast, and the
/// opening.
///
/// Pure in the sense that matters: nothing in here reads a clock or a port. The caller passes the
/// run's elapsed seconds to [`SpammerRun::tick`] and executes the presses it returns.
#[derive(Debug, Clone, PartialEq)]
pub struct SpammerRun {
    plan: SpammerPlan,
    /// When each key's cooldown was last (re)started. A fresh run seeds these NEGATIVELY so every key
    /// starts due — see [`SpammerRun::start`].
    last_press: BTreeMap<String, f64>,
    /// The combo being cast, one step per gap. Non-empty means the run is MID-COMBO and nothing else
    /// may be pressed until it drains.
    pending: VecDeque<String>,
    next_step_at: f64,
    running_gap: f64,
    opening_at: f64,
    lead_in: f64,
    /// Where the single-key walk starts looking next (TOOL-F12). Advanced past whichever key was
    /// pressed last, so a key that is due on every tick cannot take every tick.
    cursor: usize,
}

impl SpammerRun {
    /// Opens a fresh run at `now`, paying the preset's lead-in.
    ///
    /// EVERY KEY STARTS DUE, not on cooldown: Start is a fresh run and the first thing a fresh run
    /// does is cast. Waiting each key's cooldown out first is right for a 6 s rotation and wrong for a
    /// long buff, which on a short session would simply never go up at all.
    ///
    /// `startup.step` then SPACES those opening presses, in the same order the walk uses: due all at
    /// once, a rotation with animations fires its whole opening into itself and the game swallows all
    /// but the first. Staggered by UNIT and not by key: a combo is one move, so its keys share an
    /// offset and the combo's own gap does the spacing INSIDE it. Each key's `last_press` is seeded at
    /// `at - cooldown`, which puts its due moment exactly at `at`.
    #[must_use]
    pub fn start(plan: &SpammerPlan, now: f64) -> Self {
        Self::open(plan, now, plan.startup.lead_in)
    }

    /// Re-opens after yielding the game to another tool at `now`.
    ///
    /// The LEAD-IN is deliberately not paid again — it exists to give the player time to focus the
    /// game after Start, and the game is already in front by the time a reload finishes. The opening
    /// stagger IS re-applied, so a 30 s pause does not end with every expired cooldown firing at once.
    #[must_use]
    pub fn resume(plan: &SpammerPlan, now: f64) -> Self {
        Self::open(plan, now, 0.0)
    }

    fn open(plan: &SpammerPlan, now: f64, lead_in: f64) -> Self {
        let opening_at = now + lead_in;
        let mut last_press = BTreeMap::new();
        let mut opening_index: u32 = 0;

        for combo in &plan.combos {
            let at = opening_at + f64::from(opening_index) * plan.startup.step;
            for key in &combo.keys {
                if let Some(cooldown) = plan.cooldown(key) {
                    last_press.insert(key.clone(), at - cooldown);
                }
            }
            opening_index += 1;
        }
        for key in &plan.singles {
            let at = opening_at + f64::from(opening_index) * plan.startup.step;
            if let Some(cooldown) = plan.cooldown(key) {
                last_press.insert(key.clone(), at - cooldown);
            }
            opening_index += 1;
        }

        Self {
            plan: plan.clone(),
            last_press,
            pending: VecDeque::new(),
            next_step_at: 0.0,
            running_gap: 0.0,
            opening_at,
            lead_in,
            cursor: 0,
        }
    }

    /// Seconds left of the lead-in, for the card's countdown line. `None` once the opening has
    /// started, so a card never counts to a moment already gone.
    #[must_use]
    pub fn lead_in_remaining(&self, now: f64) -> Option<f64> {
        if self.lead_in > 0.0 && now < self.opening_at {
            Some(self.opening_at - now)
        } else {
            None
        }
    }

    /// The run's opening moment on the caller's clock.
    #[must_use]
    pub fn opening_at(&self) -> f64 {
        self.opening_at
    }

    /// What to press at `now`: the ordered plan for this tick.
    ///
    /// A combo is started when every one of its keys is off cooldown, and it then OWNS the tick until
    /// its last step has gone out — nothing else is pressed, "no filler, no other combo", because the
    /// previous cast's animation would swallow anything sent inside it. Each key's cooldown restarts
    /// when THAT key was cast, not when the combo began.
    ///
    /// With no combo running the single-key walk runs: every due key, in walk order — or, when the
    /// preset gives an order, exactly ONE key and then stop. The rest are delayed, not skipped: their
    /// cooldowns are untouched, so they go out on a later tick.
    ///
    /// Cooldowns are advanced for every key in the returned plan, before the caller sends anything —
    /// v2 advances them "either way, so an unusable key isn't retried every tick". The caller counts a
    /// press as a press only when the board accepted it.
    pub fn tick(&mut self, now: f64) -> SpammerTick {
        let mut presses = Vec::new();

        // Start a combo when none is running. Readiness is "EVERY key in it is off cooldown", which is
        // what keeps the pair whole.
        if self.pending.is_empty() {
            let next = self
                .plan
                .combos
                .iter()
                .find(|combo| combo.keys.iter().all(|key| self.due(key, now)))
                .map(|combo| (combo.keys.clone(), combo.gap));
            if let Some((keys, gap)) = next {
                self.running_gap = gap;
                self.pending = keys.into();
                self.next_step_at = now; // the first step goes out now; each later one after a gap
            }
        }

        if self.pending.is_empty() {
            // TOOL-F12: the walk starts where the LAST press left it and wraps. Taking the first due
            // key every tick is what let a priority key whose cooldown is one tick long press on
            // every tick forever, so nothing below it in the order ever fired — silent starvation.
            let count = self.plan.singles.len();
            let start = self.cursor;
            let mut pressed_at: Option<usize> = None;
            for step in 0..count {
                let index = (start + step) % count;
                let key = self.plan.singles[index].clone();
                if !self.due(&key, now) {
                    continue;
                }
                self.last_press.insert(key.clone(), now);
                presses.push(SpammerPress::new(key));
                pressed_at = Some(index);
                if self.plan.preempt {
                    break;
                }
            }
            if let Some(index) = pressed_at {
                self.cursor = (index + 1) % count;
            }
        } else {
            // A running combo OWNS the tick.
            if now >= self.next_step_at
                && let Some(key) = self.pending.pop_front()
            {
                self.last_press.insert(key.clone(), now);
                presses.push(SpammerPress::new(key));
                self.next_step_at = now + self.running_gap;
            }
        }

        SpammerTick { presses }
    }

    fn due(&self, key: &str, now: f64) -> bool {
        let cooldown = self.plan.cooldown(key).unwrap_or(0.0);
        let last = self
            .last_press
            .get(key)
            .copied()
            .unwrap_or(f64::NEG_INFINITY);
        now - last >= cooldown
    }
}

/// The presses one tick's decision calls for, in order. Empty means "nothing this tick".
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpammerTick {
    pub presses: Vec<SpammerPress>,
}

impl SpammerTick {
    /// The configured key names, for assertions and log lines.
    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        self.presses.iter().map(|press| press.key.clone()).collect()
    }

    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.presses.is_empty()
    }
}

/// One configured key, resolved to what the board would receive.
#[derive(Debug, Clone, PartialEq)]
pub struct SpammerPress {
    /// The key as the preset spells it, `*` included — what the log shows.
    pub key: String,
    /// A fast tap: the board's lowercase `k`/`f` command instead of `K`/`F`.
    pub fast: bool,
    /// The board press, or `None` when the firmware cannot send this key. The caller warns once and
    /// does not count it as a press (v2's `SendKey` returns false for these).
    pub board_key: Option<Key>,
}

impl SpammerPress {
    fn new(key: String) -> Self {
        let fast = key.starts_with('*');
        let board_key = Key::parse(key.strip_prefix('*').unwrap_or(key.as_str())).ok();
        Self {
            key,
            fast,
            board_key,
        }
    }

    /// The press the board should send, or `None` when it cannot.
    #[must_use]
    pub fn board_press(&self) -> Option<BoardPress> {
        self.board_key.map(|key| BoardPress {
            key,
            fast: self.fast,
        })
    }

    /// The warning v2 logs and shows once per bad key, or `None` when the key is sendable.
    #[must_use]
    pub fn unsupported_message(&self) -> Option<String> {
        match self.board_key {
            Some(_) => None,
            None => unsupported_key_message(&self.key),
        }
    }
}

/// What the board is asked to press: one key, and whether it is the fast-tap variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardPress {
    pub key: Key,
    pub fast: bool,
}

/// The warning for a key the firmware cannot send, word for word from `SkillSpammer.WarnUnsupported`:
/// "Spammer key 'X' unsupported — use one letter or digit, or F1–F12."
///
/// The key is shown WITHOUT the fast-tap `*`, exactly as v2 strips it before warning. `None` when the
/// key is sendable.
#[must_use]
pub fn unsupported_key_message(key: &str) -> Option<String> {
    let stripped = key.strip_prefix('*').unwrap_or(key);
    if Key::parse(stripped).is_ok() {
        return None;
    }
    Some(format!(
        "Spammer key '{stripped}' unsupported — use one letter or digit, or F1–F12."
    ))
}
