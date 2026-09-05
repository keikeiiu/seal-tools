using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
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
        var capturePath = Path.Combine(_captureDir, $"capture_{timestamp}.png");

        var ocrResult = _ocr!.RecognizeText(mat, capturePath);
        var items = ocrResult.WordResults ?? Array.Empty<DetBoxItem>();

        var ga = ocr.GradeArea;
        var gy = ocr.GradeY;
        var at = ocr.AttrY;
        var ry = ocr.RemainingY;
        int rowHeight = ocr.RowHeight;

        // Reconstruct line-level results from char-level WordResults.
        var textLines = BuildLines(items, rowHeight);

        // 1. Grade — OCR the grade-letter crop first (more reliable for DG/G).
        string? grade = null;
        using (var gradeCrop = mat[new Rect(ga.X1, gy[0], ga.X2 - ga.X1, gy[1] - gy[0])])
        {
            var gRes = _ocr!.RecognizeText(gradeCrop, Path.Combine(_captureDir, $"grade_{timestamp}.png"));
            foreach (var it in gRes.WordResults ?? Array.Empty<DetBoxItem>())
            {
                var t = (it.Word ?? "").Trim().ToUpperInvariant();
                if (it.Score < 0.5f) continue;
                if (t is "DG" or "XG" or "SG") { grade = t; break; }
                if (t == "N") grade = "N";
                else if (t == "G") grade = "G";
                else if (t == "D" && grade == null) grade = "DG";
            }
        }

        // 2. Color fallback (always computed for logging).
        var colorScores = DetectGradeColorScores(mat, ga);
        if (grade == null) grade = DecideGrade(colorScores);

        // 3. Grade line (full OCR, Y-filtered).
        var gradeLine = new List<string>();
        foreach (var (y, text, conf) in textLines)
        {
            var t = _cleaner.Clean(text);
            if (y >= gy[0] && y <= gy[1] && conf > 0.1f && t.Length > 1 && t != "300")
                gradeLine.Add(t);
        }
        gradeLine = gradeLine.Distinct().ToList();

        // 4. Attributes (full OCR, Y-filtered + row bucketing).
        var rows = new Dictionary<int, List<string>>();
        foreach (var (y, text, conf) in textLines)
        {
            var t = _cleaner.Clean(text);
            if (conf < 0.3f || t.Length < 2 || y < at[0] || y > at[1]) continue;
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

        // 5. Remaining spring count.
        int? remaining = null;
        foreach (var (y, text, conf) in textLines)
        {
            var t = text.Trim();
            if (y >= ry[0] && y <= ry[1] && conf > 0.4f && t.Length <= 6)
            {
                foreach (var part in t.Split(' ').Reverse())
                {
                    if (part.Length > 0 && part.All(char.IsDigit) && int.TryParse(part, out var n) && n >= 1 && n <= 99999)
                    {
                        remaining = n;
                        break;
                    }
                }
                if (remaining != null) break;
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
        };

        _ocrLog.WriteLine(JsonSerializer.Serialize(new
        {
            timestamp,
            grade,
            remaining,
            gradeLine,
            attributes,
            colorScores,
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
        if (yellow > d.DgYellow && yellow > white * d.DgWhiteRatio) return "DG";
        if (blue > d.GBlue && blue > white * d.GWhiteRatio) return "G";
        if (red > d.XgRed) return "XG";
        if (purple > d.SgPurple) return "SG";
        if (white > d.NWhite && yellow < d.NYellowMax && blue < d.NBlueMax) return "N";
        return null;
    }

    // Group char-level OCR results into text lines (group by Y row, sort by X, concatenate).
    private static List<(int y, string text, float conf)> BuildLines(DetBoxItem[] items, int rowHeight)
    {
        var map = new Dictionary<int, List<DetBoxItem>>();
        foreach (var it in items)
        {
            var y = ItemY(it);
            int rowKey = rowHeight > 0 ? y / rowHeight : 0;
            if (!map.TryGetValue(rowKey, out var list)) { list = new List<DetBoxItem>(); map[rowKey] = list; }
            list.Add(it);
        }

        var lines = new List<(int y, string text, float conf)>();
        foreach (var kv in map.OrderBy(k => k.Key))
        {
            var sorted = kv.Value.OrderBy(it => it.Box is { Length: > 0 } ? it.Box.Min(p => p.X) : 0f);
            var text = string.Concat(sorted.Select(it => it.Word ?? "")).Trim();
            var conf = kv.Value.Average(it => it.Score);
            var minY = kv.Value.Min(it => ItemY(it));
            lines.Add((minY, text, conf));
        }
        return lines;
    }

    public void Dispose()
    {
        _ocr?.Dispose();
        _ocr = null;
        _ocrLog.Dispose();
    }
}
