using System.Collections.Generic;

namespace SealTools.Core.Config;

// v2/config/attributes.yaml — the OCR "wordings" dictionary.

public sealed class AttributesConfig
{
    public List<AttributeDef> Attributes { get; set; } = new();
    public List<string> PerLevelStats { get; set; } = new();
    public List<string> NegativeAttrMarkers { get; set; } = new();
    public TextFixesConfig TextFixes { get; set; } = new();
}

public sealed class AttributeDef
{
    public string Name { get; set; } = "";
    public string Category { get; set; } = "";
    public List<string> Variants { get; set; } = new();
}

public sealed class TextFixesConfig
{
    public List<TextFix> Whole { get; set; } = new();
    public List<TextFix> Substring { get; set; } = new();
    public List<TextFix> SimplifiedTraditional { get; set; } = new();
    public List<TextFix> Final { get; set; } = new();
}

public sealed class TextFix
{
    public string From { get; set; } = "";
    public string To { get; set; } = "";
}
