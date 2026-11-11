using Medium = Kaeri.Engine.Core.Medium;
using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// The words the series page, its dialogs and the send dialog share: the main action, a list's
/// name, a chapter's second line, a volume's heading, a tracker's progress, the source in use.
/// The twin of the Apple app's ReadSeriesWords and SendWords.
/// </summary>
static class SeriesWords
{
    /// <summary>The lists a series can be put on from Status, in AniList's order.</summary>
    public static readonly LibrarySegmentRead[] Statuses =
    [
        LibrarySegmentRead.Reading, LibrarySegmentRead.Planning, LibrarySegmentRead.Completed,
        LibrarySegmentRead.Paused, LibrarySegmentRead.Dropped, LibrarySegmentRead.Rereading,
    ];

    /// <summary>The trackers that follow reading, in the order the Tracking dialog lists them.</summary>
    public static readonly string[] ReadingTrackers = ["anilist", "mangaupdates", "kitsu", "myanimelist"];

    public static string List(LibrarySegmentRead list) => list switch
    {
        LibrarySegmentRead.Reading => T.ReadListReading,
        LibrarySegmentRead.Planning => T.ReadListPlanning,
        LibrarySegmentRead.Completed => T.ReadListCompleted,
        LibrarySegmentRead.Paused => T.ReadListPaused,
        LibrarySegmentRead.Dropped => T.ReadListDropped,
        LibrarySegmentRead.Rereading => T.ReadListRereading,
        _ => T.ReadListYourFiles,
    };

    public static string Tracker(string id) => KaeriFfiMethods.TrackerName(id);

    /// <summary>
    /// The label is a chapter's number, "12", "10.5" or "12-13", which the words put "chapter" or
    /// "part" before; anything else is the chapter's own title.
    /// </summary>
    public static bool Numbered(string label) =>
        label.Length > 0 && char.IsAsciiDigit(label[0]) && label.All(c => char.IsAsciiDigit(c) || c == '.' || c == '-');

    /// <summary>The main action's words, or null where there is nothing to read.</summary>
    public static string? Action(SeriesAction? action) => action switch
    {
        // "Read chapter 12", but a chapter with no number goes by its own title: "Read Oneshot".
        SeriesAction.Read read when !Numbered(read.Label) => T.ActionReadNamed(read.Label),
        SeriesAction.Read read => T.ActionReadChapter(read.Label),
        SeriesAction.Continue go when !Numbered(go.Label) => T.ActionContinueNamedPage(go.Label, go.Page),
        SeriesAction.Continue go => T.ActionContinueChapter(go.Label, go.Page),
        SeriesAction.ContinuePart { Percent: { } percent } part when !Numbered(part.Label) => T.ActionContinueNamed(part.Label, percent),
        SeriesAction.ContinuePart { Percent: { } percent } part => T.ActionContinuePart(part.Label, percent),
        SeriesAction.ContinuePart part when !Numbered(part.Label) => T.ActionContinueNamedPlain(part.Label),
        SeriesAction.ContinuePart part => T.ActionContinuePartPlain(part.Label),
        SeriesAction.ReadPart part when !Numbered(part.Label) => T.ActionReadNamed(part.Label),
        SeriesAction.ReadPart part => T.ActionReadPart(part.Label),
        SeriesAction.ReadAgain => T.ActionReadAgain,
        _ => null,
    };

    /// <summary>The chapter the main action opens, when it opens one.</summary>
    public static string? Chapter(SeriesAction? action) => action switch
    {
        SeriesAction.Read read => read.Chapter,
        SeriesAction.Continue go => go.Chapter,
        SeriesAction.ContinuePart part => part.Chapter,
        SeriesAction.ReadPart part => part.Chapter,
        _ => null,
    };

    /// <summary>"Alpha", "Alpha and Brook", "Alpha, Brook and Harbor".</summary>
    public static string Names(IReadOnlyList<string> list) => list.Count switch
    {
        0 => "",
        1 => list[0],
        _ => T.NamesPair(list.Take(list.Count - 1).Skip(1).Aggregate(list[0], T.NamesList), list[^1]),
    };

    /// <summary>"87 of 88 chapters · volume 9", "Volume 5 of 5".</summary>
    public static string Progress(TrackerReadRow row, SeriesView series)
    {
        if (series.Medium == Medium.Novel && row.Volumes is { } volumes)
            return T.SeriesTrackerVolumes(volumes, series.VolumesTotal ?? volumes);
        var chapters = series.ChaptersTotal is { } total ? T.SeriesTrackerChaptersOf(row.Chapters, total) : T.SeriesTrackerChapters(row.Chapters);
        return row.Volumes is uint volume && volume > 0 ? T.SeriesTrackerWithVolume(chapters, volume) : chapters;
    }

    /// <summary>"books.home.example · Kavita · 1 more": the source, its version, and how many others have it.</summary>
    public static string Source(BookEditionRow edition, int others)
    {
        var parts = new List<string> { edition.SourceName };
        if (edition.VersionLabel is { Length: > 0 } label)
            parts.Add(label);
        if (others > 0)
            parts.Add(T.SeriesSourcesMore(others));
        return string.Join(" · ", parts);
    }

    /// <summary>What a chapter's download or place says when the engine gives no line of its own.</summary>
    public static string? State(ChapterRow row)
    {
        switch (row.Download)
        {
            case ChapterDownloadState.Queued:
                return T.ChapterStateQueued;
            case ChapterDownloadState.WaitsForWifi:
                return T.ChapterStateWifi;
            case ChapterDownloadState.Downloading d:
                return T.ChapterStateDownloading(d.DoneValue, d.Total);
            case ChapterDownloadState.Failed f:
                return T.ChapterStateFailed(f.Failure.Words());
        }
        return row.State switch
        {
            ChapterReadState.InProgress { Of: { } of } at => T.ChapterStatePageOf(at.Page, of),
            ChapterReadState.InProgress at => T.ChapterStatePage(at.Page),
            ChapterReadState.Read => T.ChapterStateRead,
            _ => row.NewChapter ? T.ChapterStateNew : null,
        };
    }

    /// <summary>
    /// A chapter row's second line: the engine's words ("Read · 4 days ago · Downloaded"), else
    /// its state, when it came out and the group.
    /// </summary>
    public static string ChapterLine(ChapterRow row, string? preferred)
    {
        if (row.Line is { Length: > 0 } line)
            return line.Joined();
        var parts = new List<string>();
        if (State(row) is { } state)
            parts.Add(state);
        if (row.Download is ChapterDownloadState.Done)
            parts.Add(T.ChapterStateDownloaded);
        if ((row.State is ChapterReadState.Read ? row.ReadAt ?? row.Published : row.Published) is { } at)
            parts.Add(Ago(at));
        if (row.VersionLabel is { Length: > 0 } label && (label != preferred || row.Versions > 1))
            parts.Add(label);
        if (row.Versions > 1)
            parts.Add(T.ChapterUpdatesVersions(row.Versions));
        return string.Join(" · ", parts);
    }

    /// <summary>A chapter's name: "Chapter 12", or the source's label when it has no number.</summary>
    public static string ChapterName(ChapterRow row) => row.Number is { Length: > 0 } n ? T.SeriesChapterNumber(n) : row.Label;

    /// <summary>"Volume 9" and "chapters 77 to 84 · read", or "Not in a volume yet".</summary>
    public static (string Name, string Line) Volume(VolumeGroup group, bool novel)
    {
        var name = group.Volume is { Length: > 0 } volume ? T.SeriesVolume(volume) : T.SeriesNoVolume;
        var rest = new List<string>();
        var numbers = group.Chapters
            .Select(c => double.TryParse(c.Number, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var n) ? n : (double?)null)
            .OfType<double>().ToList();
        if (!novel && numbers.Count > 1 && numbers.Min() != numbers.Max())
            rest.Add(T.SeriesChapterRange(Number(numbers.Min()), Number(numbers.Max())));
        if (group.AllRead)
            rest.Add(T.SeriesVolumeRead);
        return (name, string.Join(" · ", rest));
    }

    /// <summary>The volumes Download offers. A flat list, drawn with no volume headings, still says
    /// each chapter's volume: its rows by volume, in reading order, and none for rows without one.</summary>
    public static IReadOnlyList<VolumeGroup> DownloadVolumes(IReadOnlyList<VolumeGroup> groups)
    {
        if (groups is not [{ Flat: true } list])
            return groups;
        return [.. list.Chapters
            .Where(c => c.Volume is { Length: > 0 })
            .GroupBy(c => c.Volume!)
            .Select(rows => list with { Volume = rows.Key, Title = null, Chapters = [.. rows], AllRead = false, Flat = false })];
    }

    static string Number(double n) => n.ToString("0.##", System.Globalization.CultureInfo.CurrentCulture);

    /// <summary>"3 days ago", from a Unix time, in reading's words.</summary>
    public static string Ago(long unix)
    {
        var seconds = DateTimeOffset.UtcNow.ToUnixTimeSeconds() - unix;
        if (seconds < 60)
            return T.ReadingAgoNow;
        if (seconds < 3600)
            return T.ReadingAgoMinutes(seconds / 60);
        var then = DateTimeOffset.FromUnixTimeSeconds(unix).ToLocalTime().Date;
        var days = (long)(DateTime.Today - then).TotalDays;
        return days switch
        {
            <= 0 => T.ReadingAgoToday,
            1 => T.ReadingAgoYesterday,
            < 7 => T.ReadingAgoDays(days),
            < 31 => T.ReadingAgoWeeks(days / 7),
            < 365 => T.ReadingAgoMonths(days / 30),
            _ => T.ReadingAgoYears(days / 365),
        };
    }

    /// <summary>A date as the send dialogs say it: "6 November".</summary>
    public static string Date(long unix) => Wording.Date(unix);

    // Sending

    public static string Format(ExportFormat format) => format switch
    {
        ExportFormat.FixedEpub => T.SendFormatFixedEpub,
        ExportFormat.Kepub => T.SendFormatKepub,
        ExportFormat.Epub => T.SendFormatEpub,
        _ => T.SendFormatCbz,
    };

    /// <summary>A target's line: the format the files will have and the screen they are made for.</summary>
    public static string TargetLine(ExportTarget target, bool novel)
    {
        var parts = new List<string>();
        if (SendState.Format(target, novel) is { } format)
            parts.Add(T.SendTargetLine(Format(format), target.WidthPx.ToString(System.Globalization.CultureInfo.CurrentCulture), target.HeightPx.ToString(System.Globalization.CultureInfo.CurrentCulture)));
        if (target.Split && !novel)
            parts.Add(T.SendPartsOver(Wording.Bytes(200UL * 1_000_000)));
        return string.Join(", ", parts);
    }

    /// <summary>The e-reader's name, from its id: what the engine lists, else the id as it was kept.</summary>
    public static string Device(Ctx ctx, string id) => ctx.Known.Reading.Sending.Targets.FirstOrDefault(t => t.Id == id)?.Name ?? id;

    // Pieces

    /// <summary>
    /// A book cover in the raised colour until its art arrives, which then fades in over it. Art
    /// the app already has shows at once.
    /// </summary>
    public static Border Cover(Ctx ctx, string? url, double width, double height)
    {
        var image = new Image { Stretch = Stretch.UniformToFill };
        AutomationProperties.SetAccessibilityView(image, AccessibilityView.Raw);
        var frame = new Border
        {
            Width = width,
            Height = height,
            CornerRadius = new CornerRadius(width > 40 ? 8 : 6),
            Background = Ui.Res<Brush>("RaisedBrush"),
            Child = image,
        };
        if (url is { Length: > 0 })
        {
            ArtFade.Watch(image);
            ctx.Art.Bind(url, art => image.Source = art);
        }
        return frame;
    }

    /// <summary>Small capitals, as the facts line has them: the letters raised and spaced at caption size.</summary>
    public static TextBlock SmallCaps(string text) => new()
    {
        Text = text.ToUpper(System.Globalization.CultureInfo.CurrentCulture),
        Style = Ui.Res<Style>("HazeCaptionStyle"),
        FontSize = 11,
        CharacterSpacing = 60,
        TextWrapping = TextWrapping.WrapWholeWords,
    };

    /// <summary>A menu item with a tick before it when <paramref name="on"/>.</summary>
    public static MenuFlyoutItemBase Item(string text, Action act, bool on = false, bool enabled = true)
    {
        if (on)
        {
            var ticked = new ToggleMenuFlyoutItem { Text = text, IsChecked = true, IsEnabled = enabled };
            ticked.Click += (_, _) => act();
            return ticked;
        }
        var item = new MenuFlyoutItem { Text = text, IsEnabled = enabled };
        item.Click += (_, _) => act();
        return item;
    }

    /// <summary>A destructive menu item: its words in the critical colour.</summary>
    public static MenuFlyoutItem Destructive(string text, Action act)
    {
        var item = new MenuFlyoutItem { Text = text, Foreground = Ui.Res<Brush>("SystemFillColorCriticalBrush") };
        item.Click += (_, _) => act();
        return item;
    }

    /// <summary>A dialog's body that scrolls when it is taller than the window lets it be.</summary>
    public static ScrollViewer Scroll(UIElement body, double width = 460) => new()
    {
        Content = body,
        Width = width,
        MaxHeight = 520,
        VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        Padding = new Thickness(0, 0, 12, 0),
    };

    /// <summary>A hairline between rows.</summary>
    public static Border Rule() => new()
    {
        Height = 1,
        Background = Ui.Res<Brush>("FaintRuleBrush"),
    };

    /// <summary>A label over its value, as the side panel's rows and the dialogs' rows read.</summary>
    public static StackPanel Pair(string label, string value)
    {
        var lines = new StackPanel { Spacing = 2 };
        lines.Children.Add(Ui.Text(label, "HazeCaptionStyle", wrap: false));
        lines.Children.Add(Ui.Text(value, "BodyTextBlockStyle"));
        return lines;
    }
}
