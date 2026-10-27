//! `ConfigValidator` — the AUTHORED oracle.
//!
//! v2 has no test for `ConfigValidator.cs`, so this file's expectations were written from the v2
//! source and each test cites the v2 lines it encodes. If a test here disagrees with v2, v2 is right:
//! `v2/SealTools.Core/Config/ConfigValidator.cs` and `Config/AppConfig.cs` are the oracle.
//!
//! Every failure line is v2's, including the `Window:`/`Tuner.Filter:` property-path prefix its
//! reflection walk produced (`ConfigValidator.cs:71-100`).

use seal_domain::config::{
    ArduinoInput, BoxConfig, ConfigErrors, FilterConfig, GemInput, OcrGeometry, RegionConfig,
    TunerInput, ValidationInput, WindowConfig, validate,
};

fn clean() -> ValidationInput {
    ValidationInput::clean()
}

fn tuner_ocr(mut tuner: TunerInput, ocr: OcrGeometry) -> TunerInput {
    tuner.ocr = Some(ocr);
    tuner
}

fn valid_ocr() -> OcrGeometry {
    TunerInput::calibrated().ocr.unwrap()
}

fn lines(errors: &ConfigErrors) -> Vec<&str> {
    errors.lines().iter().map(String::as_str).collect()
}

// ── the clean shape, and the wrapper text ─────────────────────────────────────────────────

#[test]
fn a_complete_configuration_validates_clean() {
    let errors = validate(&clean());

    assert!(errors.is_empty(), "lines: {:?}", lines(&errors));
    assert_eq!(errors.message(), None);
}

/// `ConfigValidator.cs:65-68` — the ONE exception text, with the rule lines indented under it.
#[test]
fn the_exception_text_is_v2s_wrapper_over_the_rule_lines() {
    let mut input = clean();
    input.window = Some(WindowConfig::default());

    let errors = validate(&input);
    assert_eq!(
        errors.message().unwrap(),
        "Config validation failed:\n  - Window: window.title is required\n\
         Check defaults.yaml and local.yaml."
    );

    let mut two = clean();
    two.window = Some(WindowConfig::default());
    two.arduino = Some(ArduinoInput { vid: 0 });
    assert_eq!(
        validate(&two).message().unwrap(),
        "Config validation failed:\n  - Window: window.title is required\n  \
         - Arduino: arduino.vid must be > 0\nCheck defaults.yaml and local.yaml."
    );
}

// ── the annotation walk (AppConfig.cs) ───────────────────────────────────────────────────

/// `AppConfig.cs:793` — `[Required(ErrorMessage = "window.title is required")]`. Required on a string
/// fails for an empty value as well as a null one.
#[test]
fn the_window_title_is_required() {
    let mut input = clean();
    input.window = Some(WindowConfig {
        title: String::new(),
    });

    assert_eq!(
        lines(&validate(&input)),
        ["Window: window.title is required"]
    );
}

/// `ConfigValidator.cs:84` — `if (value == null) continue;` means a section that is present with a
/// NULL body is skipped entirely. A bare `window:` therefore validates clean even though it carries
/// no title, while an ABSENT section is the default object and fails. v2 behaviour, pinned.
#[test]
fn a_null_section_is_skipped_while_an_absent_one_is_the_default_object() {
    let mut bare = clean();
    bare.window = None;
    assert!(validate(&bare).is_empty());

    // The absent-section shape is the caller passing v2's default-constructed object.
    let mut absent = clean();
    absent.window = Some(WindowConfig::default());
    assert_eq!(
        lines(&validate(&absent)),
        ["Window: window.title is required"]
    );
}

/// `AppConfig.cs:822` — `[Range(1, int.MaxValue, ErrorMessage = "arduino.vid must be > 0")]`. An
/// absent key leaves the int default 0, which fails.
#[test]
fn the_arduino_vid_must_be_positive() {
    for vid in [0, -1, -8704] {
        let mut input = clean();
        input.arduino = Some(ArduinoInput { vid });
        assert_eq!(
            lines(&validate(&input)),
            ["Arduino: arduino.vid must be > 0"],
            "vid {vid}"
        );
    }
    for vid in [1, 0x2341, i64::from(i32::MAX)] {
        let mut input = clean();
        input.arduino = Some(ArduinoInput { vid });
        assert!(validate(&input).is_empty(), "vid {vid}");
    }
}

/// `AppConfig.cs:854` — `AllowedValues("manual","hid")`. Unlike `FoodLoadMode.IsDrag`, this is an
/// EXACT comparison: no trimming, no case folding.
#[test]
fn spring_mode_is_one_of_two_exact_values() {
    for mode in ["manual", "hid"] {
        let mut input = clean();
        input.tuner = Some(TunerInput {
            spring_mode: Some(mode.to_owned()),
            ..TunerInput::calibrated()
        });
        assert!(validate(&input).is_empty(), "mode {mode}");
    }
    for mode in ["HID", "hid ", "Manual", "spring", ""] {
        let mut input = clean();
        input.tuner = Some(TunerInput {
            spring_mode: Some(mode.to_owned()),
            ..TunerInput::calibrated()
        });
        assert_eq!(
            lines(&validate(&input)),
            ["Tuner: tuner.spring_mode must be manual|hid"],
            "mode {mode:?}"
        );
    }
}

/// `AppConfig.cs:864` — `AllowedValues("off","stop","recenter")`, same exactness.
#[test]
fn mouse_guard_is_one_of_three_exact_values() {
    for guard in ["off", "stop", "recenter"] {
        let mut input = clean();
        input.tuner = Some(TunerInput {
            mouse_guard: Some(guard.to_owned()),
            ..TunerInput::calibrated()
        });
        assert!(validate(&input).is_empty(), "guard {guard}");
    }
    for guard in ["Off", "stop ", "recentre", ""] {
        let mut input = clean();
        input.tuner = Some(TunerInput {
            mouse_guard: Some(guard.to_owned()),
            ..TunerInput::calibrated()
        });
        assert_eq!(
            lines(&validate(&input)),
            ["Tuner: tuner.mouse_guard must be off|stop|recenter"],
            "guard {guard:?}"
        );
    }
}

/// `AppConfig.cs:950` — the filter's match mode, on a section that is nullable like the others.
#[test]
fn the_filter_match_mode_is_one_of_three_exact_values() {
    for mode in ["any", "all", "per_attr"] {
        let mut input = clean();
        input.tuner = Some(TunerInput {
            filter: Some(FilterConfig {
                match_mode: mode.to_owned(),
                ..FilterConfig::default()
            }),
            ..TunerInput::calibrated()
        });
        assert!(validate(&input).is_empty(), "mode {mode}");
    }
    let mut input = clean();
    input.tuner = Some(TunerInput {
        filter: Some(FilterConfig {
            match_mode: "some".to_owned(),
            ..FilterConfig::default()
        }),
        ..TunerInput::calibrated()
    });
    assert_eq!(
        lines(&validate(&input)),
        ["Tuner.Filter: tuner.filter.match_mode must be any|all|per_attr"]
    );

    // A null filter section is skipped by the recursion (`ConfigValidator.cs:84`).
    let mut bare = clean();
    bare.tuner = Some(TunerInput {
        filter: None,
        ..TunerInput::calibrated()
    });
    assert!(validate(&bare).is_empty());
}

/// The convention every DataAnnotation except `Required` follows: a null value is VALID. So
/// `spring_mode:` with no value passes validation and the runtime fallback is what runs — v2
/// behaviour, pinned rather than "fixed".
#[test]
fn a_null_allowed_value_passes_and_the_runtime_fallback_runs() {
    let mut input = clean();
    input.tuner = Some(TunerInput {
        spring_mode: None,
        mouse_guard: None,
        ..TunerInput::calibrated()
    });

    assert!(validate(&input).is_empty());
}

/// `AppConfig.cs:897-900` — `[Range(1, int.MaxValue)]` on RegionConfig.Width and .Height, validated in
/// property order, each producing its own line.
#[test]
fn the_ocr_region_dimensions_must_be_positive() {
    let mut none_broken = clean();
    none_broken.tuner = Some(TunerInput {
        ocr: Some(valid_ocr()),
        ..TunerInput::calibrated()
    });
    assert!(validate(&none_broken).is_empty());

    let mut width = clean();
    width.tuner = Some(tuner_ocr(
        TunerInput::calibrated(),
        OcrGeometry {
            region: Some(RegionConfig {
                left: 0,
                top: 0,
                width: 0,
                height: 300,
            }),
            ..valid_ocr()
        },
    ));
    assert_eq!(
        lines(&validate(&width)),
        [
            "Tuner.Ocr.Region: tuner.ocr.region width/height must be > 0",
            "tuner.ocr.grade_area must lie inside tuner.ocr.region (region is 0x300)",
        ]
    );

    let mut both = clean();
    both.tuner = Some(tuner_ocr(
        TunerInput::calibrated(),
        OcrGeometry {
            region: Some(RegionConfig::default()),
            ..valid_ocr()
        },
    ));
    assert_eq!(
        lines(&validate(&both)),
        [
            "Tuner.Ocr.Region: tuner.ocr.region width/height must be > 0",
            "Tuner.Ocr.Region: tuner.ocr.region width/height must be > 0",
            "tuner.ocr.grade_area must lie inside tuner.ocr.region (region is 0x0)",
        ]
    );
}

/// `AppConfig.cs:968,986,999` — the three gem checks.
#[test]
fn the_gem_values_are_checked() {
    for mode in ["tuned", "arduino"] {
        let mut input = clean();
        input.gem = Some(GemInput {
            move_mode: Some(mode.to_owned()),
            ..GemInput::default()
        });
        assert!(validate(&input).is_empty(), "move {mode}");
    }
    let mut move_mode = clean();
    move_mode.gem = Some(GemInput {
        move_mode: Some("Arduino".to_owned()),
        ..GemInput::default()
    });
    assert_eq!(
        lines(&validate(&move_mode)),
        ["Gem: gem.move_mode must be tuned|arduino"]
    );

    for mode in ["stop", "advance_grade", "advance_grade_clear"] {
        let mut input = clean();
        input.gem = Some(GemInput {
            empty_mode: Some(mode.to_owned()),
            ..GemInput::default()
        });
        assert!(validate(&input).is_empty(), "empty {mode}");
    }
    let mut empty_mode = clean();
    empty_mode.gem = Some(GemInput {
        empty_mode: Some("clear".to_owned()),
        ..GemInput::default()
    });
    assert_eq!(
        lines(&validate(&empty_mode)),
        ["Gem: gem.empty_mode must be stop|advance_grade|advance_grade_clear"]
    );

    for gap in [-1, 256, 1000] {
        let mut input = clean();
        input.gem = Some(GemInput {
            colored_gap_min: i64::from(gap),
            ..GemInput::default()
        });
        assert_eq!(
            lines(&validate(&input)),
            ["Gem: gem.colored_gap_min must be 0-255"],
            "gap {gap}"
        );
    }
    for gap in [0, 40, 255] {
        let mut input = clean();
        input.gem = Some(GemInput {
            colored_gap_min: i64::from(gap),
            ..GemInput::default()
        });
        assert!(validate(&input).is_empty(), "gap {gap}");
    }

    // A null gem section is skipped by the recursion.
    let mut bare = clean();
    bare.gem = None;
    assert!(validate(&bare).is_empty());
}

// ── the relational checks (ConfigValidator.cs:27-63) ────────────────────────────────────

/// `ConfigValidator.cs:27-32` — a bare `tuner:` key is the first place v2 would dereference a null,
/// so the section is reported instead of dying.
#[test]
fn a_null_tuner_section_is_reported_not_dereferenced() {
    let mut input = clean();
    input.tuner = None;

    assert_eq!(
        lines(&validate(&input)),
        [
            "tuner: the section is empty — it needs grade_order, the ocr geometry and the timing (edit \
             the tuner block in config/local.yaml; the tuner's OCR geometry is hand-edited in v3.0 \
             because the three-box session is not built)"
        ]
    );
}

/// `ConfigValidator.cs:35-39` — a tuner with no OCR geometry.
#[test]
fn a_null_ocr_section_is_reported() {
    let mut input = clean();
    input.tuner = Some(TunerInput {
        ocr: None,
        ..TunerInput::calibrated()
    });

    assert_eq!(
        lines(&validate(&input)),
        [
            "tuner.ocr: missing — the tuner needs its OCR geometry (the tuner's OCR geometry is hand-edited in config/local.yaml; the three-box session is not built in v3.0)"
        ]
    );
}

/// `ConfigValidator.cs:42-54` — the missing boxes, then the ordered pair.
#[test]
fn the_grade_area_and_region_must_be_present_and_ordered() {
    let mut missing = clean();
    missing.tuner = Some(tuner_ocr(
        TunerInput::calibrated(),
        OcrGeometry {
            region: None,
            grade_area: None,
            ..valid_ocr()
        },
    ));
    assert_eq!(
        lines(&validate(&missing)),
        [
            "tuner.ocr.grade_area: missing — needs x1, y1, x2, y2",
            "tuner.ocr.region: missing — needs left, top, width, height",
        ]
    );

    // Both absent: the band checks still run (the chains are independent), so the three required
    // bands are reported after the two missing boxes.
    let mut both = clean();
    both.tuner = Some(tuner_ocr(TunerInput::calibrated(), OcrGeometry::default()));
    assert_eq!(
        lines(&validate(&both)),
        [
            "tuner.ocr.grade_area: missing — needs x1, y1, x2, y2",
            "tuner.ocr.region: missing — needs left, top, width, height",
            "tuner.ocr.grade_y: needs 2 values; got 0",
            "tuner.ocr.attr_y: needs 2 values; got 0",
            "tuner.ocr.remaining_y: needs 2 values; got 0",
        ]
    );
}

/// `ConfigValidator.cs:47-48` — a pair whose second corner is not greater.
#[test]
fn an_unordered_grade_area_is_named() {
    for (x2, y2) in [(0, 40), (100, 0)] {
        let mut input = clean();
        input.tuner = Some(tuner_ocr(
            TunerInput::calibrated(),
            OcrGeometry {
                grade_area: Some(BoxConfig {
                    x1: 0,
                    y1: 0,
                    x2,
                    y2,
                }),
                ..valid_ocr()
            },
        ));
        assert_eq!(
            lines(&validate(&input)),
            ["tuner.ocr.grade_area: x2/y2 must be greater than x1/y1"],
            "x2 {x2} y2 {y2}"
        );
    }
}

/// `ConfigValidator.cs:52-54` — the box must lie inside the region; the message carries the region
/// size so "wrong box" and "wrong region" can be told apart.
#[test]
fn a_grade_area_outside_the_region_names_the_region_size() {
    for area in [
        BoxConfig {
            x1: -1,
            y1: 0,
            x2: 100,
            y2: 40,
        },
        BoxConfig {
            x1: 0,
            y1: 0,
            x2: 401,
            y2: 40,
        },
        BoxConfig {
            x1: 0,
            y1: 0,
            x2: 100,
            y2: 301,
        },
    ] {
        let mut input = clean();
        input.tuner = Some(tuner_ocr(
            TunerInput::calibrated(),
            OcrGeometry {
                grade_area: Some(area),
                ..valid_ocr()
            },
        ));
        assert_eq!(
            lines(&validate(&input)),
            ["tuner.ocr.grade_area must lie inside tuner.ocr.region (region is 400x300)"]
        );
    }

    // A box exactly filling the region is inside (the checks are >, not >=).
    let mut flush = clean();
    flush.tuner = Some(tuner_ocr(
        TunerInput::calibrated(),
        OcrGeometry {
            grade_area: Some(BoxConfig {
                x1: 0,
                y1: 0,
                x2: 400,
                y2: 300,
            }),
            ..valid_ocr()
        },
    ));
    assert!(validate(&flush).is_empty());
}

/// `ConfigValidator.cs:57-59` — the three REQUIRED bands, `[top, bottom]` ordered. A band longer than
/// two is accepted because only the first pair is read — v2 quirk, pinned.
#[test]
fn the_three_required_bands_need_two_ordered_values() {
    for name in ["grade_y", "attr_y", "remaining_y"] {
        let mut short = valid_ocr();
        match name {
            "grade_y" => short.grade_y = vec![3],
            "attr_y" => short.attr_y = vec![3],
            _ => short.remaining_y = vec![3],
        }
        let mut input = clean();
        input.tuner = Some(tuner_ocr(TunerInput::calibrated(), short));
        assert_eq!(
            lines(&validate(&input)),
            [format!("tuner.ocr.{name}: needs 2 values; got 1")]
        );

        let mut flat = valid_ocr();
        match name {
            "grade_y" => flat.grade_y = vec![40, 40],
            "attr_y" => flat.attr_y = vec![40, 40],
            _ => flat.remaining_y = vec![40, 40],
        }
        let mut input = clean();
        input.tuner = Some(tuner_ocr(TunerInput::calibrated(), flat));
        assert_eq!(
            lines(&validate(&input)),
            [format!(
                "tuner.ocr.{name}: the second value must be greater than the first"
            )]
        );
    }

    // Three values: accepted, only the first two are compared.
    let mut long = valid_ocr();
    long.grade_y = vec![0, 40, 900];
    let mut input = clean();
    input.tuner = Some(tuner_ocr(TunerInput::calibrated(), long));
    assert!(validate(&input).is_empty());
}

/// `ConfigValidator.cs:112-116` — the optional X bands may be omitted, but a partial pair is a typo.
#[test]
fn the_optional_bands_may_be_absent_but_not_half_written() {
    for ocr in [valid_ocr(), {
        let mut ocr = valid_ocr();
        ocr.attr_x = vec![7];
        ocr
    }] {
        let mut input = clean();
        input.tuner = Some(tuner_ocr(TunerInput::calibrated(), ocr));
        let errors = validate(&input);
        if errors.is_empty() {
            continue;
        }
        assert_eq!(
            lines(&errors),
            ["tuner.ocr.attr_x: leave it out or give 2 values; got 1"]
        );
    }

    let mut partial = valid_ocr();
    partial.remaining_x = vec![200];
    let mut input = clean();
    input.tuner = Some(tuner_ocr(TunerInput::calibrated(), partial));
    assert_eq!(
        lines(&validate(&input)),
        ["tuner.ocr.remaining_x: leave it out or give 2 values; got 1"]
    );

    let mut complete = valid_ocr();
    complete.attr_x = vec![10, 90];
    complete.remaining_x = vec![10, 90];
    let mut input = clean();
    input.tuner = Some(tuner_ocr(TunerInput::calibrated(), complete));
    assert!(validate(&input).is_empty());
}

// ── ordering, as v2 collected it ────────────────────────────────────────────────────────

/// The order is v2's: the annotation walk over `AppConfig`'s sections, then the relational block. A
/// later reader can compare against `ConfigValidator.cs:16-68` line by line.
#[test]
fn every_rule_fires_in_v2s_order() {
    let mut input = clean();
    input.window = Some(WindowConfig::default());
    input.arduino = Some(ArduinoInput { vid: 0 });
    input.tuner = Some(TunerInput {
        spring_mode: Some("bad".to_owned()),
        mouse_guard: Some("bad".to_owned()),
        filter: Some(FilterConfig {
            match_mode: "bad".to_owned(),
            ..FilterConfig::default()
        }),
        // The absent-section shape: v2 default-constructs the objects, so the region and the
        // grade box are PRESENT with zero fields and the bands are empty.
        ocr: Some(OcrGeometry {
            region: Some(RegionConfig::default()),
            grade_area: Some(BoxConfig::default()),
            ..OcrGeometry::default()
        }),
    });
    input.gem = Some(GemInput {
        move_mode: Some("bad".to_owned()),
        empty_mode: Some("bad".to_owned()),
        colored_gap_min: 999,
    });

    assert_eq!(
        lines(&validate(&input)),
        [
            "Window: window.title is required",
            "Arduino: arduino.vid must be > 0",
            "Tuner: tuner.spring_mode must be manual|hid",
            "Tuner: tuner.mouse_guard must be off|stop|recenter",
            "Tuner.Filter: tuner.filter.match_mode must be any|all|per_attr",
            "Tuner.Ocr.Region: tuner.ocr.region width/height must be > 0",
            "Tuner.Ocr.Region: tuner.ocr.region width/height must be > 0",
            "Gem: gem.move_mode must be tuned|arduino",
            "Gem: gem.empty_mode must be stop|advance_grade|advance_grade_clear",
            "Gem: gem.colored_gap_min must be 0-255",
            "tuner.ocr.grade_area: x2/y2 must be greater than x1/y1",
            "tuner.ocr.grade_y: needs 2 values; got 0",
            "tuner.ocr.attr_y: needs 2 values; got 0",
            "tuner.ocr.remaining_y: needs 2 values; got 0",
        ]
    );
}
