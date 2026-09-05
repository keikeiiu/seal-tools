using System;
using System.IO;
using System.Windows;
using SealTools.Core;

namespace SealTools.Launcher;

/// <summary>Application entry point. Enables DPI awareness and logs any unhandled startup exception.</summary>
public partial class App : Application
{
    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        WindowFinder.EnablePerMonitorDpiAwareness();
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
