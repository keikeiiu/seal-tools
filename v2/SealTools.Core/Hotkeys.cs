using System.Runtime.InteropServices;

namespace SealTools.Core;

// Global hotkey polling via GetAsyncKeyState. Virtual-key codes come from config (defaults.yaml).
public static class Hotkeys
{
    [DllImport("user32.dll")]
    private static extern short GetAsyncKeyState(int vKey);

    public static bool IsDown(int vk) => (GetAsyncKeyState(vk) & 0x8000) != 0;
}
