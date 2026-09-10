using System;
using System.IO.Ports;
using System.Threading;
using OpenCvSharp;
using SealTools.Core;
using SealTools.Core.Config;

namespace SealTools.GemComposer;

// Port of gem_composer/gem_composer.py. Grade positions are client-area-relative
// (plan §3). Control is in-memory (CancellationToken), state is a shared ToolState.

public sealed class GemComposer : ToolBase
{
    // Per-channel difference above which a pixel counts as "not the empty box". The game's empty
    // result slot is static UI: a live crop of it measured pixel-identical to the saved reference
    // (0.000 at every threshold from 10 to 60), while a box holding a gem differs on ~36% of its
    // pixels by far more than 60. 30 sits in the empty middle of that gap.
    private const int EmptyDiffTolerance = 30;

    private readonly AppConfig _cfg;
    private readonly string _rootDir;
    private Mat? _emptyReference;
    private bool _emptyReferenceLoaded;

    public GemComposer(AppConfig cfg, string rootDir)
        : base(cfg.Hotkeys)
    {
        _cfg = cfg;
        _rootDir = rootDir;
    }

    // The empty result-box crop saved by the calibrator (config/calib_gem_result.png), loaded once.
    // Null when it isn't there — then the colour signature is used instead.
    private Mat? EmptyReference()
    {
        if (_emptyReferenceLoaded) return _emptyReference;
        _emptyReferenceLoaded = true;
        try
        {
            var path = Path.Combine(_rootDir, "config", "calib_gem_result.png");
            if (File.Exists(path)) _emptyReference = Cv2.ImRead(path, ImreadModes.Color);
        }
        catch { _emptyReference = null; }
        return _emptyReference;
    }

    public int Run(SerialPort ser, ToolState state, CancellationToken ct)
    {
        var grades = _cfg.Gem.Grades;
        int gidx = Math.Max(0, grades.IndexOf(_cfg.Gem.StartGrade));
        bool running = false;
        int cycle = 0;
        int emptyCount = 0;
        bool f12Was = Hotkeys.IsDown(_cfg.Hotkeys.Start);
        bool f9Was = Hotkeys.IsDown(_cfg.Hotkeys.AdvanceGrade);

        Console.WriteLine("\nGem Composer");
        Console.WriteLine("[F12] start/stop  [F9] advance grade  [F11] quit\n");

        state.Cycle = 0;
        state.Grade = grades[gidx];

        // Calibrated points/moves only exist once the user has saved them (Save Gem Composer writes
        // the positions, Save Composer Moves writes the movements). A missing entry used to throw
        // KeyNotFoundException / ArgumentOutOfRangeException, which stopped the tool with no
        // explanation on the card. Report it there and stop instead.
        void Fail(string msg)
        {
            Console.WriteLine("[!] " + msg);
            state.Message = msg;
            running = false;
            state.Running = false;
        }

        bool TryPoint(string what, IReadOnlyDictionary<string, List<int>> map, string key, out int x, out int y)
        {
            x = 0; y = 0;
            if (map.TryGetValue(key, out var p) && p is { Count: >= 2 }) { x = p[0]; y = p[1]; return true; }
            Fail($"{what} '{key}' isn't calibrated yet — open Calibrate Gem and save it.");
            return false;
        }

        bool TryMove(string what, List<int>? d, out int dx, out int dy)
        {
            dx = 0; dy = 0;
            if (d is { Count: >= 2 }) { dx = d[0]; dy = d[1]; return true; }
            Fail($"{what} isn't saved yet — open Calibrate Gem and press Save Composer Moves.");
            return false;
        }

        // Place the cursor on a calibrated POINT with the Arduino (closed loop) — the "arduino"
        // move set. The same call the calibrator's Test Click makes, so a move that lands in a test
        // lands here. Returns false (and stops the tool with a reason on the card) if the point
        // isn't calibrated or the cursor can't be placed.
        bool PlaceAt(string what, string pointName)
        {
            if (GemRoutes.Resolve(_cfg, pointName) is not { } p)
            {
                Fail($"{what}: '{pointName}' isn't calibrated yet — open Calibrate Gem and save it.");
                return false;
            }
            var display = GemPointer.Display(_cfg.Window.Title);
            if (display == null)
            {
                Fail("Game window not found (or minimized) — open and restore the game first.");
                return false;
            }
            var placed = GemPointer.To(ser, WindowFinder.ComputeCursorTarget(display, p.X, p.Y));
            if (!placed.Ok)
            {
                Fail($"{what}: couldn't move the cursor to {pointName} — {placed.Error}.");
                return false;
            }
            return true;
        }

        // One composer route move, honouring gem.move_mode: "tuned" sends the hand-tuned counts in
        // gem.movements, "arduino" places the cursor on the route's destination point instead (and
        // `tuned` is ignored). Both are one call so a route reads the same either way.
        bool Route(string what, string routeKey, List<int>? tuned)
        {
            if (_cfg.Gem.MoveMode == "arduino")
            {
                if (GemRoutes.Destination(routeKey) is not { } destination)
                {
                    Fail($"{what}: unknown route '{routeKey}'.");
                    return false;
                }
                return PlaceAt(what, destination);
            }

            if (!TryMove(what, tuned, out var dx, out var dy)) return false;
            GemPointer.Move(ser, dx, dy);
            return true;
        }

        void SelectGradeAndRegister()
        {
            var display = GemPointer.Display(_cfg.Window.Title);
            if (display == null)
            {
                Fail("Game window not found (or minimized) — open and restore the game first.");
                return;
            }

            var mv = _cfg.Gem.Movements;
            if (!TryPoint("Grade position", _cfg.Gem.GradePositions, grades[gidx], out var gx, out var gy)) return;

            // v1 sequence: select grade, move to Register, select. No focus handling — the single
            // click on the grade button does both jobs (activates the game window and presses the
            // button), so no separate click-to-focus is needed. Do NOT add a Win32 focus API or a
            // centre-click (a centre-click pins a raw-input game's cursor at centre).
            //
            // A cursor that can't be placed must NOT be clicked through: the click would land
            // wherever the pointer happens to be (docs/CURSOR-INVESTIGATION.md).
            var placed = GemPointer.To(ser, WindowFinder.ComputeCursorTarget(display, gx, gy));
            if (!placed.Ok)
            {
                Fail($"Couldn't move the cursor onto the {grades[gidx]} button — {placed.Error}. Stopped instead of clicking blind.");
                return;
            }
            SleepCheck(0.3);
            GemPointer.Click(ser);
            SleepCheck(0.5);
            if (!Route($"Move {grades[gidx]} → Register", $"radio_{grades[gidx]}",
                    mv.RadioToRegister.GetValueOrDefault(grades[gidx]))) return;
            SleepCheck(0.3);
            GemPointer.Click(ser);
            SleepCheck(0.5);
        }

        // True when the composed result-gem box is empty (no gem).
        //
        // Primary signal: the fraction of pixels that differ from the saved empty-box crop — this
        // is colour- and shape-blind, so any gem (red/green/blue, any grade's shape) reads the same.
        // Measured: empty 0.000, gem 0.357, threshold (gem.empty_distance) 0.18.
        //
        // Fallback when that crop is missing: the colour signature, which averages the whole box
        // and is therefore the weaker test (a gem colour close to the empty slot's can hide in it).
        // Returns false (not empty) when neither is available, so the composer never advances on a
        // missing reference.
        bool IsResultBoxEmpty()
        {
            if (_cfg.Gem.ResultGemArea is not { Count: 4 } area) return false;
            var hwnd = WindowFinder.FindByTitle(_cfg.Window.Title);
            var cap = ScreenCapture.CaptureClientRegion(hwnd, new RegionConfig { Left = area[0], Top = area[1], Width = area[2], Height = area[3] });
            if (cap == null) return false;
            using var crop = cap.Image;

            double? diff = null;
            if (EmptyReference() is { } reference)
                diff = GemColorAnalyzer.DiffFraction(crop, reference, EmptyDiffTolerance);

            bool empty;
            if (diff is { } fraction)
            {
                empty = fraction <= _cfg.Gem.EmptyDistance;
            }
            else if (_cfg.Gem.EmptySignature is { } sig)
            {
                empty = GemColorAnalyzer.IsEmpty(GemColorAnalyzer.Analyze(crop, _cfg.Gem.ColoredGapMin), sig, _cfg.Gem.EmptyDistance);
            }
            else
            {
                return false;
            }

            if (_cfg.Gem.SaveEmptyCaptures)
            {
                try
                {
                    var dir = Path.Combine(AppContext.BaseDirectory, "logs", "captures");
                    Directory.CreateDirectory(dir);
                    crop.ImWrite(Path.Combine(dir, $"empty_check_crop_{DateTime.Now:HHmmss_fff}.png"));
                }
                catch { }
                var detail = diff is { } f ? $"diff={f:0.000}" : "diff=n/a";
                try { File.AppendAllText(Path.Combine(AppContext.BaseDirectory, "logs", "empty_check.txt"), $"{DateTime.Now:HH:mm:ss} {detail} threshold={_cfg.Gem.EmptyDistance} empty={empty}\n"); } catch { }
            }
            return empty;
        }

        // Right-click the three resource slots to clear any stuck resource gems.
        bool ClearResources()
        {
            var mv = _cfg.Gem.Movements;
            if (!Route("Move Register → Resource1", "register_slot1", mv.RegisterSlot1)) return false;
            SleepCheck(0.2);
            GemPointer.RightClick(ser);
            SleepCheck(0.3);
            if (!Route("Move Resource1 → Resource2", "slot1_slot2", mv.Slot1Slot2)) return false;
            SleepCheck(0.2);
            GemPointer.RightClick(ser);
            SleepCheck(0.3);
            if (!Route("Move Resource2 → Resource3", "slot2_slot3", mv.Slot2Slot3)) return false;
            SleepCheck(0.2);
            GemPointer.RightClick(ser);
            SleepCheck(0.3);
            return true;
        }

        // Resource3 → grade movement for the given grade label (null when the grade isn't one of
        // the three configured gem grades, so the caller reports it instead of moving 0,0).
        List<int>? Slot3ToGrade(string grade) => grade switch
        {
            "N" => _cfg.Gem.Movements.Slot3ToN,
            "G" => _cfg.Gem.Movements.Slot3ToG,
            "DG" => _cfg.Gem.Movements.Slot3ToDg,
            _ => null,
        };

        // The route key for that same movement, so the arduino set can resolve its destination.
        static string? Slot3RouteKey(string grade) => grade switch
        {
            "N" => "slot3_n",
            "G" => "slot3_g",
            "DG" => "slot3_dg",
            _ => null,
        };

        // Advance to the next grade; in "advance_grade_clear" mode, clear the resource slots first.
        // Returns false when there is no next grade — a run is N -> G -> DG ONCE, not a loop, so the
        // composer ends instead of wrapping back to the first grade. Also false when a step failed
        // (the reason is already on the card).
        bool AdvanceGrade()
        {
            if (_cfg.Gem.EmptyMode == "advance_grade_clear" && !ClearResources())
                return false;

            if (gidx + 1 >= grades.Count)
            {
                Console.WriteLine($"[DONE] {grades[gidx]} was the last grade");
                state.Message = $"All grades done (last was {grades[gidx]}) — composer stopped.";
                running = false;
                state.Running = false;
                Beep(880, 200);
                return false;
            }

            gidx++;
            state.Grade = grades[gidx];
            Console.WriteLine($"[EMPTY] advancing -> {grades[gidx]}");

            if (_cfg.Gem.EmptyMode == "advance_grade_clear")
            {
                // Cursor is at Resource3 → move to the next grade and select it.
                if (Slot3RouteKey(grades[gidx]) is not { } slotKey)
                {
                    Fail($"No route Resource3 → {grades[gidx]}.");
                    return false;
                }
                if (!Route($"Move Resource3 → {grades[gidx]}", slotKey, Slot3ToGrade(grades[gidx]))) return false;
                SleepCheck(0.2);
                GemPointer.Click(ser);
                SleepCheck(0.5);
            }
            else
            {
                // Cursor is at Register → re-select the grade absolutely.
                var display = GemPointer.Display(_cfg.Window.Title);
                if (display == null)
                {
                    Fail("Game window not found (or minimized) — open and restore the game first.");
                    return false;
                }
                if (!TryPoint("Grade position", _cfg.Gem.GradePositions, grades[gidx], out var gx, out var gy)) return false;
                var placed = GemPointer.To(ser, WindowFinder.ComputeCursorTarget(display, gx, gy));
                if (!placed.Ok)
                {
                    Fail($"Couldn't move the cursor onto the {grades[gidx]} button — {placed.Error}. Stopped instead of clicking blind.");
                    return false;
                }
                SleepCheck(0.3);
                GemPointer.Click(ser);
                SleepCheck(0.5);
            }

            // Next grade → Register (radio_to_register) and select.
            if (!Route($"Move {grades[gidx]} → Register", $"radio_{grades[gidx]}",
                    _cfg.Gem.Movements.RadioToRegister.GetValueOrDefault(grades[gidx]))) return false;
            SleepCheck(0.3);
            GemPointer.Click(ser);
            SleepCheck(0.5);
            return true;
        }

        try
        {
            while (true)
            {
                SleepCheck(0.05);
                if (QuitPressed || ct.IsCancellationRequested) break;

                // Sync with the launcher's in-memory start/stop signal.
                if (state.Running && !running)
                {
                    running = true;
                    f12Was = true;
                    Console.WriteLine("[Panel] START");
                    Beep(523, 100);
                    SelectGradeAndRegister();
                }
                else if (!state.Running && running)
                {
                    running = false;
                    Console.WriteLine("[Panel] STOP");
                    Beep(1000, 150);
                }

                bool f12Now = Hotkeys.IsDown(_cfg.Hotkeys.Start);
                bool f9Now = Hotkeys.IsDown(_cfg.Hotkeys.AdvanceGrade);

                if (f12Now && !f12Was)
                {
                    running = !running;
                    state.Running = running;
                    if (running)
                    {
                        Console.WriteLine($"[GO] Grade: {grades[gidx]}");
                        Beep(523, 100);
                        SelectGradeAndRegister();
                    }
                    else { Console.WriteLine("[STOP]"); Beep(1000, 150); }
                }

                if (f9Now && !f9Was)
                {
                    gidx = (gidx + 1) % grades.Count;
                    Console.WriteLine($"[GRADE] -> {grades[gidx]}");
                    state.Grade = grades[gidx];
                    SelectGradeAndRegister();
                }

                f12Was = f12Now;
                f9Was = f9Now;

                if (!running) continue;

                cycle++;
                state.Cycle = cycle;

                if (QuitPressed || ct.IsCancellationRequested) break;
                if (Hotkeys.IsDown(_cfg.Hotkeys.Start))
                {
                    f12Was = true;
                    running = false;
                    state.Running = false;
                    Console.WriteLine("[STOP]");
                    Beep(1000, 150);
                    continue;
                }

                // Combine.
                if (!Route("Move Register → Combine", "register_combine", _cfg.Gem.Movements.RegisterCombine)) break;
                SleepCheck(0.2);
                GemPointer.Click(ser);
                SleepCheck(0.8);

                // Auto-advance when the result box is empty (no gem created).
                bool advanceNow = false;
                if (_cfg.Gem.EmptyMode != "stop" && _cfg.Gem.EmptySignature != null)
                {
                    if (IsResultBoxEmpty())
                    {
                        emptyCount++;
                        if (emptyCount >= _cfg.Gem.EmptyStreak)
                        {
                            emptyCount = 0;
                            advanceNow = true;
                        }
                    }
                    else emptyCount = 0;
                }

                // Move back to Register (the advance flow must start from the Register button).
                if (QuitPressed || ct.IsCancellationRequested) break;
                if (!Route("Move Combine → Register", "combine_register", _cfg.Gem.Movements.CombineRegister)) break;
                SleepCheck(0.2);

                if (advanceNow)
                {
                    if (!AdvanceGrade()) break;   // last grade done, or a step failed
                    continue;
                }

                // Normal: deregister + register.
                GemPointer.Click(ser);
                SleepCheck(0.3);
                GemPointer.Click(ser);
                SleepCheck(0.5);

                if (cycle % 10 == 0) Console.WriteLine($"  Cycle: {cycle}");
                if (PauseRequested)
                {
                    Console.WriteLine("[PAUSE] graceful stop");
                    running = false; state.Running = false;
                    PauseRequested = false;
                    break;
                }
            }
        }
        finally
        {
            state.Running = false;
        }

        Console.WriteLine($"\nDone. {cycle} cycles.");
        return 0;
    }
}
