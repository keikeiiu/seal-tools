"""
Attribute Name Matcher — maps OCR-garbled text to standard names.
Used by the tuner's filter to match user rules against OCR output.

Also handles special attributes with two values:
  每 X 等級增加 Stat +1  →  name: "每級+1力量",  extra: X (levels)

Usage:
  from attr_matcher import match_attributes, check_filter
  result = match_attributes(ocr_attributes)
  passed = check_filter(result, filter_rules)
"""

import re

# ── Standard Attribute Definitions ────────────────────────
# Each entry: (standard_name, category, [ocr_variants])
ATTRIBUTES = [
    # Core damage stats — RapidOCR reads cleanly, minimal variants needed
    ("攻擊力",    "atk",  ["攻擊力", "攻擎力", "攻孽力"]),
    ("魔法力",    "matk", ["魔法力", "魔法才", "庵法力"]),
    ("防禦力",    "def",  ["防禦力", "防票力"]),

    # Combat stats
    ("攻擊速度",  "aspd", ["攻擊速度", "攻擎速度", "攻速度"]),
    ("必殺技",    "crit", ["必殺技"]),
    ("命中率",    "hit",  ["命中率", "命中"]),
    ("迴避率",    "eva",  ["迴避率", "迴避", "迴避室", "避率"]),
    ("移動速度",  "move", ["移動速度", "移動遠度", "瑯動速度", "移動"]),

    # HP/AP
    ("HP(值)",    "hp",   ["HP(值)", "HP"]),
    ("AP(值)",    "ap",   ["AP(值)", "AP"]),
    ("HP(%)",     "hp%",  ["HP(%)", "HP%"]),
    ("AP(%)",     "ap%",  ["AP(%)", "AP%"]),

    # Special / utility
    ("減少道具等級限制", "lvlimit", ["減少道具等級限制", "減少道具配戴限制等級",
                              "減少道具配域限制等級", "減少道具酗域限制等級",
                              "減少限制等級"]),
    ("經驗值獲得量增加", "exp",   ["經驗值獲得量增加", "經驗值增加"]),

    # Damage modifiers
    ("副本傷害增加",   "dgn_dmg", ["副本傷害增加"]),
    ("增加傷害",       "dmg+",    ["增加傷害"]),
    ("減少傷害",       "dmg-",    ["減少傷害", "減少害力"]),

    # Per-level stats (special: has TWO numbers — X levels AND +1/+2 stat)
    ("每級+1力量",  "plv_str",  ["每", "等級", "力量"]),
    ("每級+1敏捷",  "plv_agi",  ["每", "等級", "敏捷"]),
    ("每級+1智力",  "plv_int",  ["每", "等級", "智力"]),
    ("每級+1幸運",  "plv_luk",  ["每", "等級", "幸運"]),
    ("每級+1體力",  "plv_vit",  ["每", "等級", "體力"]),
    ("每級+1精神",  "plv_spi",  ["每", "等級", "精神"]),
    ("每級+2力量",  "plv2_str", ["每", "等級", "力量"]),
    ("每級+2敏捷",  "plv2_agi", ["每", "等級", "敏捷"]),
    ("每級+2智力",  "plv2_int", ["每", "等級", "智力"]),
    ("每級+2幸運",  "plv2_luk", ["每", "等級", "幸運"]),
]


def normalize_name(text):
    """Clean up an OCR text string for matching."""
    t = text.strip()
    # Remove common OCR noise
    t = t.replace('|', '').replace('`', '')
    # Merge spaces
    t = re.sub(r'\s+', '', t)
    return t


def _fuzzy_match(text, variants):
    """Check if text matches any variant (substring OR whole match)."""
    n = normalize_name(text)
    for v in variants:
        nv = normalize_name(v)
        if nv in n or n in nv:
            return True
        # Check if all characters of variant appear in text in order
        if len(nv) >= 2 and all(c in n for c in nv):
            return True
    return False


def _extract_value(texts, prefer_matcher=None):
    """Extract numeric value from text. Prefer text containing the matched label."""
    # First: check text elements that contain the matched label
    for t in texts:
        t = t.strip()
        if prefer_matcher and prefer_matcher in t:
            m = re.search(r'([+-])\s*(\d+)', t)
            if m:
                sign = m.group(1)
                return int(m.group(2)) if sign == '+' else -int(m.group(2))
            m = re.search(r'(\d+)', t)
            if m:
                return int(m.group(1))
    # Fallback: any text with +/- sign
    for t in texts:
        t = t.strip()
        m = re.search(r'([+-])\s*(\d+)', t)
        if m:
            sign = m.group(1)
            val = int(m.group(2))
            return val if sign == '+' else -val
        m = re.search(r'^\s*(\d+)\s*$', t)
        if m:
            return int(m.group(1))
    return None


def _extract_perlevel(texts):
    """Extract level interval and stat from per-level attribute text.
    Example: "每 10 等級 增加 幸運 +1" → {"levels": 10, "stat": "幸運", "bonus": 1}
    """
    combined = ' '.join(texts)
    # Find the level interval
    m = re.search(r'每\s*(\d+)\s*[等級级]', combined)
    if not m:
        # Also try OCR-garbled: "每 4[等級" or "4 40#4+310"
        m = re.search(r'每\s*(\d+)', combined)
    levels = int(m.group(1)) if m else None

    # Find the stat bonus +1 or +2
    bonus = 1
    if '+2' in combined or '2' in combined:
        # Check if "+2" exists
        m2 = re.search(r'[+]\s*2', combined)
        if m2:
            bonus = 2

    # Find which stat
    stat = None
    for s in ["力量", "敏捷", "智力", "幸運", "體力", "精神"]:
        if s in combined:
            stat = s
            break

    return {"levels": levels, "stat": stat, "bonus": bonus} if levels and stat else None


def match_attributes(ocr_attrs):
    """
    Match OCR attribute output to standard names and extract values.

    Args:
        ocr_attrs: [[texts], [texts], [texts]] — 3 lines from OCR engine

    Returns:
        [{"name": "攻擊力", "value": 28, "category": "atk", "matched_via": "14J}"}, ...]
    """
    results = []

    for line in ocr_attrs:
        if not line:
            continue

        combined_text = ' '.join(line)

        # Try each defined attribute
        best_match = None
        best_score = 0

        for std_name, category, variants in ATTRIBUTES:
            # For per-level stats: MUST match the stat name (last variant)
            if category.startswith("plv"):
                # The last variant is the stat name (e.g., "力量", "敏捷")
                stat_variant = variants[-1]
                if stat_variant not in combined_text:
                    continue  # Skip if stat doesn't match
                # Check for "每" and "等級" too
                if "每" not in combined_text and "等級" not in combined_text:
                    continue
                score = len(stat_variant) + 2  # strong match
                best_score = score
                best_match = (std_name, category, stat_variant)
                break

            for variant in variants:
                score = 0
                if variant in combined_text:
                    score = len(variant)
                else:
                    nv = normalize_name(variant)
                    nc = normalize_name(combined_text)
                    if len(nv) >= 2 and all(c in nc for c in nv):
                        score = len(nv) * 0.8
                if score > best_score:
                    best_score = score
                    best_match = (std_name, category, variant)

        if best_match and best_score > 1:
            std_name, category, matched_via = best_match

            # Extract value — prefer text containing the matched label
            value = _extract_value(line, matched_via)

            # Special: per-level stats (two values)
            extra = None
            if category.startswith("plv"):
                plv = _extract_perlevel(line)
                if plv:
                    extra = plv
                    # Use levels as primary value (lower X = better)
                    value = plv["levels"]
                    if plv["bonus"] == 2:
                        std_name = std_name.replace("+1", "+2")

            results.append({
                "name": std_name,
                "category": category,
                "value": value,
                "extra": extra,
                "_raw": line,
            })

    return results


def check_filter(matched_attrs, filter_config):
    """
    Check if matched attributes pass the user's filter rules.

    Args:
        matched_attrs: output from match_attributes()
        filter_config: dict like {"enabled": True, "match_mode": "any", "rules": [...]}

    Returns:
        (passed: bool, matched_rules: list, reason: str)
    """
    if not filter_config.get("enabled", False):
        return True, [], "filter disabled"

    # Override rules: if ANY matches, stop immediately (too good to miss)
    override_rules = filter_config.get("override_rules", [])
    if override_rules:
        for rule in override_rules:
            rname = rule.get("name", "")
            count = rule.get("count", 1)
            matched = 0
            for attr in matched_attrs:
                if attr["name"] == rname:
                    val_ok = True
                    if rule.get("min") is not None and attr["value"] is not None:
                        if attr["value"] < rule["min"]:
                            val_ok = False
                    if rule.get("max") is not None and attr["value"] is not None:
                        if attr["value"] > rule["max"]:
                            val_ok = False
                    if val_ok:
                        matched += 1
            if matched >= count:
                return True, [{"rule": rname, "matched": matched, "override": True}], f"override: {rname} x{matched}"

    rules = filter_config.get("rules", [])
    mode = filter_config.get("match_mode", "any")

    # per_attr mode: each attribute checks against rules, each rule needs min count
    if mode == "per_attr":
        rules = filter_config.get("rules", [])
        # Count matches per rule
        rule_counts = {}
        matched_details = []
        attr_matched = set()  # which attr indices matched at least one rule

        for i, attr in enumerate(matched_attrs):
            for rule in rules:
                rname = rule.get("name", "")
                if attr["name"] == rname:
                    val_ok = True
                    if rule.get("min") is not None and attr["value"] is not None:
                        if attr["value"] < rule["min"]:
                            val_ok = False
                    if rule.get("max") is not None and attr["value"] is not None:
                        if attr["value"] > rule["max"]:
                            val_ok = False
                    if val_ok:
                        rule_counts[rname] = rule_counts.get(rname, 0) + 1
                        matched_details.append({"attr": attr["name"], "value": attr["value"]})
                        attr_matched.add(i)
                        break  # attr matches this rule, move to next attr

        # Check each rule's count requirement
        for rule in rules:
            rname = rule.get("name", "")
            need = rule.get("count", 1)  # default: need at least 1
            got = rule_counts.get(rname, 0)
            if got < need:
                return False, matched_details, f"'{rname}' matched {got} attrs, need {need}"

        return True, matched_details, f"{len(matched_details)} attributes matched"

    matched_rules = []

    for rule in rules:
        rule_name = rule.get("name", "")
        rule_min = rule.get("min")
        rule_max = rule.get("max")

        for attr in matched_attrs:
            if attr["name"] == rule_name:
                # Name matches — check value if specified
                value_ok = True
                if rule_min is not None and attr["value"] is not None:
                    if attr["value"] < rule_min:
                        value_ok = False
                if rule_max is not None and attr["value"] is not None:
                    if attr["value"] > rule_max:
                        value_ok = False
                if value_ok:
                    matched_rules.append({
                        "rule": rule_name,
                        "found": attr["name"],
                        "value": attr["value"],
                    })
                break

    if mode == "any":
        passed = len(matched_rules) > 0
        reason = f"matched {len(matched_rules)}/{len(rules)} rules" if passed else "no rules matched"
    else:  # mode == "all"
        passed = len(matched_rules) >= len(rules)
        reason = f"matched {len(matched_rules)}/{len(rules)} rules" if passed else f"only matched {len(matched_rules)}/{len(rules)}"

    return passed, matched_rules, reason


# ── Test ──────────────────────────────────────────────────
if __name__ == "__main__":
    # Simulate OCR output from earlier scans
    test_cases = [
        # N-grade with 3 simple attrs
        [["防禦力 + 24", "614h + 24"], ["迴避率 + 4", "{04 + 4"], ["減少道具配戴限制等級", "-3"]],
        # G-grade with per-level stat
        [["移動速度 + 5", "#zpj28 + 5"], ["減少道具酗域限制等級", "-1"], ["每 10等級增加幸運", "+1"]],
        # DG with 魔法力
        [["庵法力 + 68"], ["攻擊速度 + 13"], ["攻擊速度 + 12"]],
        # Mixed
        [["HP + 45"], ["迴避率 + 2", "{08# + 2"], ["必殺技 + 3", "~ilt + 3"]],
    ]

    print("=== Attribute Matching Test ===\n")
    for i, test in enumerate(test_cases):
        print(f"Test {i+1}: {test}")
        matched = match_attributes(test)
        for m in matched:
            val_str = f" = {m['value']}" if m['value'] is not None else ""
            extra_str = f" (extra: {m['extra']})" if m.get('extra') else ""
            print(f"  → {m['name']}{val_str}{extra_str} [{m['category']}]")
        print()

    # Test filter rules
    print("=== Filter Rule Test ===\n")
    attrs = match_attributes(test_cases[0])
    print(f"Matched attrs: {[(a['name'], a['value']) for a in attrs]}")

    # Rule 1: any match
    config1 = {"enabled": True, "match_mode": "any", "rules": [
        {"name": "防禦力", "min": 20},
        {"name": "魔法力", "min": 30},
    ]}
    passed, matched, reason = check_filter(attrs, config1)
    print("\nRule: any [防禦力>=20 OR 魔法力>=30]")
    print(f"  Passed: {passed} | {reason}")

    # Rule 2: must match all
    config2 = {"enabled": True, "match_mode": "all", "rules": [
        {"name": "防禦力", "min": 20},
        {"name": "迴避率", "min": 1},
        {"name": "減少道具等級限制", "max": 5},
    ]}
    passed, matched, reason = check_filter(attrs, config2)
    print("\nRule: all [防禦力>=20 AND 迴避率>=1 AND 等級限制<=5]")
    print(f"  Passed: {passed} | {reason}")
