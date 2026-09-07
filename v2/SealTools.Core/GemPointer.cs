using System.IO.Ports;

namespace SealTools.Core;

// Shared low-level "tool calls" for the Gem Composer (runtime loop) and the calibrate tab's test
// buttons (GemTestClick / GemTestRelativeMove). Both drive the same Arduino HID mouse and both need
// the game client rect. Centralising these two things means the test path and the runtime path send
// the EXACT same bytes and use the EXACT same window lookup — so a click that lands in a test lands
// in the composer too, and the paths can't drift apart. Callers keep their own sleeps in between so
// the composer's quit-aware SleepCheck (and its tuned delays) is preserved unchanged.
//
// Focus model (do not "fix" by adding a Win32 focus API or a centre-click): the game is always
// UNFOCUSED while the tool is in use (the launcher window holds focus). v1 never calls
// SetForegroundWindow — a CLICK on the game is what brings it forward, so Click() is the focus
// mechanism.
public static class GemPointer
{
    /// <summary>Game client rect by window-title substring; null if the game isn't open.</summary>
    public static WindowRect? Client(string titleSubstring)
        => WindowFinder.GetClientRectInScreen(WindowFinder.FindByTitle(titleSubstring));

    /// <summary>Absolute move of the OS cursor to a client-relative point (no click).</summary>
    public static void To(WindowRect client, int x, int y) => WindowFinder.MoveCursor(client, x, y);

    /// <summary>HID left-click at the current cursor position. This is what focuses the game.</summary>
    public static void Click(SerialPort ser) => ser.Write("C\n");

    /// <summary>HID relative "D dx dy" move.</summary>
    public static void Move(SerialPort ser, int dx, int dy) => ser.Write($"D {dx} {dy}\n");
}
