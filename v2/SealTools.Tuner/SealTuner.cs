using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.IO.Ports;
using System.Text.Json;
using System.Threading;
using SealTools.Core;
using SealTools.Core.Config;

namespace SealTools.Tuner;

// Port of tuner/seal_tuner.py main loop. Control is in-memory (CancellationToken),
// state is a shared ToolState object; Console.WriteLine is only for logs.

public sealed class SealTuner
{
    private readonly AppConfig _cfg;
    private readonly AttributesConfig _attrs;
    private readonly string _rootDir;
    private readonly AttrMatcher _matcher;
    private bool _quitPressed;
    private bool _pauseRequested;

    public SealTuner(AppConfig cfg, AttributesConfig attrs, string rootDir)
    {
        _cfg = cfg;
        _attrs = attrs;
        _rootDir = rootDir;
        _matcher = new AttrMatcher(attrs);
    }

    public int Run(ToolState state, CancellationToken ct)
    {
        var logDir = Path.Combine(_rootDir, "logs");
        Directory.CreateDirectory(logDir);
        var timestamp = DateTime.Now.ToString("yyyyMMdd_HHmmss", CultureInfo.InvariantCulture);
        using var runJson = new FileLogger(Path.Combine(logDir, $"run_{timestamp}.jsonl"));
        using var runTxt = new FileLogger(Path.Combine(logDir, $"run_{timestamp}.txt"));

        var port = Arduino.Find(_cfg.Arduino.Vid, _cfg.Arduino.Pid);
        if (port == null)
        {
            Console.WriteLine("[!] Arduino not found");
            return 1;
        }

        using var ser = Arduino.Open(port, _cfg.Arduino.Baud);
        Thread.Sleep(2000);
        Console.WriteLine($"[OK] Arduino on {port}");

        using var ocr = new OcrEngine(_cfg, _attrs, _rootDir);

        Console.WriteLine($"Target: {_cfg.Tuner.TargetGrade}");
        Console.WriteLine("Controls: [F12] start/stop  [F11] quit");

        bool running = false;
        int countdown = 0;
        int attempt = 0;
        bool f12Was = Hotkeys.IsDown(_cfg.Hotkeys.Start);
        var prevSig = (Grade: (string?)null, Remaining: (int?)null, Attrs: "");
        int sameCount = 0;

        try
        {
            while (true)
            {
                SleepCheck(0.05);
                if (_quitPressed || ct.IsCancellationRequested) break;

                // Sync with the launcher's in-memory start/stop signal.
                if (state.Running && !running)
                {
                    running = true;
                    countdown = 5;
                    Console.WriteLine("[Panel] START");
                    // No beep here — the panel already gives visual feedback;
                    // the 1500 Hz beep sounds when the countdown finishes (matches v1).
                }
                else if (!state.Running && running)
                {
                    running = false;
                    Console.WriteLine("[Panel] STOP");
                    Beep(1000, 150);
                }

                bool f12Now = Hotkeys.IsDown(_cfg.Hotkeys.Start);
                if (f12Now && !f12Was && !_quitPressed)
                {
                    running = !running;
                    state.Running = running;
                    if (running) { Console.WriteLine("[GO]"); countdown = 5; Beep(523, 100); }
                    else { Console.WriteLine("[STOP]"); countdown = 0; Beep(1000, 150); }
                }
                f12Was = f12Now;

                if (countdown > 0)
                {
                    Console.WriteLine($"  {countdown}...");
                    for (int i = 0; i < 20; i++)
                    {
                        SleepCheck(0.05);
                        if (Hotkeys.IsDown(_cfg.Hotkeys.Start)) { running = false; state.Running = false; countdown = 0; break; }
                        if (_quitPressed || ct.IsCancellationRequested) break;
                    }
                    countdown--;
                    if (countdown == 0) { Console.WriteLine("[>] RUNNING"); Beep(1500, 150); }
                    continue;
                }

                if (!running) continue;

                attempt++;
                state.Attempt = attempt;
                var timing = _cfg.Tuner.Timing;

                // Click + Enter (Arduino C/E commands).
                try
                {
                    ser.Write("C\n");
                    SleepCheck(timing.ClickEnterDelay);
                    if (_quitPressed || ct.IsCancellationRequested) break;
                    ser.Write("E\n");
                    SleepCheck(timing.OcrDelay);
                }
                catch (Exception)
                {
                    Console.WriteLine("[!] Arduino disconnected — stopping");
                    running = false;
                    state.Running = false;
                    break;
                }

                if (_quitPressed || ct.IsCancellationRequested) break;

                var result = ocr.Scan();
                string? grade = result?.Grade;
                int? remaining = result?.Remaining;
                var matched = result != null ? _matcher.MatchAttributes(result.Attributes) : new List<MatchedAttr>();
                var filter = _cfg.Tuner.Filter;
                var filterResult = AttrMatcher.CheckFilter(matched, filter);
                bool filterPass = filterResult.Passed;

                // Persist the attempt to the run log (for later review).
                runJson.WriteLine(JsonSerializer.Serialize(new
                {
                    attempt,
                    grade,
                    remaining,
                    attributes = matched.Select(m => new { m.Name, m.Value }),
                }));
                runTxt.WriteLine($"{attempt:000} {grade ?? "?"} {(remaining.HasValue ? remaining.Value.ToString(CultureInfo.InvariantCulture) : "?")} | " +
                    string.Join(" | ", matched.Select(m => $"{m.Name}={m.Value}")));

                var sig = (grade, remaining, string.Join("|", matched.Select(m => m.Name + "=" + m.Value)));
                if (sig == prevSig) sameCount++;
                else sameCount = 0;
                prevSig = sig;

                Console.WriteLine($"[{attempt:0000}] Grade: {grade ?? "?"}  Remaining: {(remaining.HasValue ? remaining.Value.ToString(CultureInfo.InvariantCulture) : "?")}");
                foreach (var m in matched)
                    Console.WriteLine($"    {m.Name} = {m.Value}");
                if (filter.Enabled)
                    Console.WriteLine($"    Filter: {(filterPass ? "MATCH" : "no match")} ({filterResult.Reason})");

                // Update the in-memory state (source of truth for the panel).
                state.Grade = grade;
                state.Remaining = remaining;
                state.Attributes = matched.Select(m => m.Name + "=" + m.Value).ToList();
                state.FilterStatus = filter.Enabled ? filterResult.Reason : "disabled";

                SleepCheck(0.3);
                if (_quitPressed || ct.IsCancellationRequested) break;

                // Stop conditions.
                string target = _cfg.Tuner.TargetGrade;
                string? requireGrade = filter.RequireGrade;
                string effectiveGrade = (!string.IsNullOrWhiteSpace(requireGrade) && !requireGrade.Equals("false", StringComparison.OrdinalIgnoreCase))
                    ? requireGrade : target;
                bool filterOk = filter.Enabled ? filterPass : true;
                bool gradeOk = grade != null && GradeIndex(grade) >= GradeIndex(effectiveGrade);

                if (gradeOk && filterOk)
                {
                    Console.WriteLine($">>> {grade} REACHED {(filter.Enabled ? "+ FILTER MATCH" : "(no filter)")} <<<");
                    running = false; state.Running = false;
                    BeepMany();
                    break;
                }
                else if (requireGrade == null && filterPass)
                {
                    Console.WriteLine($">>> FILTER MATCHED at grade {grade} (no grade requirement) <<<");
                    running = false; state.Running = false;
                    BeepMany();
                    break;
                }
                else if (gradeOk && !filterPass)
                    Console.WriteLine($">>> {grade} reached but filter not met ({filterResult.Reason}) — continuing <<<");
                else if (filterPass && !gradeOk && requireGrade != null)
                    Console.WriteLine($">>> Filter matched but grade {grade} < {requireGrade} — continuing <<<");

                if (remaining is <= 0)
                {
                    Console.WriteLine(">>> OUT OF SPRINGS <<<");
                    running = false; state.Running = false;
                    break;
                }
                if (sameCount >= 2)
                {
                    Console.WriteLine(">>> STUCK / OUT OF SPRINGS (same result x3) <<<");
                    running = false; state.Running = false;
                    break;
                }
                if (attempt >= _cfg.Tuner.MaxRetries)
                {
                    Console.WriteLine($"[!] Max {_cfg.Tuner.MaxRetries}");
                    running = false; state.Running = false;
                    break;
                }
                if (_pauseRequested)
                {
                    Console.WriteLine("[PAUSE] graceful stop");
                    running = false; state.Running = false;
                    _pauseRequested = false;
                    break;
                }
            }
        }
        finally
        {
            state.Running = false;
        }

        Console.WriteLine($"Done. {attempt} attempts.");
        return 0;
    }

    private int GradeIndex(string? grade) =>
        grade == null ? -1 : _cfg.Tuner.GradeOrder.IndexOf(grade);

    private void SleepCheck(double seconds)
    {
        var steps = Math.Max(1, (int)(seconds / 0.05));
        var ms = Math.Max(1, (int)(seconds / steps * 1000));
        for (int i = 0; i < steps; i++)
        {
            Thread.Sleep(ms);
            if (Hotkeys.IsDown(_cfg.Hotkeys.Quit)) { _quitPressed = true; return; }
            if (Hotkeys.IsDown(_cfg.Hotkeys.Pause)) { _pauseRequested = true; }
        }
    }

    private static void Beep(int freq, int ms)
    {
        try { Console.Beep(freq, ms); } catch { }
    }

    private static void BeepMany()
    {
        for (int i = 0; i < 5; i++) Beep(1200, 200);
    }
}
