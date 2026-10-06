using System.Collections.Generic;
using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// `arduino.port` was documented in local.yaml.example as an override and read by nothing. These pin the
// behaviour it now has — including the direction that matters, which is REFUSING to open a board the
// player did not name rather than quietly opening the one that was discovered.
public class PortChoiceTests
{
    private static readonly string[] Machine = { "COM3", "COM5", "COM9" };
    private static string? Discovered() => "COM9";

    [Fact]
    public void ANamedPortThatExistsIsUsed()
    {
        var (port, error) = PortChoice.Choose("COM5", Machine, Discovered);

        Assert.Equal("COM5", port);
        Assert.Null(error);
    }

    // The point of the whole method: a port the player named is not a hint. Opening the discovered one
    // instead would drive a board they explicitly did not choose, silently.
    [Fact]
    public void ANamedPortThatDoesNotExistIsAnErrorRatherThanAFallThrough()
    {
        var (port, error) = PortChoice.Choose("COM7", Machine, Discovered);

        Assert.Null(port);
        Assert.NotNull(error);
        Assert.Contains("COM7", error!);   // names the port, so the message says which one is wrong
    }

    // Windows reports COM3; a hand-written config says com3 as often as not.
    [Fact]
    public void ThePortNameIsMatchedWithoutCase()
    {
        var (port, error) = PortChoice.Choose("com5", Machine, Discovered);

        Assert.Equal("com5", port);
        Assert.Null(error);
    }

    [Fact]
    public void SurroundingSpaceIsNotPartOfTheName()
    {
        var (port, _) = PortChoice.Choose("  COM5  ", Machine, Discovered);

        Assert.Equal("COM5", port);
    }

    // Empty, whitespace and absent all mean the same thing: no override, so discover.
    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("   ")]
    public void NoConfiguredPortFallsBackToDiscovery(string? configured)
    {
        var (port, error) = PortChoice.Choose(configured, Machine, Discovered);

        Assert.Equal("COM9", port);
        Assert.Null(error);
    }

    // Nothing named and nothing found: no port, and NO error here — the caller writes that message,
    // because only it knows the configured VID and PIDs to name in it.
    [Fact]
    public void NothingNamedAndNothingFoundIsTheCallersToExplain()
    {
        var (port, error) = PortChoice.Choose("", Machine, () => null);

        Assert.Null(port);
        Assert.Null(error);
    }

    // A named port is honoured even on a machine where discovery would find nothing — which is the case
    // the override exists for: a board whose IDs are not reported properly.
    [Fact]
    public void ANamedPortIsUsedEvenWhenDiscoveryWouldFail()
    {
        var (port, error) = PortChoice.Choose("COM3", Machine, () => null);

        Assert.Equal("COM3", port);
        Assert.Null(error);
    }
}
