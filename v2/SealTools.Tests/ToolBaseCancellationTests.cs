using System;
using System.Diagnostics;
using System.Threading;
using SealTools.Core;
using SealTools.Core.Config;
using Xunit;

namespace SealTools.Tests;

// SleepCheck throwing on cancellation is what makes Stop work on the pet feeder. The loops already
// read the token, but a loop is only reached when the step it is running has finished — and a pet
// visit can be several rows of OCR and a ~30 s reload wait, so a Stop used to be honoured only at a
// visit boundary, which looks exactly like a broken button.
//
// The hotkey config is a plain object of ints, so the zero defaults make Hotkeys.IsDown read key 0,
// which is never down — these tests never depend on a real key being pressed.
public class ToolBaseCancellationTests
{
    private sealed class Probe : ToolBase
    {
        public Probe(bool ignoresQuitHotkey = false) : base(new HotkeysConfig(), ignoresQuitHotkey) { }

        public void Watch(CancellationToken ct) => WatchCancellation(ct);
        public void Sleep(double seconds) => SleepCheck(seconds);
    }

    [Fact]
    public void ASleepThrowsOnceTheRunIsCancelled()
    {
        var probe = new Probe();
        using var cts = new CancellationTokenSource();
        probe.Watch(cts.Token);
        cts.Cancel();

        Assert.Throws<OperationCanceledException>(() => probe.Sleep(1.0));
    }

    [Fact]
    public void CancellingMidSleepStopsItEarlyRatherThanAtTheEnd()
    {
        // The whole point: a long step is abandoned DURING it, not after it. Without this the pet's
        // ~30 s reload wait would run to completion before anything noticed the stop.
        var probe = new Probe();
        using var cts = new CancellationTokenSource();
        probe.Watch(cts.Token);

        var sw = Stopwatch.StartNew();
        cts.CancelAfter(120);
        Assert.Throws<OperationCanceledException>(() => probe.Sleep(5.0));
        sw.Stop();

        Assert.True(sw.ElapsedMilliseconds < 2000,
            $"a 5 s sleep should have been abandoned well before its end, took {sw.ElapsedMilliseconds}ms");
    }

    [Fact]
    public void TheToolThatIgnoresTheQuitHotkeyStillStopsOnACancellation()
    {
        // The pet feeder sets ignoresQuitHotkey so that ONE hotkey press cannot end a schedule that is
        // feeding four pets — the flag is about accidental presses, and it must not extend to the
        // player pressing Stop. Those are different signals and this pins that they stay different.
        var probe = new Probe(ignoresQuitHotkey: true);
        using var cts = new CancellationTokenSource();
        probe.Watch(cts.Token);
        cts.Cancel();

        Assert.Throws<OperationCanceledException>(() => probe.Sleep(0.05));
    }

    [Fact]
    public void AnUncancelledSleepStillReturnsNormally()
    {
        // The everyday path, unchanged. If this ever threw, every tool would abort on its first sleep.
        var probe = new Probe();
        using var cts = new CancellationTokenSource();
        probe.Watch(cts.Token);

        probe.Sleep(0.05);
    }

    [Fact]
    public void ASleepWithNoTokenWatchedDoesNotThrow()
    {
        // A tool that forgot WatchCancellation must not crash on every sleep. A CancellationToken's
        // default is the un-cancellable one, so the failure mode stays "the stop does not work" — loud
        // in testing — rather than "the tool dies on its first wait".
        var probe = new Probe();

        probe.Sleep(0.05);
    }

    [Fact]
    public void AShortCancelledSleepStillThrows()
    {
        // SleepCheck clamps to at least one step, so even a 0 s sleep must observe the token — a step
        // boundary that quietly skipped the check would be a stop that works everywhere except there.
        var probe = new Probe();
        using var cts = new CancellationTokenSource();
        probe.Watch(cts.Token);
        cts.Cancel();

        Assert.Throws<OperationCanceledException>(() => probe.Sleep(0));
    }
}
