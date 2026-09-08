using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO.Ports;
using System.Linq;
using System.Management;

namespace SealTools.Core;

// A detected serial port with its USB PNP id, friendly name, and whether it matches the
// configured Arduino VID/PID. Used by the launcher's "Arduino" status tab.
public sealed record ArduinoDevice(string Port, string PnpId, string Name, bool IsMatch);

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

    // Enumerate every detected serial port with its PNP id + friendly name, flagging the one(s)
    // that match the configured VID/PID. Always returns a list (possibly empty) — never throws —
    // so the status tab can show exactly what the OS sees even when nothing matches.
    public static List<ArduinoDevice> Diagnose(int vid, IEnumerable<int> pids)
    {
        var vidHex = vid.ToString("X4", CultureInfo.InvariantCulture);
        var pidHexes = pids.Select(p => p.ToString("X4", CultureInfo.InvariantCulture)).ToHashSet(StringComparer.OrdinalIgnoreCase);
        var devices = new List<ArduinoDevice>();

        try
        {
            using var searcher = new ManagementObjectSearcher(
                "SELECT DeviceID, PNPDeviceID FROM Win32_SerialPort");
            foreach (var obj in searcher.Get())
            {
                var port = obj["DeviceID"]?.ToString();
                var pnp = obj["PNPDeviceID"]?.ToString() ?? "";
                if (string.IsNullOrEmpty(port)) continue;
                bool match = pnp.Contains($"VID_{vidHex}", StringComparison.OrdinalIgnoreCase)
                    && pidHexes.Any(ph => pnp.Contains($"PID_{ph}", StringComparison.OrdinalIgnoreCase));
                devices.Add(new ArduinoDevice(port, pnp, "", match));
            }
        }
        catch (Exception ex)
        {
            devices.Add(new ArduinoDevice("", "WMI error: " + ex.Message, "", false));
        }

        // Attach friendly device names (e.g. "Arduino Leonardo (COM5)").
        try
        {
            using var searcher = new ManagementObjectSearcher(
                "SELECT Name FROM Win32_PnPEntity WHERE Name LIKE '%(COM%'");
            var names = new List<string>();
            foreach (var obj in searcher.Get())
                if (obj["Name"]?.ToString() is { } n && n.Length > 0)
                    names.Add(n);

            for (int i = 0; i < devices.Count; i++)
            {
                var d = devices[i];
                var name = names.FirstOrDefault(n => n.EndsWith($"({d.Port})", StringComparison.OrdinalIgnoreCase));
                if (name != null) devices[i] = d with { Name = name };
            }
        }
        catch
        {
            // name lookup is cosmetic — ignore failures
        }

        return devices;
    }

    public static SerialPort Open(string port, int baud)
    {
        var sp = new SerialPort(port, baud)
        {
            ReadTimeout = 1000,
            WriteTimeout = 1000,
            // The ATmega32U4 (Leonardo/Pro Micro) handles USB CDC on the main chip and needs DTR
            // (and RTS) asserted to establish/keep the serial connection alive. (Unlike the Uno,
            // which resets on DTR — the 32U4 does not.)
            DtrEnable = true,
            RtsEnable = true,
        };
        sp.Open();
        return sp;
    }
}
