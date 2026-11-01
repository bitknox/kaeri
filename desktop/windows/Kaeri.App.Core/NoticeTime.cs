namespace Kaeri.App;

/// <summary>
/// How long a notice stays. Windows has no timeout setting of its own that the app can read, so
/// the rule is the app's: 4 seconds for information and 8 for an alert, a second more for every ten
/// words past twelve, and twice as long while a screen reader runs. A notice with a button stays
/// until it is dismissed or acted on.
/// </summary>
public static class NoticeTime
{
    /// <summary>The time a notice stays, or null for until it is dismissed.</summary>
    public static TimeSpan? Stays(string text, bool alert, bool hasAction, bool screenReader)
    {
        if (hasAction)
            return null;
        var words = text.Split(' ', StringSplitOptions.RemoveEmptyEntries).Length;
        var seconds = (alert ? 8 : 4) + Math.Max(0, words - 12) / 10;
        return TimeSpan.FromSeconds(seconds * Scale(screenReader));
    }

    /// <summary>What a timer is multiplied by: 2 while a screen reader runs, else 1.</summary>
    public static int Scale(bool screenReader) => screenReader ? 2 : 1;
}
