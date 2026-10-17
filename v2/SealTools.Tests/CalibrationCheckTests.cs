using System;
using SealTools.Core;
using SealTools.Core.Config;
using Xunit;

namespace SealTools.Tests;

/// <summary>The Setup tab's comparison — what differs between the machine a calibration was made on and
/// the machine in front of the player. Pure, so it is tested here rather than through the tab: the
/// launcher is net8.0-windows and stays out of this project.
///
/// The three fields are the ones that silently invalidate every stored coordinate at once, and the old
/// line on that tab compared only the client size — so a changed monitor scale, which moves every click
/// just as completely, showed nothing at all.</summary>
public class CalibrationCheckTests
{
    private static DisplayInfo Display(double scale = 1.5, int screenW = 3840, int screenH = 2160,
        int clientW = 2865, int clientH = 1789) =>
        new(scale, (uint)Math.Round(96 * scale), screenW, screenH,
            new WindowRect(100, 100, clientW + 16, clientH + 39),
            new WindowRect(108, 131, clientW, clientH),
            new WindowRect(72, 87, clientW, clientH));

    private static CalibrationInfo Stored(double? scale = 1.5, int? clientW = 2865, int? clientH = 1789,
        int? screenW = 3840, int? screenH = 2160) => new()
    {
        DpiScale = scale,
        MonitorDpi = scale is null ? null : (uint)Math.Round(96 * scale.Value),
        ClientSize = clientW is null || clientH is null ? null : new List<int> { clientW.Value, clientH.Value },
        Screen = screenW is null || screenH is null ? null : new List<int> { screenW.Value, screenH.Value },
    };

    [Fact]
    public void AMatchingMachineReportsNothing()
    {
        Assert.Empty(CalibrationCheck.Differences(Stored(), Display()));
    }

    [Fact]
    public void AScaleThatOnlyDiffersInTheLastDigitsIsNotAMismatch()
    {
        // The stored scale is rounded to four places on save, so a float that came back a hair off is
        // the same scale rather than something to warn about.
        Assert.Empty(CalibrationCheck.Differences(Stored(scale: 1.5), Display(scale: 1.50003)));
    }

    [Fact]
    public void ADifferentScaleIsNamed()
    {
        var only = Assert.Single(CalibrationCheck.Differences(Stored(scale: 1.5), Display(scale: 1.25)));

        Assert.Contains("scale", only, StringComparison.Ordinal);
        Assert.Contains("1.5", only, StringComparison.Ordinal);
        Assert.Contains("1.25", only, StringComparison.Ordinal);
    }

    [Fact]
    public void ADifferentClientSizeIsNamed()
    {
        var only = Assert.Single(CalibrationCheck.Differences(Stored(), Display(clientW: 1920, clientH: 1080)));

        Assert.Contains("client 2865×1789", only, StringComparison.Ordinal);
        Assert.Contains("1920×1080", only, StringComparison.Ordinal);
    }

    [Fact]
    public void ADifferentScreenIsNamed()
    {
        var only = Assert.Single(CalibrationCheck.Differences(Stored(), Display(screenW: 2560, screenH: 1440)));

        Assert.Contains("screen 3840×2160", only, StringComparison.Ordinal);
        Assert.Contains("2560×1440", only, StringComparison.Ordinal);
    }

    [Fact]
    public void EveryMovedFieldIsReportedTogether()
    {
        var diffs = CalibrationCheck.Differences(
            Stored(), Display(scale: 1.25, screenW: 2560, screenH: 1440, clientW: 1920, clientH: 1080));

        Assert.Equal(3, diffs.Count);
    }

    [Fact]
    public void WhatTheStoredCalibrationDoesNotCarryIsNotAMismatch()
    {
        // An install that has never been calibrated has nothing to be wrong about — the tab says
        // "nothing stored yet" separately, and a field the file does not carry must not read as one that
        // moved.
        var neverCalibrated = Stored(scale: null, clientW: null, clientH: null, screenW: null, screenH: null);

        Assert.Empty(CalibrationCheck.Differences(neverCalibrated, Display()));
    }
}
