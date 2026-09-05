using System.Text.RegularExpressions;
using SealTools.Core.Config;

namespace SealTools.Tuner;

// Applies the OCR text-cleanup table from attributes.yaml (ported from clean_text()).
public sealed class TextCleaner
{
    private readonly TextFixesConfig _fixes;

    public TextCleaner(TextFixesConfig fixes) => _fixes = fixes;

    public string Clean(string text)
    {
        var t = text.Trim();

        // Whole-string fixes: if the entire string equals `from`, replace and stop.
        foreach (var f in _fixes.Whole)
            if (t == f.From)
                return f.To;

        foreach (var f in _fixes.Substring)
            t = ApplyFix(t, f);

        foreach (var f in _fixes.SimplifiedTraditional)
            t = t.Replace(f.From, f.To);

        foreach (var f in _fixes.Final)
            t = t.Replace(f.From, f.To);

        return t;
    }

    private static string ApplyFix(string t, TextFix f)
    {
        // Pure-ASCII OCR artifacts (GRADB/RADE/CRA) use word boundaries so we don't
        // corrupt a correct "GRADE" into "GGRADE" (matches the v1 \b regex).
        if (f.From.Length > 0 && IsAscii(f.From))
            return Regex.Replace(t, $@"\b{Regex.Escape(f.From)}\b", f.To);
        return t.Replace(f.From, f.To);
    }

    private static bool IsAscii(string s)
    {
        foreach (var c in s)
            if (c > 127)
                return false;
        return true;
    }
}
