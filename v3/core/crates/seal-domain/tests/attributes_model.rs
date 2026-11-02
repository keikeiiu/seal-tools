//! The OCR dictionary model, pinned against `v2/SealTools.Core/Config/AttributesConfig.cs` and the
//! shapes the committed `v2/config/attributes.yaml` actually has.
//!
//! The dictionary is hand-editable by contract ("Adding or correcting an OCR variant is a config
//! edit, not a code change"), so the same tolerance as the config model applies: unknown keys are
//! ignored, a missing section is empty rather than an error, and a key present with no value does not
//! crash the decode. The US variant ships as `attributes: []` and nothing else, which is why the
//! empty shape has to load: pointing the suite at the US client must work before anyone fills it in.

use seal_domain::config::{AttributesConfig, TextFixesConfig};
use serde_json::{Value, json};

/// The committed TW dictionary's shape, with the real fix tables. Values are the committed
/// `v2/config/attributes.yaml` — fair input (never `local.yaml`).
fn tw_dictionary_shape() -> Value {
    json!({
        "attributes": [
            {"name": "攻擊力", "category": "atk", "variants": ["攻擊力", "攻擎力", "攻孽力"]},
            {"name": "減少傷害", "category": "dmg-", "variants": ["減少傷害", "減少害力"]},
            {"name": "每級+1力量", "category": "plv_str", "variants": ["每", "等級", "力量"]}
        ],
        "per_level_stats": ["力量", "敏捷", "智力", "幸運", "體力", "精神"],
        "negative_attr_markers": ["減少", "限制等級", "配戴限制"],
        "text_fixes": {
            "whole": [
                {"from": "GRADB 8", "to": "GRADE :"},
                {"from": "GRADB", "to": "GRADE :"}
            ],
            "substring": [
                {"from": "RADE", "to": "GRADE"},
                {"from": "攻孽遠度", "to": "攻擊速度"},
                {"from": "攻孽", "to": "攻擊"}
            ],
            "simplified_traditional": [
                {"from": "减少", "to": "減少"},
                {"from": "等级", "to": "等級"}
            ],
            "final": [
                {"from": "傷窖", "to": "傷害"},
                {"from": "配域", "to": "配戴"}
            ],
            "regex": [
                {"from": "增加幸(?!運)", "to": "增加幸運"},
                {"from": "增加力(?!量)", "to": "增加力量"}
            ]
        }
    })
}

#[test]
fn the_committed_dictionary_shape_decodes_field_for_field() {
    let config: AttributesConfig = serde_json::from_value(tw_dictionary_shape()).unwrap();

    assert_eq!(config.attributes.len(), 3);
    assert_eq!(config.attributes[0].name, "攻擊力");
    assert_eq!(config.attributes[0].category, "atk");
    assert_eq!(
        config.attributes[0].variants,
        vec!["攻擊力", "攻擎力", "攻孽力"]
    );
    assert_eq!(config.attributes[2].category, "plv_str");
    assert_eq!(config.per_level_stats.len(), 6);
    assert_eq!(
        config.negative_attr_markers,
        vec!["減少", "限制等級", "配戴限制"]
    );

    let fixes = &config.text_fixes;
    assert_eq!(fixes.whole[0].from, "GRADB 8");
    assert_eq!(fixes.whole[0].to, "GRADE :");
    assert_eq!(fixes.substring[1].to, "攻擊速度");
    assert_eq!(fixes.simplified_traditional[0].from, "减少");
    // `final` is a Rust keyword, so the field is `final_rules` renamed back to v2's `final`.
    assert_eq!(fixes.final_rules[0].to, "傷害");
    assert_eq!(fixes.regex[0].from, "增加幸(?!運)");
}

#[test]
fn the_whole_dictionary_round_trips_with_v2_field_names() {
    let fixture = tw_dictionary_shape();
    let config: AttributesConfig = serde_json::from_value(fixture.clone()).unwrap();

    assert_eq!(serde_json::to_value(&config).unwrap(), fixture);
}

/// The US dictionary is `attributes: []` and nothing else — deliberately, so pointing the suite at
/// the US client works out of the box. Every other section must default empty.
#[test]
fn the_shipped_us_shape_loads_as_an_empty_dictionary() {
    let config: AttributesConfig = serde_json::from_value(json!({"attributes": []})).unwrap();

    assert_eq!(
        config.attributes,
        [] as [seal_domain::config::AttributeDef; 0]
    );
    assert_eq!(config.per_level_stats, [] as [std::string::String; 0]);
    assert_eq!(config.negative_attr_markers, [] as [std::string::String; 0]);
    assert_eq!(
        config.text_fixes.whole,
        [] as [seal_domain::config::TextFix; 0]
    );
    assert_eq!(
        config.text_fixes.regex,
        [] as [seal_domain::config::TextFix; 0]
    );
}

#[test]
fn an_empty_or_partial_file_loads_without_error() {
    for fixture in [json!({}), json!({"attributes": []})] {
        let config: AttributesConfig = serde_json::from_value(fixture).unwrap();
        assert_eq!(
            config.attributes,
            [] as [seal_domain::config::AttributeDef; 0]
        );
        assert_eq!(
            config.text_fixes.whole,
            [] as [seal_domain::config::TextFix; 0]
        );
    }

    // A bare `text_fixes:` and a bare `attributes:` are nulls a hand-edit produces.
    let config: AttributesConfig = serde_json::from_value(json!({
        "attributes": null,
        "per_level_stats": null,
        "negative_attr_markers": null,
        "text_fixes": {"whole": null, "substring": null, "regex": null}
    }))
    .unwrap();
    assert_eq!(
        config.attributes,
        [] as [seal_domain::config::AttributeDef; 0]
    );
    assert_eq!(
        config.text_fixes.substring,
        [] as [seal_domain::config::TextFix; 0]
    );
}

/// `IgnoreUnmatchedProperties` in v2: a key nothing models is ignored on load.
#[test]
fn unknown_keys_are_tolerated() {
    let config: AttributesConfig = serde_json::from_value(json!({
        "attributes": [{"name": "攻擊力", "category": "atk", "variants": ["攻擊力"], "future_field": 1}],
        "a_section_from_a_later_release": ["x"]
    }))
    .unwrap();

    assert_eq!(config.attributes.len(), 1);
}

/// An entry with only `from` is a shape a hand-edit produces. v2 would hand `null` to
/// `string.Replace` and throw; the decode repairs it to an empty replacement instead, which the
/// cleaner then rejects if the cleaner cannot honour it (a `from` is required, a `to` is not).
#[test]
fn a_fix_entry_missing_its_replacement_decodes_to_empty() {
    let config: AttributesConfig = serde_json::from_value(json!({
        "text_fixes": {"substring": [{"from": "GRADB"}, {"from": "RADE", "to": "GRADE"}]}
    }))
    .unwrap();

    assert_eq!(config.text_fixes.substring[0].from, "GRADB");
    assert_eq!(config.text_fixes.substring[0].to, "");
    assert_eq!(config.text_fixes.substring[1].to, "GRADE");
}

#[test]
fn a_dictionary_with_no_fix_tables_has_empty_tables() {
    let fixes = TextFixesConfig::default();
    assert_eq!(fixes.whole, [] as [seal_domain::config::TextFix; 0]);
    assert_eq!(fixes.substring, [] as [seal_domain::config::TextFix; 0]);
    assert_eq!(
        fixes.simplified_traditional,
        [] as [seal_domain::config::TextFix; 0]
    );
    assert_eq!(fixes.final_rules, [] as [seal_domain::config::TextFix; 0]);
    assert_eq!(fixes.regex, [] as [seal_domain::config::TextFix; 0]);
}
