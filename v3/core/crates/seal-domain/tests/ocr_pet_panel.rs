//! `PetPanel` — the hover-panel parser, translated from v2's `SealTools.Tests/PetPanelTests.cs`.
//!
//! The tests are mostly about panels that must NOT parse, because the caller fails open: an
//! unparsed panel reloads as normal, so the dangerous bug is a panel that parses WRONG.
//!
//! One translation note: v2's `Parse(IEnumerable<string>? lines)` accepts null and answers null. A
//! Rust `&[String]` cannot be null, and an absent line list is the empty slice, which gives the same
//! answer — so the "null lines give null" case is covered by the empty test.

use seal_domain::ocr::PetPanel;

fn parse(lines: &[&str]) -> Option<PetPanel> {
    let owned: Vec<String> = lines.iter().map(|line| (*line).to_owned()).collect();
    PetPanel::parse(&owned)
}

/// The measured read, verbatim (2026-09-18): the panel on screen was
/// `(6階) 真蔚藍鳳凰 +7 [52.18%]` and the recogniser returned the line below.
const LIVE_READ: &str = "（6）真蔚蓝凤凰+7[52.18%]";

#[test]
fn parses_the_live_read() {
    let panel = parse(&[LIVE_READ]).unwrap();

    assert_eq!(panel.stage, Some(6));
    assert_eq!(panel.growth, 7);
    assert!((panel.exp - 52.18).abs() < 0.001);
    assert!(!panel.is_finished());
}

#[test]
fn parses_across_several_lines() {
    let panel = parse(&[
        LIVE_READ,
        "所有瞬業皆可使用", // 職業 misread, and it must not matter
        "贩直價格",         // 販賣 misread, likewise
        "1,234",
    ])
    .unwrap();

    assert_eq!(panel.growth, 7);
}

#[test]
fn a_full_bar_on_top_growth_is_finished() {
    let panel = parse(&["（6）真蔚蓝凤凰+9[100%]"]).unwrap();

    assert!(panel.is_finished());
}

/// A growth read one digit too long reads as its FIRST digit — which for "+99" is the 9 that is
/// really there, so a finished pet still reads as finished (measured live, 2026-09-21).
#[test]
fn a_growth_read_one_digit_too_long_still_reads_the_digit() {
    let panel = parse(&["（6）真蔚蓝凤凰+99[100%]"]).unwrap();

    assert_eq!(panel.growth, PetPanel::MAX_GROWTH);
    assert!(panel.is_finished());
}

/// Both halves must hold. Each of these is finished by one number and not the other; reading either
/// alone would get the pet fed forever or dropped while it still needs food.
#[test]
fn one_number_alone_is_not_finished() {
    for line in [
        "（6）真蔚蓝凤凰+9[99.99%]", // top growth, bar still filling
        "（6）真蔚蓝凤凰+7[100%]",   // full bar, seven more levels to go
        "（6）真蔚蓝凤凰+8[100%]",
    ] {
        let panel = parse(&[line]).unwrap();
        assert!(!panel.is_finished(), "{line}");
    }
}

/// A growth above the game's maximum is a misread; "at least 9" would answer "done" to a garbage
/// panel and stop feeding a pet that needs it. The text is the paid extension's label in the same
/// window, and the one-digit rule is what keeps it out of the growth.
#[test]
fn a_growth_above_the_maximum_is_never_finished() {
    let panel = parse(&["（6）真蔚蓝凤凰+15 Days[100%]"]).unwrap();

    assert_eq!(panel.growth, 1);
    assert!(!panel.is_finished());
}

// ── what must NOT parse ─────────────────────────────────────────────────────────────────────────

#[test]
fn nothing_read_gives_none() {
    assert!(parse(&[]).is_none());
    // v2's null-lines case cannot be expressed as a slice; empty is its Rust spelling.
}

/// A food stack's tooltip: a count, but no growth and no percentage. It shares the same offset and
/// the same box, so a scan that hovers the wrong cell lands here.
#[test]
fn a_food_panel_is_not_a_pet_parse() {
    assert!(parse(&["高級寵物食物", "300"]).is_none());
}

#[test]
fn growth_without_exp_gives_none() {
    assert!(parse(&["（6）真蔚蓝凤凰+7"]).is_none());
}

/// The bracket is what makes a percentage an EXP bar rather than any other number that carries a %.
#[test]
fn an_unbracketed_percentage_is_not_a_pet() {
    assert!(parse(&["某個物品", "增加 10% 的掉落率"]).is_none());
}

/// The other two panels the same session produced, both from hovering the wrong thing.
#[test]
fn the_other_tooltips_the_session_read_are_not_pets() {
    assert!(
        parse(&[
            "能量的舜渣",
            "促希菌特大陵各每收集而来的神秘掉落物：",
            "邂子城的?能量守恒者?知道這是什应東西?拿去给他看看吧",
        ])
        .is_none()
    );
    assert!(
        parse(&[
            "奥藤的請求信",
            "奥藤為了找回冬天，正在收集大量的",
            "有效期間",
            "2026年9月月20日0時58分",
        ])
        .is_none()
    );
}

// ── the whole panel, as the live game actually produced it (2026-09-19) ────────────────────────

/// One read verbatim, from the Pet tab's Test read. It is the fixture that matters most because it
/// is the only panel that was NOT hand-written.
fn live_zero_pet() -> [&'static str; 5] {
    [
        "（6踏）真蔚蓝米鲁[0.06%]",
        "所有瞬業皆可使用",
        "等級限制1150",
        "名望限制51595",
        "贩直價格-500000s",
    ]
}

/// A +0 pet renders NO "+N". Requiring one rejected every pet at the start of its run, which is the
/// pet this tool feeds most often.
#[test]
fn a_zero_pet_has_no_growth_and_parses_as_zero() {
    let lines = live_zero_pet();
    let panel = parse(&lines).unwrap();

    assert_eq!(panel.stage, Some(6));
    assert_eq!(panel.growth, 0);
    assert!((panel.exp - 0.06).abs() < 0.001);
    assert!(!panel.is_finished());
}

/// The 階 was misread as 踏 and the stage still has to come out.
#[test]
fn a_mangled_stage_unit_still_parses_the_stage() {
    let lines = live_zero_pet();
    assert_eq!(parse(&lines).unwrap().stage, Some(6));
}

/// The panel states the sell price, the level limit and the fame limit — three large numbers a parser
/// hunting for digits would happily mistake for something. Anchoring on the bracket and the + is what
/// makes them impossible to pick up.
#[test]
fn the_panels_other_numbers_are_not_mistaken_for_growth_or_exp() {
    let lines = live_zero_pet();
    let panel = parse(&lines).unwrap();

    assert_eq!(panel.growth, 0); // not 150, not 51595, not 500000
    assert!((panel.exp - 0.06).abs() < 0.001); // and not one of those either
}

// ── shapes the recogniser plausibly returns ────────────────────────────────────────────────────

/// A full-width digit is a different character to the recogniser and the same number to a reader.
#[test]
fn full_width_digits_parse() {
    let panel = parse(&["（６）真蔚蓝凤凰＋９［１００％］"]).unwrap();

    assert_eq!(panel.stage, Some(6));
    assert_eq!(panel.growth, 9);
    assert!(panel.is_finished());
}

#[test]
fn the_stage_unit_is_optional() {
    for line in [
        "（6）真蔚蓝凤凰+7[52.18%]",
        "(6階)真蔚蓝凤凰+7[52.18%]",
        "(6阶)真蔚蓝凤凰+7[52.18%]",
    ] {
        assert_eq!(parse(&[line]).unwrap().stage, Some(6), "{line}");
    }
}

/// The stage is the one value nothing acts on yet, so losing ONLY the stage still yields a usable
/// panel.
#[test]
fn a_missing_stage_still_parses() {
    let panel = parse(&["真蔚蓝凤凰+7[52.18%]"]).unwrap();

    assert_eq!(panel.stage, None);
    assert_eq!(panel.growth, 7);
}

/// Spacing is not ours to rely on: the game's own panel has spaces where the read does not, and OCR
/// moves them around.
#[test]
fn spacing_is_irrelevant() {
    let panel = parse(&["(6階) 真蔚藍鳳凰 +9 [ 100 %]"]).unwrap();

    assert_eq!(panel.growth, 9);
    assert!((panel.exp - 100.0).abs() < 0.001);
    assert!(panel.is_finished());
}

#[test]
fn describe_says_what_it_read() {
    let panel = parse(&["（6）真蔚蓝凤凰+7[52.18%]"]).unwrap();
    assert_eq!(panel.describe(), "stage 6, +7, 52.18%");

    let finished = parse(&["（6）真蔚蓝凤凰+9[100%]"]).unwrap();
    assert_eq!(finished.describe(), "stage 6, +9, 100%  ← finished (+10)");

    let no_stage = parse(&["真蔚蓝凤凰+7[52.18%]"]).unwrap();
    assert!(
        no_stage.describe().starts_with("stage ?, "),
        "got {:?}",
        no_stage.describe()
    );
}
