using System;
using System.IO;

namespace SealTools.Core;

// Simple thread-safe append logger. Writes results to a file for later review,
// independent of the console. Logs are never read back to control the program.
public sealed class FileLogger : IDisposable
{
    private readonly object _lock = new();
    private readonly StreamWriter _writer;

    public FileLogger(string path)
    {
        var dir = Path.GetDirectoryName(path);
        if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);
        var stream = new FileStream(path, FileMode.Append, FileAccess.Write, FileShare.ReadWrite);
        _writer = new StreamWriter(stream) { AutoFlush = true };
    }

    public void WriteLine(string line)
    {
        lock (_lock)
        {
            _writer.WriteLine(line);
        }
    }

    public void Dispose() => _writer.Dispose();
}
