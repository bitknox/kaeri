using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// Profiles, optional: one person is the default and meets no picker, no names and no prompt.
/// From the second profile on, the account item names the one in use and switches.
/// </summary>
public static class ProfileWords
{
    /// <summary>Whether this PC has profiles to pick between: two or more.</summary>
    public static bool Picking(IReadOnlyCollection<ProfileView> profiles) => profiles.Count >= 2;

    /// <summary>The profile in use, when there are profiles.</summary>
    public static ProfileView? Active(IReadOnlyCollection<ProfileView> profiles) =>
        Picking(profiles) ? profiles.FirstOrDefault(p => p.Active) : null;

    /// <summary>The initials an avatar shows: the engine's, or the name's first letter.</summary>
    public static string Initials(ProfileView profile) =>
        profile.Initials is { Length: > 0 } initials ? initials
        : profile.Name.Trim() is { Length: > 0 } name ? char.ToUpperInvariant(name[0]).ToString()
        : "";

    /// <summary>A name typed for a profile, as it is kept: trimmed, and never empty.</summary>
    public static string? Typed(string text) => text.Trim() is { Length: > 0 } name ? name : null;

    /// <summary>
    /// The command that opens the app again on the profile chosen: it waits for this copy to end,
    /// since a second copy only hands its launch to the first, then starts the app with the
    /// arguments it ran with, less a link and the testing switches.
    /// </summary>
    public static string Relaunch(int pid, string app, IEnumerable<string> arguments)
    {
        static string Quoted(string s) => "'" + s.Replace("'", "''").Replace("\"", "\\\"") + "'";
        var again = Updates.RelaunchArguments(arguments).ToList();
        var list = again.Count == 0 ? "" : " -ArgumentList " + string.Join(",", again.Select(Quoted));
        return $"-NoProfile -NonInteractive -WindowStyle Hidden -Command \"Wait-Process -Id {pid} -ErrorAction SilentlyContinue; Start-Process -FilePath {Quoted(app)}{list}\"";
    }
}
