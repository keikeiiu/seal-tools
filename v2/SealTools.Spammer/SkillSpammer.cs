using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.IO.Ports;
using System.Threading;
using SealTools.Core;
using SealTools.Core.Config;

namespace SealTools.Spammer;

// Port of skill_spammer/skill_spammer.py. Control in-memory (CancellationToken),
// state in a shared ToolState; Console.WriteLine is only for logs.

public sealed class SkillSpammer : ToolBase
{
    private readonly AppConfig _cfg;

    /// <summary>The game claim, or null when the tool was built without one (the tests, and any caller
    /// that does not run against a board). Null simply means this tool never yields.</summary>
    private readonly PortGate? _gate;

    /// <summary>The name this tool claims the game under — its tool id, so a card that says it is
    /// waiting for "spammer" names the thing the player started.</summary>
    private const string GateOwner = "spammer";

    public SkillSpammer(AppConfig cfg, PortGate? gate = null)
        : base(cfg.Hotkeys)
    {
        _cfg = cfg;
        _gate = gate;
    }

    public int Run(SerialPort ser, ToolState state, CancellationToken ct)
    {
        WatchCancellation(ct);
        var cooldowns = _cfg.Spammer.ActiveKeys;
        if (cooldowns.Count == 0)
        {
            // ActiveKeys no longer substitutes another preset, so an empty set means the active one
            // is missing (or none is configured). Say which, and don't run: pressing some other
            // rotation silently would be worse than pressing nothing.
            var msg = _cfg.Spammer.Presets.Count == 0
                ? "No spammer presets are configured — add one in the Spammer tab."
                : $"The active preset '{_cfg.Spammer.Active}' was not found — pick one in the Spammer tab.";
            Console.WriteLine("[!] " + msg);
            state.Message = msg;
            state.Running = false;
            return 0;
        }

        // ── the plan: combos first, then the keys that stand on their own ────────────────────────
        // A combo is an ordered group that must be cast together — the shape a game's combo effect
        // needs, where the second skill only lands while the first is still animating. Combos are
        // tried BEFORE any single key, and their members are removed from the single-key walk, so the
        // second half of a pair can never go out alone and quietly fail to trigger. SpammerOrder owns
        // both of those decisions; what is decided HERE is the preempt flag, because an empty priority
        // list cannot say whether it means "no rule" or "nothing outranks anything".
        var priority = _cfg.Spammer.ActivePriority;
        var combos = SpammerOrder.Runnable(cooldowns.Keys, _cfg.Spammer.ActiveCombos);
        var singles = SpammerOrder.Singles(cooldowns.Keys, priority, combos);
        bool preempt = priority.Count > 0;
        // How this preset's run OPENS: a lead-in before anything is pressed, then a gap between the
        // opening presses. Both zero — the default, and what an absent entry means — is exactly how
        // this behaved before either setting existed.
        var startup = _cfg.Spammer.ActiveStartup;
        // The stopwatch moment the opening begins. Everything before it is the lead-in, during which
        // nothing is pressed at all.
        double openingAt = 0;

        // The combo being cast, one step per gap. Non-empty means the spammer is MID-COMBO and the
        // loop presses nothing else until it drains — including the fillers, which is the point.
        var pending = new Queue<string>();
        double nextStepAt = 0;
        double runningGap = 0;

        bool running = false;
        int count = 0;
        string current = "";
        // True while the game has been handed to another tool. Nothing is pressed until it comes back,
        // but the loop keeps ticking so Stop and the hotkeys still work.
        bool yielded = false;
        var last = new Dictionary<string, double>();
        foreach (var k in cooldowns.Keys) last[k] = 0;

        bool f12Was = Hotkeys.IsDown(_cfg.Hotkeys.Start);
        var sw = Stopwatch.StartNew();

        Console.WriteLine("\nSkill Spammer");
        foreach (var c in combos)
            Console.WriteLine($"  combo: {string.Join(" → ", c.Keys)}  (gap {c.Gap:g}s, nothing else pressed between)");
        if (preempt)
            Console.WriteLine($"  order: {string.Join(" > ", singles)}  (the first one that is due wins; the rest wait)");
        foreach (var k in singles)
            Console.WriteLine($"  {k}: every {cooldowns[k]:g}s");
        Console.WriteLine("[F12] start/stop  [F11] quit\n");

        void Reset(bool payLeadIn = true)
        {
            // Everything starts DUE, not on cooldown. Start is a fresh run and the first thing a fresh
            // run does is cast; waiting each key's cooldown out first is right for a 6 s rotation and
            // wrong for a long buff, which on a short session would simply never go up at all.
            //
            // `startupStep` then SPACES those opening presses. Due all at once, a rotation with
            // animations fires its whole opening into itself and the game swallows all but the first —
            // so keys come due one step apart, in the same order the walk uses. At 0 they are all due
            // at once, which is how this behaved before the setting existed.
            //
            // Staggered by UNIT and not by key: a combo is one move, so its keys share an offset and
            // the combo's own gap does the spacing INSIDE it — stepping the halves too would space
            // them twice. `last[k] = at - cd` puts the key's due moment exactly at `at`.
            openingAt = sw.Elapsed.TotalSeconds + (payLeadIn ? startup.LeadIn : 0);

            // The lead-in is the one thing here with nothing to show for itself — the tool sits
            // pressing nothing, which is indistinguishable from a Start that did not take. So it is
            // reported through the card's SCHEDULE line, the same mechanism the pet feeder uses for a
            // reload: the tool sets the MOMENT and the card does the arithmetic, so it counts down
            // rather than showing a number that was true when it was written.
            if (payLeadIn && startup.LeadIn > 0)
            {
                state.Schedule = "lead-in — starting shortly";
                state.NextActionAt = DateTime.Now.AddSeconds(startup.LeadIn);
            }
            else
            {
                state.Schedule = null;
                state.NextActionAt = null;
            }

            int opening = 0;
            foreach (var combo in combos)
            {
                double at = openingAt + opening * startup.Step;
                foreach (var k in combo.Keys) last[k] = at - cooldowns[k];
                opening++;
            }
            foreach (var k in singles)
            {
                last[k] = openingAt + opening * startup.Step - cooldowns[k];
                opening++;
            }
            pending.Clear(); // a half-cast combo must not survive a stop/start
            // A fresh run is NOT yielded, and this is load-bearing rather than tidiness: the launcher
            // claims the gate on the tool's behalf at Start, so a stale `yielded` would have the tool
            // trying to re-acquire something it already holds — refused, every tick, forever, with the
            // rotated pressing nothing and saying only that it is paused.
            yielded = false;
            count = 0;
            current = "";
            state.Cycle = 0;
            state.Current = null;
        }

        // Sends one key and reports a dead port. Split out because the combo runner and the
        // single-key walk BOTH need it, and a second copy of stop-with-a-reason is where one of the
        // two would eventually stop without saying why.
        bool TrySend(string key, out bool dead)
        {
            dead = false;
            try
            {
                return SendKey(ser, key, state);
            }
            catch (Exception ex)
            {
                // The port is gone — unplugged, or the handle died. Nothing this tool does means
                // anything now, so stop with a reason on the card rather than let the exception
                // unwind to the launcher's generic handler.
                state.Message = $"Arduino disconnected — stopped ({ex.Message})";
                running = false;
                state.Running = false;
                dead = true;
                return false;
            }
        }

        // Do NOT touch state.Running here: LauncherService starts the tool with Running = true and
        // this loop picks that up to begin. Clearing it would leave the tool paused on Start.
        try
        {
            while (true)
            {
                SleepCheck(0.02);
                if (QuitPressed || ct.IsCancellationRequested) break;

                // Sync with the launcher's in-memory start/stop signal.
                if (state.Running && !running)
                {
                    running = true;
                    QuitPressed = false;
                    Reset();
                    Console.WriteLine("[Panel] START");
                    Beep(523, 100);
                }
                else if (!state.Running && running)
                {
                    running = false;
                    Console.WriteLine("[Panel] STOP");
                    Beep(1000, 150);
                }

                bool f12Now = Hotkeys.IsDown(_cfg.Hotkeys.Start);
                if (f12Now && !f12Was)
                {
                    running = !running;
                    state.Running = running;
                    if (running) { QuitPressed = false; Reset(); Console.WriteLine($"[GO] {string.Join(", ", cooldowns.Keys)}"); Beep(523, 100); }
                    else { Console.WriteLine("[STOP]"); Beep(1000, 150); }
                }
                f12Was = f12Now;

                // Checked HERE rather than after the work, because the yield below can skip the rest of
                // the tick with a continue — and a graceful stop that could be missed for as long as a
                // pet happens to be feeding is not a graceful stop.
                if (PauseRequested)
                {
                    Console.WriteLine("[PAUSE] graceful stop");
                    running = false; state.Running = false;
                    PauseRequested = false;
                    break;
                }

                if (!running) continue;

                // The lead-in is over once its moment has passed, and the countdown goes with it —
                // left up, the card would sit counting to a time already gone.
                if (state.NextActionAt != null && sw.Elapsed.TotalSeconds >= openingAt)
                {
                    state.Schedule = null;
                    state.NextActionAt = null;
                }

                // ── yielding the game to a waiter ────────────────────────────────────────────────
                // The pet feeder's reload is ~30 s and this run is unbounded, so without this the
                // feeder waits out the whole run and eventually runs dry. A claim can be refused but
                // never interrupted, so this is the holder choosing to stand down — and it stands down
                // only while someone is actually WAITING, which is what keeps a spammer running alone
                // from yielding to nobody.
                //
                // The pause lasts exactly as long as the waiter's action does: it ends when the gate
                // comes free, not on a timer, so a reload that takes 40 s is waited out and one that
                // takes 10 s is not waited 30.
                bool yieldedNow = yielded;
                if (yielded)
                {
                    if (_gate?.TryAcquire(GateOwner) == true)
                    {
                        yielded = false;
                        yieldedNow = false;
                        state.Message = null;
                        // Re-open through the rotation's own rules so a 30 s pause does not end with
                        // every expired cooldown firing at once. The LEAD-IN is deliberately not paid
                        // again — it exists to give you time to focus the game after Start, and the
                        // game is already in front by the time a reload finishes.
                        Reset(payLeadIn: false);
                        Console.WriteLine("[RESUME] the game is ours again");
                    }
                }
                else if (_gate is { } held && held.Owner == GateOwner && held.Waiting is { } waiter)
                {
                    held.Release(GateOwner);
                    yielded = true;
                    yieldedNow = true;
                    state.Message = $"paused — {waiter} has the game";
                    Console.WriteLine($"[YIELD] handed the game to {waiter}");
                }

                if (yieldedNow) continue; // nothing is pressed while the game is someone else's

                bool disconnected = false;

                // Start a combo when none is running. Readiness is "EVERY key in it is off cooldown",
                // which is what keeps the pair whole: starting a 1→2 that cannot finish would spend
                // skill 1, miss the effect, and say nothing about either.
                if (pending.Count == 0)
                {
                    double probe = sw.Elapsed.TotalSeconds;
                    foreach (var combo in combos)
                    {
                        if (!combo.Keys.All(k => probe - last[k] >= cooldowns[k])) continue;
                        runningGap = combo.Gap;
                        foreach (var k in combo.Keys) pending.Enqueue(k);
                        nextStepAt = probe; // the first step goes out now; each later one after a gap
                        break;
                    }
                }

                // A running combo OWNS the tick. Nothing else is pressed — no filler, no other combo
                // — until its last step has gone out. That silence is the point: the previous cast's
                // animation would swallow anything sent inside it, and a filler landing there would
                // break the very combo it was meant to fill around.
                bool comboRan = false;
                if (pending.Count > 0)
                {
                    comboRan = true;
                    if (sw.Elapsed.TotalSeconds >= nextStepAt)
                    {
                        var k = pending.Dequeue();
                        current = k;
                        // Each key's cooldown starts when IT was cast, not when the combo began —
                        // otherwise the second skill would come off cooldown a gap early, every cycle.
                        last[k] = sw.Elapsed.TotalSeconds;

                        bool sent = TrySend(k, out disconnected);
                        if (sent)
                        {
                            count++;
                            state.Current = k;
                            state.Cycle = count;
                        }
                        nextStepAt = sw.Elapsed.TotalSeconds + runningGap;
                    }
                }

                if (!comboRan)
                {
                    double now = sw.Elapsed.TotalSeconds;
                    foreach (var k in singles)
                    {
                        double cd = cooldowns[k];
                        if (now - last[k] < cd) continue;

                        current = k;
                        last[k] = now; // advance the cooldown either way, so an unusable key isn't retried every tick

                        // Only count a press that actually went out. SendKey bails on a key the
                        // firmware cannot send, and counting it made the card report a rising Cycle
                        // and a changing Current while nothing was being pressed at all.
                        // A DEAD PORT is the other failure and must leave the walk at once, or the
                        // remaining keys are each sent into a socket that is already gone.
                        if (!TrySend(k, out disconnected))
                        {
                            if (disconnected) break;
                            continue;
                        }

                        count++;
                        state.Current = k;
                        state.Cycle = count;

                        // PREEMPT — and only when the preset names an order. One press per tick, the
                        // rest held: a key that is due but outranked keeps its cooldown untouched and
                        // goes on a later tick, so it is DELAYED rather than skipped. That delay is
                        // what "the rotation goes first, the extras when we are able to" means.
                        //
                        // Opt-in per preset rather than always on, for the reason this rule is
                        // dangerous: a top key whose cooldown is at or under the 20 ms tick is due
                        // every tick, and everything below it then never fires at all. A preset with
                        // no order keeps sending every due key as it always has.
                        if (preempt) break;
                    }
                }

                if (disconnected) break;
            }
        }
        finally
        {
            state.Running = false;
        }

        Console.WriteLine("\nDone.");
        return 0;
    }

    private static readonly HashSet<string> WarnedKeys = new();

    /// <summary>Sends one key press. False when the key can't be sent at all — the caller must not
    /// count that as a press. A serial write failure is deliberately left to throw, so the caller
    /// stops the tool with a reason instead of pretending the press happened.</summary>
    private static bool SendKey(SerialPort ser, string key, ToolState state)
    {
        bool fast = key.StartsWith('*');
        if (fast) key = key.Substring(1);

        string cmd;
        if (key.StartsWith('F'))
        {
            // F1-F12: the firmware's table goes to twelve. ParseVk accepts F1-F24 elsewhere, so
            // anything above twelve is refused here rather than sent and silently ignored on board.
            if (!int.TryParse(key.AsSpan(1), NumberStyles.Integer, CultureInfo.InvariantCulture, out var f) || f is < 1 or > 12)
            {
                WarnUnsupported(key, state);
                return false;
            }
            cmd = (fast ? "f " : "F ") + f + "\n";
        }
        else if (key.Length == 1 && key[0] >= 0x20 && key[0] <= 0x7E)
        {
            // Any printable ASCII — the firmware's K handler presses one character, so letters work
            // as well as digits. Sending the character through unchanged is what makes "K 5" and
            // "K q" the same shape of command.
            cmd = (fast ? "k " : "K ") + key + "\n";
        }
        else
        {
            WarnUnsupported(key, state);
            return false;
        }
        ser.Write(cmd);
        return true;
    }

    // The firmware presses one printable character (K/k) or one function key up to F12. Anything
    // else is refused here rather than sent and dropped on the board. Warn once per bad key, on the
    // launcher card as well as the console (which the published WinExe doesn't have).
    private static void WarnUnsupported(string key, ToolState state)
    {
        if (WarnedKeys.Add(key))
        {
            var msg = $"Spammer key '{key}' unsupported — use one letter or digit, or F1–F12.";
            Console.WriteLine("[!] " + msg);
            state.Message = msg;
        }
    }

}
