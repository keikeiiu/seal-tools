using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO.Ports;
using System.Linq;
using System.Management;

namespace SealTools.Core;

// Locates and opens the Arduino Pro Micro (USB serial), the single shared input device.
public static class Arduino
{
    // Find by USB VID/PID via WMI (equivalent to Python serial.tools.list_ports.comports).
    public static string? Find(int vid, IEnumerable<int> pids)
    {
        var vidHex = vid.ToString("X4", CultureInfo.InvariantCulture);
        var pidHexes = pids.Select(p => p.ToString("X4", CultureInfo.InvariantCulture)).ToHashSet(StringComparer.OrdinalIgnoreCase);

        try
        {
            using var searcher = new ManagementObjectSearcher(
                "SELECT DeviceID, PNPDeviceID FROM Win32_SerialPort");
            foreach (var obj in searcher.Get())
            {
                var deviceId = obj["DeviceID"]?.ToString();
                var pnp = obj["PNPDeviceID"]?.ToString() ?? "";
                if (string.IsNullOrEmpty(deviceId) || pnp.Length == 0) continue;
                if (!pnp.Contains($"VID_{vidHex}", StringComparison.OrdinalIgnoreCase)) continue;
                foreach (var pidHex in pidHexes)
                    if (pnp.Contains($"PID_{pidHex}", StringComparison.OrdinalIgnoreCase))
                        return deviceId;
            }
        }
        catch
        {
            // WMI unavailable — fall through to the name-based fallback below.
        }

        // Fallback: first COM port whose device name mentions Arduino.
        try
        {
            using var searcher = new ManagementObjectSearcher(
                "SELECT Name FROM Win32_PnPEntity WHERE Name LIKE '%(COM%'");
            foreach (var obj in searcher.Get())
            {
                var name = obj["Name"]?.ToString() ?? "";
                if (name.Contains("Arduino", StringComparison.OrdinalIgnoreCase))
                {
                    var start = name.LastIndexOf("(COM", StringComparison.OrdinalIgnoreCase);
                    if (start >= 0)
                    {
                        var end = name.IndexOf(')', start);
                        if (end > start)
                            return name.Substring(start + 1, end - start - 1); // "COM5"
                    }
                }
            }
        }
        catch
        {
            // ignore
        }

        return null;
    }

    public static SerialPort Open(string port, int baud)
    {
        var sp = new SerialPort(port, baud)
        {
            ReadTimeout = 1000,
            WriteTimeout = 1000,
        };
        sp.Open();
        return sp;
    }
}
