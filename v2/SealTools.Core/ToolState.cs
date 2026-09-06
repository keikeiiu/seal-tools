namespace SealTools.Core;

// Live state of a running tool, shared between the tool thread and the WPF launcher.
// The launcher UI reads this directly — it does NOT parse the tool's console logs.
public sealed class ToolState
{
    public bool Running { get; set; }
    public string? Grade { get; set; }
    public int? Remaining { get; set; }
    public int Attempt { get; set; }
    public int Cycle { get; set; }
    public string? Current { get; set; }
    public string FilterStatus { get; set; } = "";
    public List<string> Attributes { get; set; } = new();
}
