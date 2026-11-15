using Kaeri.Engine.Model;

namespace Kaeri.App;

// The series page's group: the row that names the group in use, the picker that changes it, the
// chapter lines that name another group, and Choose chapters' first ticks.

/// <summary>One row of the group picker: the group (null for Most chapters), its two lines, and whether it is in use.</summary>
public sealed record GroupPick(string? Group, string Name, string Line, bool InUse);

public static class GroupPicker
{
    /// <summary>The Group row shows when the source has more than one group to read a chapter from.</summary>
    public static bool Offered(BookEditionRow? edition) => edition?.Groups is { Length: > 1 };

    /// <summary>The group in use: the source's own label for it, or the first group's.</summary>
    public static string InUse(BookEditionRow edition) =>
        (edition.Groups ?? []).FirstOrDefault(g => g.InUse)?.Label
        ?? (edition.VersionLabel is { Length: > 0 } label ? label : (edition.Groups ?? []).FirstOrDefault()?.Label ?? "");

    /// <summary>"Lanternlight Scans · most chapters", or "· your pick" once the person picked it.</summary>
    public static string Value(BookEditionRow edition) =>
        InUse(edition) + " · " + (edition.GroupChosen ? T.SeriesGroupPicked : T.SeriesGroupMost);

    /// <summary>
    /// Most chapters first, naming the group it reads from now, then each group with how many
    /// chapters it has of the series' and its language.
    /// </summary>
    public static List<GroupPick> Rows(BookEditionRow edition)
    {
        var groups = edition.Groups ?? [];
        var rows = new List<GroupPick>
        {
            new(null, T.GroupPickerMost, T.GroupPickerMostNow(groups.FirstOrDefault()?.Label ?? ""), !edition.GroupChosen),
        };
        foreach (var group in groups)
        {
            var line = T.GroupPickerCount(group.Chapters, Math.Max(edition.Chapters, group.Chapters));
            if (group.Language is { Length: > 0 } tag)
                line += " · " + ReadingLines.Language(tag);
            rows.Add(new GroupPick(group.Label, group.Label, line, edition.GroupChosen && group.InUse));
        }
        return rows;
    }
}

public static class ReadingLines
{
    /// <summary>A language tag ("es", "pt-BR") by its language's name in the person's language.</summary>
    public static string Language(string tag) => Wording.LanguageName(tag.Split('-', '_')[0]);

    /// <summary>The line starts with a capital, whatever the words it starts with.</summary>
    public static string Capitalized(string text) =>
        text.Length == 0 ? text : char.ToUpper(text[0], System.Globalization.CultureInfo.CurrentCulture) + text[1..];

    /// <summary>
    /// A chapter row's line from the engine's parts: the languages of other groups by name, and a
    /// capital at the start only, so "from Group B, Spanish" reads "From Group B, Spanish" when it
    /// leads and stays in lower case after a dot.
    /// </summary>
    public static string Row(ChapterRow row)
    {
        var parts = (row.Line ?? []).Select(msg => msg switch
        {
            Msg.Reading { V1: ReadingMsg.ReadingRowFromGroupIn whole } =>
                new Msg.Reading(whole with { Language = Named(row.GroupLanguage, whole.Language) }),
            Msg.Reading { V1: ReadingMsg.ReadingRowPartFromGroupIn part } =>
                new Msg.Reading(part with { Language = Named((row.PartGroups ?? []).FirstOrDefault(p => p.Part == part.Part)?.Language, part.Language) }),
            _ => msg,
        });
        return Capitalized(parts.Joined());
    }

    static string Named(string? tag, string english) => tag is { Length: > 0 } t ? Language(t) : english;
}

public static class ChooseChapters
{
    /// <summary>
    /// The chapters Choose chapters opens with ticked, in reading order: the next
    /// <paramref name="count"/> unread ones not downloaded, from the reader's place on (the
    /// chapter in progress, or the one after the last read). With none after it, the first
    /// such chapters of the series.
    /// </summary>
    public static List<string> Start(IReadOnlyList<ChapterRow> rows, int count = 10)
    {
        var reading = LastIndex(rows, r => r.State is ChapterReadState.InProgress);
        var read = LastIndex(rows, r => r.State is ChapterReadState.Read);
        var at = reading >= 0 ? reading : read + 1;
        static bool Wanted(ChapterRow r) => r.State is not ChapterReadState.Read && r.Download is null;
        var after = rows.Skip(Math.Min(at, rows.Count)).Where(Wanted).ToList();
        var next = after.Count > 0 ? after : [.. rows.Where(Wanted)];
        return [.. next.Take(count).Select(r => r.Id)];
    }

    static int LastIndex(IReadOnlyList<ChapterRow> rows, Func<ChapterRow, bool> match)
    {
        for (var i = rows.Count - 1; i >= 0; i--)
            if (match(rows[i]))
                return i;
        return -1;
    }
}
