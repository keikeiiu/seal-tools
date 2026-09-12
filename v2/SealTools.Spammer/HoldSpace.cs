using System;
using System.IO.Ports;
using System.Threading;
using SealTools.Core;
using SealTools.Core.Config;

namespace SealTools.Spammer;

// Holds the spacebar (via the Arduino keyboard HID) so the game auto-picks up items. Start presses
// and holds space (P), Stop releases it (U); the finally block always sends U so the key can't stay
// stuck down if the tool is killed or the port drops.

public sealed class HoldSpace : ToolBase
{
    private readonly HotkeysConfig _hotkeys;

    public HoldSpace(AppConfig cfg)
        : base(cfg.Hotkeys)
    {
        _hotkeys = cfg.Hotkeys;
    }

    public int Run(SerialPort ser, ToolState state, CancellationToken ct)
    {
        bool running = false;
        bool f12Was = Hotkeys.IsDown(_hotkeys.Start);

        void Hold() { try { ser.Write("P\n"); } catch (Exception) { } }
        void Release() { try { ser.Write("U\n"); } catch (Exception) { } }

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
                    QuitPressed = false;
                    Hold();
                    Console.WriteLine("[Panel] HOLDING space");
                    Beep(523, 100);
                }
                else if (!state.Running && running)
                {
                    running = false;
                    Release();
                    Console.WriteLine("[Panel] RELEASED space");
                    Beep(1000, 150);
                }

                bool f12Now = Hotkeys.IsDown(_hotkeys.Start);
                if (f12Now && !f12Was)
                {
                    running = !running;
                    state.Running = running;
                    if (running) { QuitPressed = false; Hold(); Console.WriteLine("[GO] holding space"); Beep(523, 100); }
                    else { Release(); Console.WriteLine("[STOP] released"); Beep(1000, 150); }
                }
                f12Was = f12Now;
            }
        }
        finally
        {
            Release(); // never leave the spacebar stuck down
            state.Running = false;
        }

        Console.WriteLine("Done.");
        return 0;
    }
}
