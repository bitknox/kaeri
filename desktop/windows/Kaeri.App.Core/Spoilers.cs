using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// Spoiler protection: whether it is on for a show, and which episode stills it blurs. Read from
/// <see cref="Known"/> each time, so a page that asks again after <c>Event::Settings</c> or
/// <c>Event::ShowSettings</c> follows the change at once. The Mac's <c>spoilerProtection(for:)</c>.
/// </summary>
public static class Spoilers
{
    /// <summary>
    /// Whether spoiler protection is on for <paramref name="id"/>: what the show sets for itself,
    /// else the Settings row. On until the engine has said either, as the engine's default is.
    /// </summary>
    public static bool On(Known known, uint id)
    {
        if (known.ShowSheets.GetValueOrDefault(id)?.Rows?.FirstOrDefault(r => r.Key == ShowSettingKey.SpoilerProtection) is { Own: not null } own)
            return own.Value is "on" or "true";
        return known.Setting(SettingKey.SpoilerProtection) is not { } row || SettingsCatalog.IsOn(row);
    }

    /// <summary>
    /// Whether an episode's still is blurred: protection is on, and the episode is numbered after
    /// the next one to watch and not finished. The next episode itself always shows.
    /// </summary>
    public static bool StillHidden(EpisodeRow row, uint next, bool on) =>
        on && uint.TryParse(row.Number, out var n) && n > next && !row.Completed;
}
