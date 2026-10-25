//! The config model: the `local.yaml` overlay this crate ports from v2's `ConfigLoader`, plus the
//! value types shared with the merged config (`AppConfig.cs`).
//!
//! What is here and what is not:
//!
//! - **Here:** the shapes and their defaults, the null repairs a hand-edited YAML file needs, and
//!   the resolved types the authoring rules operate on ([`QuestConfig`], [`GameConfig`],
//!   [`HotkeysConfig`]).
//! - **Not here:** file I/O, the three-layer load, the merge and the writer. Those are
//!   `seal-config`'s job (`PLAN-V3-CORE.md` §6.1) — this crate has no I/O at all.
//!
//! The JSON and YAML encodings are the same serde data model, and the field names are v2's
//! underscore convention exactly (`UnderscoredNamingConvention`), so a v2 `local.yaml` decodes into
//! these types unchanged.

mod de;
mod local;
mod quest;
mod shared;

pub use local::{
    LocalArduino, LocalBuySell, LocalGem, LocalOverrides, LocalPet, LocalPetQueueEntry,
    LocalPetSlot, LocalQuest, LocalSettings, LocalSettingsArduino, LocalSettingsGame,
    LocalSettingsGem, LocalSettingsPet, LocalSettingsTuner, LocalSpammer, LocalTuner, LocalUi,
};
pub use quest::{
    QuestConfig, QuestPreset, QuestSequence, QuestSequenceEntry, QuestStep, quest_action,
};
pub use shared::{
    BlueMaskConfig, BoxConfig, BuyPreset, CalibrationInfo, ColorComposition, FilterConfig,
    FilterRule, GameConfig, GameVariant, GradeColorsConfig, GradeDecisionConfig, HotkeysConfig,
    ModelsConfig, MovementsConfig, OcrGeometry, PurpleMaskConfig, RedMaskConfig,
    ReferenceWindowConfig, RegionConfig, SpammerCombo, SpammerStartup, TimingConfig, TooltipConfig,
    WhiteMaskConfig, WindowConfig, YellowMaskConfig,
};
