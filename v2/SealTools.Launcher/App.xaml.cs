using System;
using System.IO;
using System.Windows;
using SealTools.Core;
using Wpf.Ui.Appearance;
using Wpf.Ui.Controls;

namespace SealTools.Launcher;

/// <summary>Application entry point. Enables DPI awareness, applies the WPF-UI dark theme, and logs unhandled exceptions.</summary>
public partial class App : Application
{
    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        // The calibrated coordinates are LOGICAL (v1 convention — the "logical" Debug Cursor value
        // is 727,696). Run DPI-unaware so SetCursorPos treats them as logical pixels, matching v1.
        // PerMonitorV2 would read them as physical and land in the wrong place.
        ApplicationThemeManager.Apply(ApplicationTheme.Dark, WindowBackdropType.Mica, true);
        DispatcherUnhandledException += (_, args) => { LogError(args.Exception); args.Handled = true; };
        AppDomain.CurrentDomain.UnhandledException += (_, args) => LogError(args.ExceptionObject as Exception);
    }

    private static void LogError(Exception? ex)
    {
        if (ex == null) return;
        try
        {
            var dir = Path.Combine(AppContext.BaseDirectory, "logs");
            Directory.CreateDirectory(dir);
            File.AppendAllText(Path.Combine(dir, "error.log"), $"{DateTime.Now:yyyy-MM-dd HH:mm:ss}: {ex}\n\n");
        }
        catch
        {
            // ignore logging failures
        }
    }
}
