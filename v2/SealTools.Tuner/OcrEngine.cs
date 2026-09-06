using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;
using System.Text.Json;
using OpenCvSharp;
using RapidOCRSharpOnnx;
using RapidOCRSharpOnnx.Configurations;
using RapidOCRSharpOnnx.Inference.PPOCR_Det.Models;
using RapidOCRSharpOnnx.Providers;
using RapidOCRSharpOnnx.Utils;
using SealTools.Core;
using SealTools.Core.Config;

namespace SealTools.Tuner;

// Faithful port of tuner/ocr_engine.py TuningOCR. Geometry, grade-color thresholds,
// and model paths all come from config (no hardcoded values).

public sealed class ScanResult
{
    public string? Grade { get; set; }
    public Dictionary<string, int> ColorScores { get; set; } = new();
    public int? Remaining { get; set; }
    public List<string> GradeLine { get; set; } = new();
    public List<List<string>> Attributes { get; set; } = new();
    public string? CapturePath { get; set; }
    public WindowRect? Window { get; set; }

    /// <summary>
    /// The vertical spacing between attribute rows, measured from the raw detected items
    /// in the attribute band. Used by the calibrator to persist an accurate row_height
    /// instead of assuming attr.Height/3 (which is only correct if the box is drawn tight
    /// around the three lines). Null when it couldn't be measured.
    /// </summary>
    public int? AttrLineSpacing { get; set; }
}

public sealed class OcrEngine : IDisposable
{
    private readonly AppConfig _cfg;
    private readonly TextCleaner _cleaner;
    private readonly List<string> _negativeMarkers;
    private readonly string _rootDir;
    private readonly string _captureDir;
    private readonly FileLogger _ocrLog;
    private RapidOCRSharp? _ocr;

    public OcrEngine(AppConfig cfg, AttributesConfig attributes, string rootDir)
    {
        _cfg = cfg;
        _cleaner = new TextCleaner(attributes.TextFixes);
        _negativeMarkers = attributes.NegativeAttrMarkers;
        _rootDir = rootDir;
        _captureDir = Path.Combine(rootDir, "logs", "captures");
        Directory.CreateDirectory(_captureDir);
        _ocrLog = new FileLogger(Path.Combine(rootDir, "logs", "ocr_log.jsonl"));
    }

    private void InitOcr()
    {
        if (_ocr != null) return;
        var m = _cfg.Tuner.Models;
        var ocrConfig = new OcrConfig(
            Resolve(m.Detector), Resolve(m.Recognizer), LangRec.CH, OCRVersion.PPOCRV4, Resolve(m.Classifier))
        {
            ReturnWordBox = true,
        };
        _ocr = new RapidOCRSharp(new ExecutionProviderCPU(ocrConfig));
    }

    private string Resolve(string path) =>
        Path.IsPathRooted(path) ? path : Path.GetFullPath(Path.Combine(_rootDir, path));

    public ScanResult? Scan() => Scan(_cfg.Tuner.Ocr);

    public ScanResult? Scan(OcrGeometry ocr)
    {
        var hwnd = WindowFinder.FindByTitle(_cfg.Window.Title);
        var client = WindowFinder.GetClientRectInScreen(hwnd);
        if (client == null) return null;

        InitOcr();

        var region = ocr.Region;
        using var mat = ScreenCapture.CaptureRegion(client, region);

        var timestamp = DateTime.Now.ToString("yyyyMMdd_HHmmss_fff", CultureInfo.InvariantCulture);
        // Captures are debug-only artifacts; save_captures:false keeps the disk clean for end users.
        var capturePath = _cfg.Tuner.SaveCaptures ? Path.Combine(_captureDir, $"capture_{timestamp}.png") : null;

        var ocrResult = _ocr!.RecognizeText(mat, capturePath!);
        var items = ocrResult.WordResults ?? Array.Empty<DetBoxItem>();

        var ga = ocr.GradeArea;
        var gy = ocr.GradeY;
        var at = ocr.AttrY;
        var ry = ocr.RemainingY;
        var ax = ocr.AttrX;
        var rx = ocr.RemainingX;
        int rowHeight = ocr.RowHeight;

        // Reconstruct line-level results from char-level WordResults.
        var textLines = BuildLines(items, rowHeight);
        // Attributes and remaining are additionally filtered by X, so a stray element
        // sitting beside them at the same Y (e.g. the "X" near the count) is excluded.
        var attrTextLines = BuildLines(FilterItems(items, ax, at), rowHeight);
        var remainingTextLines = BuildLines(FilterItems(items, rx, ry), rowHeight);

        // 1. Grade line (full OCR, Y-filtered) — the "<label>：<letter>" line.
        var gradeLine = new List<string>();
        foreach (var (y, _, _, text, conf) in textLines)
        {
            var t = _cleaner.Clean(text);
            if (y >= gy[0] && y <= gy[1] && conf > 0.1f && t.Length > 1 && t != "300")
                gradeLine.Add(t);
        }
        gradeLine = gradeLine.Distinct().ToList();

        // 2. Grade letter — detect it from the grade line, not by scanning every raw
        //    glyph in the band. The band also contains the "GRADE" label, whose own "D"
        //    made the old per-glyph D/G scan read a genuine "G" item as "DG".
        string? grade = DetectGradeFromLine(gradeLine);

        // 3. Color fallback (always computed for logging).
        var colorScores = DetectGradeColorScores(mat, ga);
        if (grade == null) grade = DecideGrade(colorScores);

        // 4. Attributes (X+Y-filtered + row bucketing).
        var rows = new Dictionary<int, List<string>>();
        foreach (var (y, _, _, text, conf) in attrTextLines)
        {
            var t = _cleaner.Clean(text);
            if (conf < 0.3f || t.Length < 2) continue;
            int rowKey = rowHeight > 0 ? y / rowHeight : 0;
            if (!rows.TryGetValue(rowKey, out var list)) { list = new List<string>(); rows[rowKey] = list; }
            if (!list.Contains(t)) list.Add(t);
        }

        var attrLines = new List<(int Y, List<string> Labels, List<string> Values)>();
        foreach (var rk in rows.Keys.OrderBy(k => k))
        {
            var labels = new List<string>();
            var values = new List<string>();
            foreach (var s in rows[rk].Select(t => t.Trim()).Where(s => s.Length > 0))
            {
                bool hasCh = s.Any(c => c >= '一' && c <= '鿿');
                bool hasSign = s.Contains('+') || s.Contains('-');
                var stripped = s.Replace(",", "").Replace(".", "").TrimStart('+', '-');
                bool isNum = stripped.Length > 0 && stripped.All(char.IsDigit);
                if (hasCh) labels.Add(s);
                else if (hasSign || (isNum && s.Length <= 3)) values.Add(s);
                else labels.Add(s);
            }
            attrLines.Add((rk * rowHeight, labels, values));
        }

        var lines = attrLines.Where(a => a.Y >= 10).Take(3).ToList();
        var attributes = new List<List<string>>();
        for (int i = 0; i < 3; i++)
            attributes.Add(i < lines.Count ? FixSign(lines[i].Labels, lines[i].Values) : new List<string>());

        // 5. Remaining spring count — X+Y-filtered, regex so it works with a glued label.
        int? remaining = null;
        foreach (var (y, _, _, text, conf) in remainingTextLines)
        {
            var t = text.Trim();
            if (conf <= 0.4f) continue;
            var m = Regex.Match(t, @"\d+");
            if (m.Success && int.TryParse(m.Value, out var n) && n >= 1 && n <= 99999)
            {
                remaining = n;
                break;
            }
        }

        var result = new ScanResult
        {
            Grade = grade,
            ColorScores = colorScores,
            Remaining = remaining,
            GradeLine = gradeLine,
            Attributes = attributes,
            CapturePath = capturePath,
            Window = client,
            AttrLineSpacing = MeasureLineSpacing(FilterItems(items, ax, at)),
        };

        _ocrLog.WriteLine(JsonSerializer.Serialize(new
        {
            timestamp,
            grade,
            remaining,
            gradeLine,
            attributes,
            colorScores,
            client = new { left = client.Left, top = client.Top, w = client.Width, h = client.Height },
            region = new { left = region.Left, top = region.Top, w = region.Width, h = region.Height },
            dpi = WindowFinder.GetDpi(hwnd),
            lines = textLines.Select(t => new { y = t.y, x = t.minX, x2 = t.maxX, text = t.text, conf = t.conf }),
        }));

        return result;
    }

    private List<string> FixSign(List<string> labels, List<string> values)
    {
        bool isNeg = labels.Any(l => _negativeMarkers.Any(n => l.Contains(n)));
        var result = new List<string>();
        foreach (var v in values)
        {
            var vv = v.Trim().TrimStart('+');
            if (isNeg && !vv.StartsWith('-')) vv = "-" + vv;
            else if (!isNeg && vv.All(char.IsDigit)) vv = "+" + vv;
            result.Add(vv);
        }
        return labels.Concat(result).ToList();
    }

    private static int ItemY(DetBoxItem it) =>
        it.Box is { Length: > 0 } ? (int)it.Box.Min(p => p.Y) : 0;

    private static int ItemX(DetBoxItem it) =>
        it.Box is { Length: > 0 } ? (int)it.Box.Min(p => p.X) : 0;

    private static int ItemMaxX(DetBoxItem it) =>
        it.Box is { Length: > 0 } ? (int)it.Box.Max(p => p.X) : 0;

    // Detect the grade letter from the reconstructed grade line. The line reads
    // "<label>：<letter>" (e.g. "GRADE：G"), and the label's own "D"/"G" must not be
    // mistaken for a grade — so we cut at the last separator (：/:) or strip a leading
    // "GRADE"/等级 label, then return the rightmost longest known grade token.
    // Returns null when no known grade token is present (caller falls back to color).
    private static string? DetectGradeFromLine(List<string> gradeLine)
    {
        var text = string.Concat(gradeLine).ToUpperInvariant();
        var sep = Math.Max(text.LastIndexOf('：'), text.LastIndexOf(':'));
        if (sep >= 0)
        {
            text = text[(sep + 1)..];
        }
        else
        {
            foreach (var label in new[] { "GRADE", "等级" })
                if (text.StartsWith(label, StringComparison.Ordinal)) { text = text[label.Length..]; break; }
        }

        // Longest-first so "DG"/"XG"/"SG" win over a bare "G" nested inside them;
        // rightmost wins so the letter after the label is chosen.
        string[] grades = { "SG", "XG", "DG", "G", "N" };
        string best = "";
        var bestIdx = -1; var bestLen = 0;
        foreach (var g in grades)
        {
            int start = 0;
            while ((start = text.IndexOf(g, start, StringComparison.Ordinal)) >= 0)
            {
                if (start > bestIdx || (start == bestIdx && g.Length > bestLen))
                {
                    best = g; bestIdx = start; bestLen = g.Length;
                }
                start += g.Length;
            }
        }
        return best.Length > 0 ? best : null;
    }

    private Dictionary<string, int> DetectGradeColorScores(Mat img, BoxConfig ga)
    {
        using var crop = img[new Rect(ga.X1, ga.Y1, ga.X2 - ga.X1, ga.Y2 - ga.Y1)];
        var gc = _cfg.Tuner.GradeColors;
        int yellow = 0, blue = 0, red = 0, purple = 0, white = 0;

        using var typed = new Mat<Vec3b>(crop);
        var idx = typed.GetIndexer();
        for (int y = 0; y < crop.Height; y++)
            for (int x = 0; x < crop.Width; x++)
            {
                var px = idx[y, x];
                int B = px.Item0, G = px.Item1, R = px.Item2;

                bool mYellow = R > G + gc.Yellow.RgGap && G > B + gc.Yellow.GbGap && R > gc.Yellow.RMin && G > gc.Yellow.GMin;
                bool mBlue = B > G + gc.Blue.BgGap && B > R + gc.Blue.BrGap && B > gc.Blue.BMin;
                bool mRed = R > G + gc.Red.RgGap && R > B + gc.Red.RbGap && R > gc.Red.RMin;
                bool mPurple = R > B + gc.Purple.RbGap && B > G + gc.Purple.BgGap && R > gc.Purple.RMin && B > gc.Purple.BMin;
                bool mWhite = R > gc.White.RMin && G > gc.White.GMin && B > gc.White.BMin;

                bool anyColor = mYellow || mBlue || mRed || mPurple;
                if (mWhite && !anyColor) white++;
                if (mYellow) yellow++;
                if (mBlue) blue++;
                if (mRed) red++;
                if (mPurple) purple++;
            }

        return new Dictionary<string, int>
        {
            ["N"] = white,
            ["G"] = blue,
            ["DG"] = yellow,
            ["XG"] = red,
            ["SG"] = purple,
        };
    }

    private string? DecideGrade(Dictionary<string, int> scores)
    {
        var d = _cfg.Tuner.GradeColors.Decision;
        int white = scores["N"], blue = scores["G"], yellow = scores["DG"], red = scores["XG"], purple = scores["SG"];
        int total = white + yellow + blue + red + purple;
        if (total < d.TotalMin) return null;

        // Each grade letter is a single colour. The "GRADE" label is white and constant, so
        // the old white-ratio rule let the label veto a real letter (blue=119 < white*0.5=385)
        // and produced no colour grade. The letter is the ONLY coloured blob in the box, so
        // it wins by being the dominant colour above its absolute floor. A white "N" grade
        // has no coloured blob and falls through to the white branch.
        bool blueWin = blue > d.GBlue && blue >= yellow && blue >= red && blue >= purple;
        bool yellowWin = yellow > d.DgYellow && yellow >= blue && yellow >= red && yellow >= purple;
        bool redWin = red > d.XgRed && red >= blue && red >= yellow && red >= purple;
        bool purpleWin = purple > d.SgPurple && purple >= blue && purple >= yellow && purple >= red;

        if (blueWin) return "G";
        if (yellowWin) return "DG";
        if (redWin) return "XG";
        if (purpleWin) return "SG";
        if (white > d.NWhite && yellow < d.NYellowMax && blue < d.NBlueMax) return "N";
        return null;
    }

    // Group char-level OCR results into text lines (group by Y row, sort by X, concatenate).
    private static List<(int y, int minX, int maxX, string text, float conf)> BuildLines(DetBoxItem[] items, int rowHeight)
    {
        var map = new Dictionary<int, List<DetBoxItem>>();
        foreach (var it in items)
        {
            var y = ItemY(it);
            int rowKey = rowHeight > 0 ? y / rowHeight : 0;
            if (!map.TryGetValue(rowKey, out var list)) { list = new List<DetBoxItem>(); map[rowKey] = list; }
            list.Add(it);
        }

        var lines = new List<(int y, int minX, int maxX, string text, float conf)>();
        foreach (var kv in map.OrderBy(k => k.Key))
        {
            var sorted = kv.Value.OrderBy(it => it.Box is { Length: > 0 } ? it.Box.Min(p => p.X) : 0f);
            var text = string.Concat(sorted.Select(it => it.Word ?? "")).Trim();
            var conf = kv.Value.Average(it => it.Score);
            var minY = kv.Value.Min(it => ItemY(it));
            var minX = kv.Value.Min(it => ItemX(it));
            var maxX = kv.Value.Max(it => ItemMaxX(it));
            lines.Add((minY, minX, maxX, text, conf));
        }
        return lines;
    }

    // Keep only items whose Y falls in the band and whose X overlaps the X range
    // (X is optional for back-compat with older local.yaml that only had Y bands).
    private static DetBoxItem[] FilterItems(DetBoxItem[] items, List<int> xRange, List<int> yRange)
    {
        var hasX = xRange.Count >= 2;
        var result = new List<DetBoxItem>();
        foreach (var it in items)
        {
            var y = ItemY(it);
            if (y < yRange[0] || y > yRange[1]) continue;
            if (hasX && (ItemMaxX(it) < xRange[0] || ItemX(it) > xRange[1])) continue;
            result.Add(it);
        }
        return result.ToArray();
    }

    // Measure the vertical pitch between attribute rows from the raw detected items,
    // independent of the config row_height. Collapse tops that are a few px apart
    // (same physical line + descender jitter), then take the median gap between adjacent
    // line tops. Returns the pitch in pixels, or null when fewer than two lines exist.
    private static int? MeasureLineSpacing(DetBoxItem[] items)
    {
        var collaped = new List<int>();
        foreach (var y in items.Select(ItemY).OrderBy(y => y))
        {
            if (collaped.Count == 0 || y - collaped[^1] > 3) collaped.Add(y);
        }
        if (collaped.Count < 2) return null;

        var gaps = new List<int>();
        for (int i = 1; i < collaped.Count; i++)
            gaps.Add(collaped[i] - collaped[i - 1]);
        gaps.Sort();
        return gaps[gaps.Count / 2]; // median gap
    }

    public void Dispose()
    {
        _ocr?.Dispose();
        _ocr = null;
        _ocrLog.Dispose();
    }
}
