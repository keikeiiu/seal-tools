using System;
using System.IO;

namespace SealTools.Core;

// Simple thread-safe append logger. Writes to a file so results can be reviewed
// later (or read back by the program), independent of the console.
public sealed class FileLogger : IDisposable
{
    private readonly object _lock = new();
    private readonly StreamWriter _writer;

    public FileLogger(string path)
    {
        var dir = Path.GetDirectoryName(path);
        if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);
        _writer = new StreamWriter(path, append: true) { AutoFlush = true };
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
