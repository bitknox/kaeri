using System.Globalization;
using Kaeri.App;
using Microsoft.Windows.ApplicationModel.Resources;

namespace Kaeri.Windows;

/// <summary>
/// Connects <see cref="T"/>, the generated strings in Kaeri.App.Core, to this app's resource
/// files (Strings/&lt;language&gt;/Resources.resw, which the build packs into resources.pri), in
/// the language the person picked in Settings or, by default, the system's. Until it is installed,
/// and for any key the language lacks, T answers with its English.
/// </summary>
static class StringResources
{
    /// <summary>The language picked in Settings, or null to follow the system. Read once at start: a change applies on the next launch.</summary>
    public static string? Chosen { get; private set; }

    /// <summary>The choice this run started with: when <see cref="Chosen"/> differs, a restart applies it.</summary>
    public static string? AtStart { get; private set; }

    /// <summary>A fixture run, which neither reads nor writes the profile's choice.</summary>
    static bool fixture;

    /// <summary>
    /// Reads the saved choice and installs the resolver. <paramref name="only"/> is a testing
    /// build's <c>--language</c>, for this run only and never saved (the pseudo-locale's shots).
    /// </summary>
    public static void Install(string? only, bool fixture)
    {
        StringResources.fixture = fixture;
        Chosen = AtStart = fixture ? null : Saved();
        var language = only ?? Chosen;
        try
        {
            if (only is null && !fixture)
                Microsoft.Windows.Globalization.ApplicationLanguages.PrimaryLanguageOverride = Chosen ?? "";
        }
        catch
        {
            // A runtime without the override: the saved file still picks the strings.
        }
        try
        {
            var manager = new ResourceManager();
            var context = manager.CreateResourceContext();
            if (language is { Length: > 0 })
                context.QualifierValues["Language"] = language;
            var map = manager.MainResourceMap.GetSubtree("Resources");
            T.Resolver = key =>
            {
                try { return map.TryGetValue(key, context)?.ValueAsString; }
                catch { return null; }
            };
            T.Language = language is { Length: > 0 } ? language : CultureInfo.CurrentUICulture.Name;
            if (language is { Length: > 0 })
            {
                var culture = CultureInfo.GetCultureInfo(language);
                CultureInfo.DefaultThreadCurrentUICulture = culture;
                CultureInfo.CurrentUICulture = culture;
            }
        }
        catch
        {
            // No resources.pri beside the executable (a bare `dotnet run`): English from T.
        }
        FormatIn(Shown(language));
    }

    /// <summary>
    /// The language the strings come out in: the one asked for, else the system's display
    /// language or its parent when the app has text for it, else English, as the resources fall back.
    /// </summary>
    static string Shown(string? language)
    {
        if (language is { Length: > 0 })
            return language;
        for (var c = CultureInfo.CurrentUICulture; c.Name.Length > 0; c = c.Parent)
        {
            var name = c.Name;
            if (T.Languages.FirstOrDefault(l => string.Equals(l.Tag, name, StringComparison.OrdinalIgnoreCase)) is { Tag: { } tag })
                return tag;
        }
        return "en";
    }

    /// <summary>
    /// Dates and numbers in the language the strings are shown in, with the person's region, as
    /// Apple's Locale.current does. English text on a Danish system reads "Wednesday 4" with the
    /// Danish order and separators (en-DK), not "onsdag 4". When the regional format already
    /// speaks the shown language, it stays as it is, with any changes the person made to it.
    /// </summary>
    static void FormatIn(string shown)
    {
        try
        {
            var current = CultureInfo.CurrentCulture;
            var language = CultureInfo.GetCultureInfo(shown);
            if (current.TwoLetterISOLanguageName == language.TwoLetterISOLanguageName)
                return;
            var culture = language;
            try
            {
                var region = new RegionInfo(current.Name).TwoLetterISORegionName;
                culture = CultureInfo.GetCultureInfo($"{language.TwoLetterISOLanguageName}-{region}", predefinedOnly: true);
            }
            catch (ArgumentException)
            {
                // No such pairing (or no region): the shown language's own formats.
            }
            CultureInfo.DefaultThreadCurrentCulture = culture;
            CultureInfo.CurrentCulture = culture;
        }
        catch (ArgumentException)
        {
            // An unknown tag: the system's formats.
        }
    }

    /// <summary>Saves the choice for the next launch; null follows the system.</summary>
    public static void Choose(string? language)
    {
        Chosen = language;
        if (fixture)
            return;
        try
        {
            Microsoft.Windows.Globalization.ApplicationLanguages.PrimaryLanguageOverride = language ?? "";
        }
        catch
        {
        }
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(Saving)!);
            if (language is null)
                File.Delete(Saving);
            else
                File.WriteAllText(Saving, language);
        }
        catch
        {
        }
    }

    /// <summary>
    /// The saved choice: kept in the profile beside the engine's config as well as in the
    /// override, which an unpackaged app may not keep from one launch to the next.
    /// </summary>
    static string? Saved()
    {
        try
        {
            var tag = File.Exists(Saving) ? File.ReadAllText(Saving).Trim() : "";
            return T.Languages.Any(l => l.Tag == tag) ? tag : null;
        }
        catch
        {
            return null;
        }
    }

    static string Saving => Path.Combine(AppDirs.User().Config, "language");
}
