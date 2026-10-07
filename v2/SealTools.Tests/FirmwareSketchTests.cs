using System;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;
using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// THE FIRMWARE'S PROTOCOL LEVEL IS A PROMISE, AND NOTHING WAS KEEPING IT.
//
// FW_VERSION is a hand-maintained #define in the sketch, and all of FirmwareVersion.Summary rests on it:
// a board reporting 2 is promised to have L/l, and the launcher now tells the player exactly that, in
// words, as a verdict. Add a command without bumping the number and the launcher starts describing a
// board that is not there — the failure this repo keeps paying for, in the one place a player has no way
// to check for themselves.
//
// So the sketch's LEVEL and its COMMAND SET are pinned together, here, against the real file. Change
// either one alone and this goes red. The fix is never to edit the expectation quietly:
//
//   1. Does the change alter the board's BEHAVIOUR? (The sketch's own rule for when to bump — a build
//      counter would tell the launcher nothing it could act on.) If yes, bump FW_VERSION in the sketch
//      and extend the level table in its comment.
//   2. Does the host need to know? If the new level changes what the launcher may DO — as level 2 did
//      for dragging — check FirmwareVersion.Current / DragLevel still say the truth.
//   3. Tag it, in the same commit: `git tag -a fw3 <sha> -m "fw3 — ..."`, matching fw1 and fw2. The tag
//      is the RECORD of what a board could do at each level, and it enforces nothing — this test is
//      what makes the bump impossible to forget.
//   4. Then update the two pins below, in the same commit as the firmware change.
//
// WHAT THIS CANNOT SEE, said plainly: it pins the commands, not the behaviour. A timing constant or a
// changed failsafe alters the board without touching the command set, and this test will not notice.
// The rule is still the author's to keep; this only makes the mechanical half impossible to forget.
public class FirmwareSketchTests
{
    private const int PinnedLevel = 2;

    /// <summary>Every command the sketch dispatches, ordered — 20 of them at level 2.</summary>
    private const string PinnedCommands = "CDEFHKLPQRSTUVWXZfkl";

    /// <summary>The sketch, found by walking up from the test binary rather than by a relative path. A
    /// test that silently passed because it could not find the file would be worse than no test.</summary>
    private static string SketchPath()
    {
        for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir is not null; dir = dir.Parent)
        {
            var candidate = Path.Combine(dir.FullName, "arduino", "seal_mouse", "seal_mouse.ino");
            if (File.Exists(candidate)) return candidate;
        }

        throw new FileNotFoundException(
            "arduino/seal_mouse/seal_mouse.ino was not found by walking up from " + AppContext.BaseDirectory);
    }

    private static string Sketch() => File.ReadAllText(SketchPath());

    private static int SketchLevel(string src) =>
        int.Parse(Regex.Match(src, @"#define\s+FW_VERSION\s+(\d+)").Groups[1].Value,
            System.Globalization.CultureInfo.InvariantCulture);

    private static string SketchCommands(string src) =>
        string.Concat(Regex.Matches(src, @"type\s*==\s*'([A-Za-z])'")
            .Select(m => m.Groups[1].Value)
            .Distinct()
            .OrderBy(c => c, StringComparer.Ordinal));

    [Fact]
    public void TheLevelAndTheCommandSetArePinnedTogether()
    {
        // Both halves in one test on purpose: the failure worth catching is one changing WITHOUT the
        // other, and two separate tests would each stay green in exactly that case.
        var src = Sketch();

        Assert.Equal(PinnedLevel, SketchLevel(src));
        Assert.Equal(PinnedCommands, SketchCommands(src));
    }

    [Fact]
    public void EveryCommandIsStillDispatchedInTheShapeThisTestCanSee()
    {
        // The pinning above only means anything while every command is guarded by `type == 'X'`. A
        // switch or a lookup table would slip past it and this test would go quiet without going red —
        // so the shape it depends on is asserted too.
        var src = Sketch();

        Assert.False(Regex.IsMatch(src, @"switch\s*\(\s*type"),
            "commands moved into a switch — the pinning above can no longer see them");
        Assert.Contains("type == 'V'", src);
    }

    [Fact]
    public void TheHostsDragLevelNamesALevelTheSketchActuallyReports()
    {
        // The card tells the player their board can drag food based on DragLevel. A level the sketch has
        // never reported would make that a guess dressed as a verdict.
        var src = Sketch();

        Assert.True(FirmwareVersion.DragLevel <= SketchLevel(src),
            "the host claims dragging arrives at a level the sketch has never reported");
        Assert.Contains("'L'", src);
        Assert.Contains("'l'", src);
    }

    [Fact]
    public void TheSketchIsWhereThisTestThinksItIs()
    {
        // The lookup is the part that can rot silently — a moved or renamed sketch would fail every
        // assertion above for the wrong reason, and the message would point at the firmware.
        var path = SketchPath();

        Assert.True(File.Exists(path));
        Assert.EndsWith("seal_mouse.ino", path);
    }
}
