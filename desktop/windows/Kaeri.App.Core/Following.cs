using System.Globalization;
using System.Runtime.InteropServices;
using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// The people the viewer follows on AniList, worded for the title page's section and the
/// episode rows' marks: each person's line ("Watching · Episode 8"), the score at a row's end
/// ("8/10") and what a screen reader says in its place, the pictures a mark shows and the
/// sentence its tooltip and the row's name read. Scores belong to the section alone; nothing
/// for a mark carries one.
/// </summary>
public static class Following
{
    /// <summary>The people the section shows before Show all.</summary>
    public const int Shown = 5;

    /// <summary>The pictures a mark shows before "+N".</summary>
    public const int Pictures = 3;

    /// <summary>Whether AniList is signed in here, which is when the switch shows.</summary>
    public static bool SignedIn(Known known) => known.Sync.GetValueOrDefault("anilist") is { Connected: true };

    /// <summary>
    /// Whether a title page asks for the people: AniList signed in and this device's switch on
    /// (on until the engine says otherwise). Only then does the page keep room for them while
    /// they come.
    /// </summary>
    public static bool Live(Known known) =>
        SignedIn(known) && (known.Setting(SettingKey.ShowFollowing) is not { } row || SettingsCatalog.IsOn(row));

    /// <summary>A list status, as the title page's status button words it.</summary>
    public static string Status(LibrarySegment status) => status switch
    {
        LibrarySegment.Watching => T.LibraryStatusWatching,
        LibrarySegment.Planning => T.LibraryStatusPlanning,
        LibrarySegment.Completed => T.LibraryStatusCompleted,
        LibrarySegment.Paused => T.LibraryStatusPaused,
        _ => T.LibraryStatusDropped,
    };

    /// <summary>
    /// The line under a name: "Watching · Episode 8" for someone watching, paused or dropped part
    /// way, the status alone for completed and planning (and for no progress yet). Empty for a
    /// status this build does not know.
    /// </summary>
    public static string Line(FollowedEntry person)
    {
        if (person.Status is not { } status)
            return "";
        var word = Status(status);
        var underway = status is LibrarySegment.Watching or LibrarySegment.Paused or LibrarySegment.Dropped;
        return underway && person.Progress > 0 ? T.MoreFollowedProgress(word, T.EpisodeNumber(person.Progress)) : word;
    }

    /// <summary>The score out of ten, rounded half up, or null when there is none or it rounds to nought.</summary>
    public static long? Score(FollowedEntry person) =>
        person.Score is { } hundred && (long)Math.Round(hundred / 10.0, MidpointRounding.AwayFromZero) is var ten and > 0 ? ten : null;

    /// <summary>"8/10" at the row's end, or empty.</summary>
    public static string ScoreText(FollowedEntry person) => Score(person) is { } score ? T.MoreFollowedScore(score) : "";

    /// <summary>
    /// What a screen reader says for a person's row, as one element: the name, the line, then
    /// "Score 8 out of 10" in place of "8/10".
    /// </summary>
    public static string Spoken(FollowedEntry person)
    {
        var said = person.Name;
        if (Line(person) is { Length: > 0 } line)
            said = T.EpisodeRowSpoken(said, line);
        if (Score(person) is { } score)
            said = T.EpisodeRowSpoken(said, T.FollowingScoreSpoken(score));
        return said;
    }

    /// <summary>The pictures a mark draws, at most three, and how many more the "+N" after them counts.</summary>
    public static (IReadOnlyList<FollowedMark> Pictures, int More) Stack(IReadOnlyList<FollowedMark> marks)
    {
        var shown = marks.Take(Pictures).ToList();
        return (shown, marks.Count - shown.Count);
    }

    /// <summary>"+1", the count after a mark's pictures.</summary>
    public static string More(int count) => T.FollowingMarkMore(count);

    /// <summary>
    /// "Ines, Lumi and Dario watched up to here.": a mark's tooltip and the sentence an episode
    /// row's name gains, every name read, joined by the language's list format. Empty for none.
    /// </summary>
    public static string MarksSpoken(IReadOnlyList<FollowedMark> marks) =>
        marks.Count == 0 ? "" : T.FollowingMarkSpoken(Names.And(marks.Select(m => m.Name).ToList()));

    /// <summary>The marks of a folded run of watched rows: everyone marked inside it, in the rows' order, each once.</summary>
    public static FollowedMark[] Union(IEnumerable<EpisodeRow> rows)
    {
        var seen = new HashSet<string>();
        // A row made with the record's defaults, as a test makes one, has no list at all.
        return [.. rows.SelectMany(r => r.Followed ?? []).Where(m => seen.Add(m.Name))];
    }

    /// <summary>Whether two lists of marks show the same people, picture for picture.</summary>
    public static bool Same(IReadOnlyList<FollowedMark>? a, IReadOnlyList<FollowedMark>? b) =>
        (a ?? []).Select(m => (m.Name, m.AvatarUrl)).SequenceEqual((b ?? []).Select(m => (m.Name, m.AvatarUrl)));

    /// <summary>Whether two lists of people say the same, for drawing the section again only when it changed.</summary>
    public static bool Same(IReadOnlyList<FollowedEntry>? a, IReadOnlyList<FollowedEntry>? b) =>
        (a ?? []).Select(p => (p.Name, p.AvatarUrl, p.Status, p.Score, p.Progress))
            .SequenceEqual((b ?? []).Select(p => (p.Name, p.AvatarUrl, p.Status, p.Score, p.Progress)));

    /// <summary>The letter a person without a picture shows on the raised fill.</summary>
    public static string Initial(string name)
    {
        var text = name.Trim();
        if (text.Length == 0)
            return "";
        var first = StringInfo.GetNextTextElementLength(text);
        return text[..first].ToUpper(CultureInfo.CurrentCulture);
    }
}

/// <summary>
/// Names joined as the language lists them ("Ines, Lumi and Dario"), through the list format of
/// ICU, which Windows carries as its globalization library (icu.dll, Windows 10 1903 and later).
/// Where that is missing, the catalogue's "{first} and {second}" joins the last name to the
/// others, which are separated by commas.
/// </summary>
public static class Names
{
    public static string And(IReadOnlyList<string> names)
    {
        switch (names.Count)
        {
            case 0:
                return "";
            case 1:
                return names[0];
        }
        if (Icu.And(Locale(), names) is { Length: > 0 } listed)
            return listed;
        return Fallback(names);
    }

    /// <summary>The catalogue's join, which <see cref="And"/> uses without ICU.</summary>
    public static string Fallback(IReadOnlyList<string> names) => names.Count switch
    {
        0 => "",
        1 => names[0],
        _ => T.FollowingNamesTwo(string.Join(", ", names.Take(names.Count - 1)), names[^1]), // i18n-skip
    };

    /// <summary>
    /// The app's language, with the system's region when it is the same language, so English
    /// lists as the viewer's English does.
    /// </summary>
    static string Locale()
    {
        var language = T.Language;
        var culture = CultureInfo.CurrentCulture.Name;
        var tag = culture.StartsWith(language, StringComparison.OrdinalIgnoreCase) ? culture : language;
        return tag.Replace('-', '_');
    }

    /// <summary>ICU's ulistfmt C API, looked up in icu.dll once.</summary>
    static unsafe class Icu
    {
        static readonly bool loaded;
        static readonly delegate* unmanaged[Cdecl]<byte*, int*, nint> open;
        static readonly delegate* unmanaged[Cdecl]<nint, char**, int*, int, char*, int, int*, int> format;
        static readonly delegate* unmanaged[Cdecl]<nint, void> close;

        static Icu()
        {
            if (!OperatingSystem.IsWindows() || !NativeLibrary.TryLoad("icu.dll", out var library))
                return;
            if (NativeLibrary.TryGetExport(library, "ulistfmt_open", out var o)
                && NativeLibrary.TryGetExport(library, "ulistfmt_format", out var f)
                && NativeLibrary.TryGetExport(library, "ulistfmt_close", out var c))
            {
                open = (delegate* unmanaged[Cdecl]<byte*, int*, nint>)o;
                format = (delegate* unmanaged[Cdecl]<nint, char**, int*, int, char*, int, int*, int>)f;
                close = (delegate* unmanaged[Cdecl]<nint, void>)c;
                loaded = true;
            }
        }

        /// <summary>Whether the list format was found, for a test to say which join ran.</summary>
        public static bool Loaded => loaded;

        public static string? And(string locale, IReadOnlyList<string> names)
        {
            if (!loaded)
                return null;
            var tag = System.Text.Encoding.ASCII.GetBytes(locale + "\0");
            int status = 0;
            nint formatter;
            fixed (byte* at = tag)
                formatter = open(at, &status);
            if (status > 0 || formatter == 0)
                return null;
            var handles = new GCHandle[names.Count];
            try
            {
                var pointers = new char*[names.Count];
                var lengths = new int[names.Count];
                for (var i = 0; i < names.Count; i++)
                {
                    handles[i] = GCHandle.Alloc(names[i], GCHandleType.Pinned);
                    pointers[i] = (char*)handles[i].AddrOfPinnedObject();
                    lengths[i] = names[i].Length;
                }
                var capacity = names.Sum(n => n.Length) + 16 * names.Count + 16;
                for (var attempt = 0; attempt < 2; attempt++)
                {
                    var buffer = new char[capacity];
                    status = 0;
                    int length;
                    fixed (char** strings = pointers)
                    fixed (int* sizes = lengths)
                    fixed (char* result = buffer)
                        length = format(formatter, strings, sizes, names.Count, result, capacity, &status);
                    // U_BUFFER_OVERFLOW_ERROR: room for the length it needs, then once more.
                    if (status == 15 && length > 0)
                    {
                        capacity = length + 1;
                        continue;
                    }
                    if (status > 0 || length <= 0 || length > capacity)
                        return null;
                    return new string(buffer, 0, length);
                }
                return null;
            }
            finally
            {
                foreach (var handle in handles)
                    if (handle.IsAllocated)
                        handle.Free();
                close(formatter);
            }
        }
    }

    /// <summary>Whether ICU's list format was found on this system.</summary>
    public static bool Platform => Icu.Loaded;
}
