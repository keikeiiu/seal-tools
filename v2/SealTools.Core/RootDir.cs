using System;
using System.IO;

namespace SealTools.Core;

// Where the application's own files live: the folder that holds config/. Every data path in the launcher
// is resolved from here rather than from the executable's folder, because in a dev run the executable
// sits in bin\Debug and its configuration is several levels up — the log-path comment in MainWindow
// records what went wrong when the two were confused.
//
// The marker is a defaults file under EITHER of its two names. The tracked one (config/defaults.yaml) is
// what a checkout has and what every install had until the update stopped shipping it; the shipped
// template (config/defaults.yaml.example) is what an install has afterwards, and it is the file the zip
// carries so that extracting an update cannot cover the player's own defaults.yaml. Accepting both is
// what lets a folder that has taken an update still be found — and it is why the marker is a FILE check
// rather than a "read the config" check: this has to answer before anything has been loaded.
public static class RootDir
{
    /// <summary>The shipped template first: an install that has taken an update is found by it even when
    /// the player has since deleted their own defaults.yaml, and a checkout without one is unaffected
    /// because the second name covers it.</summary>
    private static readonly string[] DefaultsMarkers = { "defaults.yaml.example", "defaults.yaml" };

    /// <summary>The application root, found by walking up from the executable.</summary>
    public static string Find() => Find(AppContext.BaseDirectory);

    /// <summary>The application root at or above <paramref name="startDir"/>. Throws when there is none,
    /// which is the existing behaviour and the right one: a folder with no configuration is not something
    /// to guess at.</summary>
    public static string Find(string startDir)
    {
        for (var dir = new DirectoryInfo(startDir); dir != null; dir = dir.Parent)
        {
            if (IsRoot(dir.FullName)) return dir.FullName;
        }

        throw new InvalidOperationException(
            $"Could not locate the application root — no config/defaults.yaml or " +
            $"config/defaults.yaml.example at or above {startDir}.");
    }

    /// <summary>True when <paramref name="dir"/> holds a config folder carrying either defaults marker.
    /// Shared with the loader's own rule rather than duplicated, so "is this the root?" has one answer.</summary>
    public static bool IsRoot(string dir)
    {
        foreach (var marker in DefaultsMarkers)
        {
            if (File.Exists(Path.Combine(dir, "config", marker))) return true;
        }

        return false;
    }
}
