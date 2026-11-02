//! How many compose cycles a run has spent on each grade — shared by the tuner and the gem composer,
//! both of which run a grade ladder and both of which report where the cycles went.
//!
//! Ported from `v2/SealTools.Core/GradeCycles.cs`; its `GradeCyclesTests.cs` is the translated oracle
//! in `tests/tuner.rs`.

use std::collections::BTreeMap;

/// One grade's line on the card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradeCycleRow {
    pub name: String,
    pub status: String,
}

/// How many compose cycles a run has spent on each grade.
///
/// The composer's single Cycle counter says how much work a run has done and not where it went, and
/// the interesting number is per grade: a run that took 40 cycles on N and 3 on DG is telling you
/// something a total of 43 cannot. The player asked for it in as many words — *"N how many cycle, G
/// how many cycle, DG how many cycle"*.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GradeCycles {
    counts: BTreeMap<String, i32>,
}

impl GradeCycles {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// One more cycle spent on `grade`.
    pub fn add(&mut self, grade: &str) {
        *self.counts.entry(grade.to_owned()).or_default() += 1;
    }

    /// Cycles spent on `grade` so far — 0 for one not reached yet.
    #[must_use]
    pub fn count_for(&self, grade: &str) -> i32 {
        self.counts.get(grade).copied().unwrap_or(0)
    }

    /// A new run: every count back to zero.
    pub fn reset(&mut self) {
        self.counts.clear();
    }

    /// One row per grade, in the order the composer runs them. Every grade appears including the ones
    /// not reached: "DG: 0 cycles" is information, and a list that grows as the run proceeds moves
    /// under the reader's eye.
    #[must_use]
    pub fn rows(&self, grades: &[String]) -> Vec<GradeCycleRow> {
        grades
            .iter()
            .map(|grade| GradeCycleRow {
                name: grade.clone(),
                status: describe(self.count_for(grade)),
            })
            .collect()
    }

    /// The whole tally on one line, for the log — `N 12 · G 8 · DG 3 · 23 total`. The total is the sum
    /// of the grades rather than a separate counter, so it cannot disagree with them.
    #[must_use]
    pub fn summary(&self, grades: &[String]) -> String {
        let mut parts: Vec<String> = grades
            .iter()
            .map(|grade| format!("{grade} {}", self.count_for(grade)))
            .collect();
        parts.push(format!("{} total", self.total(grades)));
        parts.join(" · ")
    }

    /// Cycles this tally has counted, across the listed grades.
    #[must_use]
    pub fn total(&self, grades: &[String]) -> i32 {
        grades.iter().map(|grade| self.count_for(grade)).sum()
    }
}

/// `1 cycle` / `4 cycles` — one wording, so the card and the log cannot disagree, and "1 cycles"
/// never reaches a reader.
#[must_use]
pub fn describe(cycles: i32) -> String {
    if cycles == 1 {
        "1 cycle".to_owned()
    } else {
        format!("{cycles} cycles")
    }
}
