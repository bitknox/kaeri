using Medium = Kaeri.Engine.Core.Medium;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

static class ReaderText
{
    public static TextBlock Line(string text, double size, global::Windows.UI.Color colour, bool strong = false) => new()
    {
        Text = text,
        FontSize = size,
        FontWeight = strong ? FontWeights.SemiBold : FontWeights.Normal,
        Foreground = new SolidColorBrush(colour),
        TextWrapping = TextWrapping.WrapWholeWords,
    };
}

/// <summary>
/// Opening a chapter: the page plate holds the page's place from the first frame, with the
/// chapter on it, and the page fades in over it when it comes; there is no spinner and no blank
/// page. From 3 s a line under the chapter says what the open waits on and counts the wait; from
/// <see cref="LongWait"/> it offers another source and Go back. Narrator hears each stage once.
/// </summary>
sealed class OpeningView : Grid
{
    /// <summary>How long a stage goes on before the opening offers another way, as the player's start does.</summary>
    public const int LongWait = 15;

    readonly PagePlate plate;
    readonly DispatcherQueueTimer tick;
    readonly DateTimeOffset since = DateTimeOffset.UtcNow;
    readonly ReaderOverlay owner;
    readonly ReaderPalette palette;
    readonly string chapter;
    string? said;
    bool offered;

    public OpeningView(ReaderOverlay owner, ReaderPalette palette, string chapter)
    {
        this.owner = owner;
        this.palette = palette;
        this.chapter = chapter;
        Background = new SolidColorBrush(palette.Page);
        var words = Stage(owner.Reader.Opening?.Wait, chapter);
        plate = new PagePlate(chapter, words, palette, atOnce: true);
        AutomationProperties.SetAccessibilityView(plate, AccessibilityView.Raw);
        Children.Add(plate);
        said = words;
        AutomationProperties.SetName(this, words);
        AutomationProperties.SetLiveSetting(this, AutomationLiveSetting.Polite);
        tick = Timers.Weak(this, TimeSpan.FromSeconds(1), true, o => o.Tick());
        Loaded += (_, _) =>
        {
            tick.Start();
            if (words.Length > 0)
                ReaderSpeech.Raise(this);
        };
        Unloaded += (_, _) => tick.Stop();
    }

    /// <summary>What the open waits on, in words: the stage the engine named, else the chapter.</summary>
    public static string Stage(ReaderWait? wait, string chapter) => wait switch
    {
        ReaderWait.ChapterList list => T.ReaderWaitChapterList(list.Source),
        ReaderWait.ChapterPages pages => T.ReaderWaitChapterPages(pages.Chapter, pages.Source),
        _ => string.IsNullOrEmpty(chapter) ? "" : T.WindowsReaderOpening(chapter),
    };

    void Tick()
    {
        var seconds = (int)(DateTimeOffset.UtcNow - since).TotalSeconds;
        var stage = Stage(owner.Reader.Opening?.Wait, chapter);
        if (stage.Length == 0)
            return;
        if (stage != said)
        {
            // A new stage is said aloud, as the first one was.
            said = stage;
            AutomationProperties.SetName(this, stage);
            ReaderSpeech.Raise(this);
        }
        plate.Loading(seconds < 3 ? stage : T.ReaderWaitSeconds(stage, seconds));
        if (seconds >= LongWait && !offered)
            Offer();
    }

    /// <summary>A stage that goes on offers the way on, as a failed open does.</summary>
    void Offer()
    {
        offered = true;
        var stack = new StackPanel { Spacing = 10, MaxWidth = 440, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Bottom, Margin = new Thickness(28, 28, 28, 40) };
        stack.Children.Add(ReaderText.Line(T.ReaderWaitLong, 15, palette.Haze));
        var quiet = new Grid { ColumnSpacing = 8 };
        quiet.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        quiet.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var choose = ReaderButtons.Plain(T.WindowsReaderChooseSource, palette, (_, _) => owner.ChooseSource());
        var back = ReaderButtons.Plain(T.WindowsReaderGoBack, palette, (_, _) => owner.Close());
        SetColumn(back, 1);
        quiet.Children.Add(choose);
        quiet.Children.Add(back);
        stack.Children.Add(quiet);
        Children.Add(stack);
    }
}

/// <summary>
/// A chapter that would not open: the engine's words, Choose source as the one amber action, and
/// Try again and Close quiet beside each other under it.
/// </summary>
sealed class ChapterFailedView : Grid
{
    public ChapterFailedView(ReaderOverlay owner, string chapter, ReadingFailure? failure)
    {
        var palette = owner.Palette;
        Background = new SolidColorBrush(palette.Ground);
        var stack = new StackPanel { Spacing = 10, MaxWidth = 440, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(28) };
        var title = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10 };
        var glyph = new FontIcon { Glyph = "\uE7BA", FontSize = 20, Foreground = new SolidColorBrush(palette.Vermilion), VerticalAlignment = VerticalAlignment.Top, Margin = new Thickness(0, 3, 0, 0) };
        AutomationProperties.SetAccessibilityView(glyph, AccessibilityView.Raw);
        title.Children.Add(glyph);
        var heading = ReaderText.Line(string.IsNullOrEmpty(chapter) ? T.ReaderTryAgain : T.WindowsReaderChapterFailed(chapter), 22, palette.Silk, strong: true);
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level1);
        title.Children.Add(heading);
        stack.Children.Add(title);
        if (failure is not null)
            stack.Children.Add(ReaderText.Line(failure.Words(), 15, palette.Haze));
        var choose = ReaderButtons.Amber(T.WindowsReaderChooseSource, palette, (_, _) => owner.ChooseSource());
        choose.Margin = new Thickness(0, 22, 0, 0);
        stack.Children.Add(choose);
        var quiet = new Grid { ColumnSpacing = 8 };
        quiet.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        quiet.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var again = ReaderButtons.Plain(T.ReaderTryAgain, palette, (_, _) => owner.Reader.Retry(null));
        var close = ReaderButtons.Plain(T.ReaderClose, palette, (_, _) => owner.Close());
        SetColumn(close, 1);
        quiet.Children.Add(again);
        quiet.Children.Add(close);
        stack.Children.Add(quiet);
        Children.Add(stack);
        Loaded += (_, _) => choose.Focus(FocusState.Programmatic);
    }
}

/// <summary>
/// The page after the last, as the reader board and the Apple and Linux readers lay it out: a
/// 440 column, left-aligned and centred, on the page's own ground (the novel's look, the manga
/// background). What ended, whether it counted and where it was sent; the next chapter with its
/// facts and its first page, or a plate with its number when there is none yet, at every window
/// width; the one amber Continue. With no next chapter it says why. In a reading demo, the
/// demo's cover with its credit and licence, and the way to add a source.
/// </summary>
sealed class EndPageView : Grid
{
    /// <summary>The next chapter's first page, at the board's 1:1.42.</summary>
    const double PreviewWidth = 180;

    public EndPageView(ReaderOverlay owner, double width)
    {
        var reader = owner.Reader;
        var palette = owner.PagePalette;
        Background = new SolidColorBrush(palette.Page);
        var view = reader.View;
        var finished = view?.Chapter;
        var chapter = finished?.Number is { Length: > 0 } number ? T.ReaderEndOfChapterNumber(number) : T.WindowsReaderEndOfChapter(finished?.Label ?? "");
        var next = reader.EndNext;
        var demo = view?.DemoCredit;
        var novel = reader.Medium == Medium.Novel;

        var words = new StackPanel
        {
            Width = Math.Min(440, Math.Max(240, width - 56)),
            VerticalAlignment = VerticalAlignment.Center,
            HorizontalAlignment = HorizontalAlignment.Center,
            Margin = new Thickness(28, 72, 28, 72),
        };
        var heading = ReaderText.Line(chapter, 24, palette.Silk, strong: true);
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level1);
        words.Children.Add(heading);
        var readLine = !reader.Read ? T.WindowsReaderEndNotRead
            : reader.ReadTrackers.Length switch
            {
                0 => T.ReaderEndRead,
                1 => T.WindowsReaderEndTracker(reader.ReadTrackers[0]),
                2 => T.WindowsReaderEndTracker(T.WindowsReaderEndTrackersTwo(reader.ReadTrackers[0], reader.ReadTrackers[1])),
                _ => T.WindowsReaderEndTracker(string.Join(", ", reader.ReadTrackers)),
            };
        var read = ReaderText.Line(readLine, 15, palette.Haze);
        read.Margin = new Thickness(0, 6, 0, 0);
        words.Children.Add(read);
        words.Children.Add(owner.ServerLine.Take());

        if (next is not null && reader.FlowFailed is { } failed && failed.Chapter.Id == next.Id)
        {
            // A strip flowing on could not open the next chapter: it says so here, at the join,
            // and tries that chapter again.
            var block = new StackPanel { Spacing = 6, Margin = new Thickness(0, 22, 0, 0) };
            block.Children.Add(ReaderText.Line(T.ReaderEndNext, 13, palette.Haze, strong: true));
            var label = next.Number is { Length: > 0 } n ? T.ReaderChapterLine(n) : next.Label;
            block.Children.Add(ReaderText.Line(T.WindowsReaderChapterFailed(label), 20, palette.Silk, strong: true));
            block.Children.Add(ReaderText.Line(failed.Failure.Words(), 15, palette.Haze));
            words.Children.Add(block);
            var again = ReaderButtons.Amber(T.ReaderTryAgain, palette, (_, _) => reader.RetryFlow());
            again.Margin = new Thickness(0, 32, 0, 0);
            words.Children.Add(again);
        }
        else if (next is not null)
        {
            var block = new StackPanel { Spacing = 4, Margin = new Thickness(0, 22, 0, 0) };
            block.Children.Add(ReaderText.Line(T.ReaderEndNext, 13, palette.Haze, strong: true));
            block.Children.Add(ReaderText.Line(next.Title ?? next.Label, 20, palette.Silk, strong: true));
            if (Facts(next, reader.End?.NextSource) is { Length: > 0 } facts)
                block.Children.Add(ReaderText.Line(facts, 15, palette.Haze));
            if (!novel)
                block.Children.Add(Preview(owner, next, palette));
            words.Children.Add(block);
            var go = ReaderButtons.Amber(T.WindowsReaderContinue, palette, (_, _) => reader.OpenChapter(next.Id));
            ToolTipService.SetToolTip(go, T.TooltipWithKey(T.WindowsReaderContinue, "Space"));
            go.Margin = new Thickness(0, 32, 0, 0);
            words.Children.Add(go);
            // Past the extras: Skip to chapter 17, between Continue and Back to the series.
            if (reader.End?.SkipTo is { } skip)
            {
                var past = ReaderButtons.Pill(T.ReaderEndSkipTo(skip.Number ?? skip.Label), palette, (_, _) => reader.OpenChapter(skip.Id));
                AutomationProperties.SetHelpText(past, reader.End?.SkipHint ?? "");
                past.Margin = new Thickness(0, 12, 0, 0);
                past.HorizontalAlignment = HorizontalAlignment.Stretch;
                words.Children.Add(past);
            }
            // On a touch screen, one more swipe goes on, as on a phone.
            if (owner.TouchInput && !owner.StripShown)
            {
                var swipe = ReaderText.Line(T.ReaderEndSwipe, 13, palette.Haze);
                swipe.HorizontalAlignment = HorizontalAlignment.Center;
                swipe.Margin = new Thickness(0, 10, 0, 0);
                words.Children.Add(swipe);
            }
        }
        else if (demo is not null)
        {
            var block = new StackPanel { Spacing = 6, Margin = new Thickness(0, 22, 0, 0) };
            block.Children.Add(ReaderText.Line(T.ReaderDemoEnd, 20, palette.Silk, strong: true));
            block.Children.Add(ReaderText.Line(T.ReaderDemoEndLine, 15, palette.Haze));
            block.Children.Add(DemoCard(owner, demo, palette));
            words.Children.Add(block);
            var add = ReaderButtons.Amber(T.ReaderDemoAddSource, palette, (_, _) => owner.AddSource());
            add.Margin = new Thickness(0, 32, 0, 0);
            words.Children.Add(add);
            var again = ReaderButtons.Pill(T.ReaderDemoAgain, palette, (_, _) => owner.FromStart());
            again.Margin = new Thickness(0, 12, 0, 0);
            again.HorizontalAlignment = HorizontalAlignment.Stretch;
            words.Children.Add(again);
        }
        else if (reader.End?.Why is { } why)
        {
            var label = Following(view);
            var block = new StackPanel { Spacing = 6, Margin = new Thickness(0, 22, 0, 0) };
            var title = why switch
            {
                NoNext.NotOutYet => T.WindowsReaderEndNotOut(label),
                NoNext.CaughtUp => T.ReaderEndCaughtUp,
                NoNext.Finished => T.WindowsReaderEndFinished,
                NoNext.NoSourceHasIt => T.WindowsReaderEndNoSource(label),
                _ => T.WindowsReaderEndOffline(label),
            };
            string? body = why switch
            {
                NoNext.NotOutYet => T.WindowsReaderEndNotOutBody,
                NoNext.CaughtUp => T.WindowsReaderEndCaughtUpBody,
                NoNext.Finished => T.WindowsReaderEndFinishedBody,
                NoNext.NoSourceHasIt => null,
                _ => T.WindowsReaderEndOfflineBody,
            };
            block.Children.Add(ReaderText.Line(title, 20, palette.Silk, strong: true));
            if (body is not null)
                block.Children.Add(ReaderText.Line(body, 15, palette.Haze));
            words.Children.Add(block);
        }
        var back = ReaderButtons.Pill(T.WindowsReaderBackToSeries, palette, (_, _) => owner.ToSeries());
        back.Margin = new Thickness(0, 12, 0, 0);
        back.HorizontalAlignment = HorizontalAlignment.Stretch;
        words.Children.Add(back);

        // A short window scrolls the column rather than cutting it.
        Children.Add(new ScrollViewer
        {
            Content = words,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            HorizontalScrollMode = ScrollMode.Disabled,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
        });
    }

    /// <summary>
    /// The chapter a heading with no next chapter names: the next one listed, which offline is
    /// one only a source has; past the last one listed, the next whole number.
    /// </summary>
    internal static string Following(ReaderView? view)
    {
        if (view?.Next is { } next)
            return next.Number is { Length: > 0 } n ? T.ReaderChapterLine(n) : next.Label;
        if (view?.Chapter.Number is { } number
            && double.TryParse(number, global::System.Globalization.NumberStyles.Float, global::System.Globalization.CultureInfo.InvariantCulture, out var at))
            return T.ReaderChapterLine(((long)Math.Floor(at) + 1).ToString(global::System.Globalization.CultureInfo.InvariantCulture));
        return "";
    }

    /// <summary>
    /// "Chapter 13 · 34 pages · from books.example": the chapter's number when it has a title of
    /// its own, its length, and the source it came from.
    /// </summary>
    internal static string Facts(ChapterRow next, string? source)
    {
        var number = next.Title is null ? null : next.Number ?? next.Label;
        var pages = next.Pages is { } count && count > 0 ? T.ReaderEndPages(count) : null;
        var facts = (number, pages) switch
        {
            ({ } n, { } p) => T.ReaderEndFacts(n, p),
            ({ } n, null) => T.ReaderChapterLine(n),
            (null, var p) => p,
        };
        var from = source is { Length: > 0 } ? source : null;
        return (facts, from) switch
        {
            ({ } f, { } s) => T.ReaderDividerFactsSource(f, s),
            (null, { } s) => T.ReaderEndFrom(s),
            (var f, null) => f ?? "",
        };
    }

    /// <summary>The next chapter's first page, faded in over a plate that names the chapter, which stays when there is no page yet.</summary>
    static FrameworkElement Preview(ReaderOverlay owner, ChapterRow next, ReaderPalette palette)
    {
        var frame = new Grid
        {
            Width = PreviewWidth,
            Height = Math.Round(PreviewWidth * 1.42),
            HorizontalAlignment = HorizontalAlignment.Left,
            Margin = new Thickness(0, 14, 0, 0),
            CornerRadius = new CornerRadius(4),
            Background = new SolidColorBrush(palette.Raised),
        };
        var plateLine = next.Number is { Length: > 0 } number ? T.ReaderChapterLine(number) : next.Label;
        frame.Children.Add(new TextBlock
        {
            Text = plateLine,
            FontSize = 15,
            Foreground = new SolidColorBrush(palette.Haze),
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            TextAlignment = TextAlignment.Center,
            TextWrapping = TextWrapping.WrapWholeWords,
            Margin = new Thickness(12),
        });
        if (owner.Reader.NextPreview is { } file)
        {
            var preview = new PageImageView { Dim = palette.Paper, Cut = palette.Paper };
            preview.Show(file, owner.Scale);
            frame.Children.Add(preview);
        }
        AutomationProperties.SetAccessibilityView(frame, AccessibilityView.Raw);
        return frame;
    }

    /// <summary>The demo work's cover beside its credit and licence, on the page's raised fill.</summary>
    static FrameworkElement DemoCard(ReaderOverlay owner, DemoWorkInfo demo, ReaderPalette palette)
    {
        var row = new Grid { ColumnSpacing = 14 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        if (demo.Cover is { Length: > 0 } cover)
        {
            var image = new Image { Stretch = Stretch.UniformToFill };
            var art = new Border
            {
                Width = 72,
                Height = Math.Round(72 * 1.42),
                CornerRadius = new CornerRadius(4),
                Background = new SolidColorBrush(palette.Selected),
                Child = image,
                VerticalAlignment = VerticalAlignment.Top,
            };
            AutomationProperties.SetAccessibilityView(art, AccessibilityView.Raw);
            Ctx.Current.Art.Bind(cover, source => image.Source = source);
            row.Children.Add(art);
        }
        var credit = new StackPanel { Spacing = 4, VerticalAlignment = VerticalAlignment.Center };
        credit.Children.Add(ReaderText.Line(T.ReaderDemoCredit(demo.Series, demo.Author, demo.Licence ?? ""), 13, palette.Haze));
        Grid.SetColumn(credit, 1);
        row.Children.Add(credit);
        return new Border
        {
            Background = new SolidColorBrush(palette.Raised),
            CornerRadius = new CornerRadius(8),
            Padding = new Thickness(14),
            Margin = new Thickness(0, 12, 0, 0),
            Child = row,
        };
    }
}

/// <summary>
/// The book server's line on the end page: saved, sending, waiting, or not saved with Try again.
/// The engine says nothing when a send starts, so the session marks it when the chapter counts as
/// read; the line shows nothing for 600 ms, then "Sending to" for at least 0.3 s, then the answer.
/// Each change crossfades in place on 0.3 s (Paper mode cuts), and two lines' height is held from
/// the first frame, so nothing under it moves. The reader keeps one, and each end page built
/// takes it, so a rebuild keeps its timing and what it showed.
/// </summary>
sealed class ServerLineView : Grid
{
    static readonly TimeSpan Wait = TimeSpan.FromMilliseconds(600);

    readonly ReaderOverlay owner;
    readonly DispatcherQueueTimer later;
    FrameworkElement? shown;
    string? shownKey;
    DateTimeOffset shownAt;
    bool shownPaper;
    /// <summary>A send was seen under way, so its answer comes in with a crossfade.</summary>
    bool sawSending;

    public ServerLineView(ReaderOverlay owner)
    {
        this.owner = owner;
        MinHeight = 44;
        Margin = new Thickness(0, 6, 0, 0);
        later = Timers.Weak(this, Wait, false, o => o.Update());
    }

    /// <summary>The line, out of the end page that had it, for a new one.</summary>
    public FrameworkElement Take()
    {
        if (Parent is Panel panel)
            panel.Children.Remove(this);
        Update();
        return this;
    }

    /// <summary>What the session says now: the line follows, on the waiting rule.</summary>
    public void Update()
    {
        later.Stop();
        var reader = owner.Reader;
        // The line keeps its room from the first frame when the engine names the work's server
        // (progress_host), and takes none for a work from no server.
        Visibility = reader.View?.ProgressHost is { Length: > 0 } || reader.ServerProgress is not null || reader.ServerSending is not null
            ? Visibility.Visible : Visibility.Collapsed;
        var now = DateTimeOffset.UtcNow;
        if (reader.ServerProgress is { } sent)
        {
            // "Sending to" stays up for at least 0.3 s once it showed.
            if (shownKey?.StartsWith("sending|", StringComparison.Ordinal) == true && now - shownAt < Motion.Standard)
            {
                Schedule(Motion.Standard - (now - shownAt));
                return;
            }
            var text = Words(sent.Host, sent.Result);
            var again = text is not null && sent.Result is WriteBackResult.NotSaved && reader.ServerProgressServer is not null;
            var key = text is null ? null : "result|" + text + (again ? "|again" : "");
            Swap(key, () => Result(text!, again ? reader.ServerProgressServer : null), animate: shownKey is not null || sawSending);
            return;
        }
        if (reader.ServerSending is { } sending)
        {
            sawSending = true;
            var due = sending.Since + Wait - now;
            if (due > TimeSpan.Zero)
            {
                Schedule(due);
                return;
            }
            Swap("sending|" + sending.Host, () => Line(T.ProgressServerSending(sending.Host)), animate: true);
            return;
        }
        // Another chapter, or none from a server: the line is empty at once.
        sawSending = false;
        Swap(null, null, animate: false);
    }

    void Schedule(TimeSpan due)
    {
        later.Interval = due < TimeSpan.FromMilliseconds(16) ? TimeSpan.FromMilliseconds(16) : due;
        later.Start();
    }

    static string? Words(string host, WriteBackResult result) => result switch
    {
        WriteBackResult.Saved => T.ProgressServerSaved(host),
        WriteBackResult.Waiting { Problem: WriteBackProblem.Offline } => T.ProgressServerOffline(host),
        WriteBackResult.Waiting => T.ProgressServerRetrying(host),
        WriteBackResult.NotSaved { Problem: WriteBackProblem.NotSetToReceive } => null,
        WriteBackResult.NotSaved { Problem: var problem } => problem switch
        {
            WriteBackProblem.RefusedKey => T.ProgressServerFailed(host, T.ProgressWhyKey),
            WriteBackProblem.RefusedPassword => T.ProgressServerFailed(host, T.ProgressWhyPassword),
            WriteBackProblem.SeriesGone => T.ProgressServerFailed(host, T.ProgressWhyGone),
            WriteBackProblem.Offline => T.ProgressServerOffline(host),
            _ => T.ProgressServerFailed(host, T.ProgressWhyGaveUp),
        },
        _ => null,
    };

    TextBlock Line(string text)
    {
        var line = ReaderText.Line(text, 13, owner.PagePalette.Haze);
        AutomationProperties.SetLiveSetting(line, AutomationLiveSetting.Polite);
        return line;
    }

    FrameworkElement Result(string text, string? retry)
    {
        var stack = new StackPanel { Spacing = 6 };
        stack.Children.Add(Line(text));
        if (retry is not null)
        {
            var again = ReaderButtons.Pill(T.ReaderTryAgain, owner.PagePalette, (_, _) => owner.RetryProgress(retry), height: 32);
            again.HorizontalAlignment = HorizontalAlignment.Left;
            stack.Children.Add(again);
        }
        return stack;
    }

    /// <summary>The new words fade in over the old, which fade away, in the same place.</summary>
    void Swap(string? key, Func<FrameworkElement>? make, bool animate)
    {
        var paper = owner.PaperMode;
        if (key == shownKey && paper == shownPaper)
            return;
        var old = shown;
        shownKey = key;
        shownAt = DateTimeOffset.UtcNow;
        shownPaper = paper;
        shown = key is null ? null : make!();
        if (shown is not null)
            Children.Add(shown);
        if (old is null && shown is null)
            return;
        if (!animate || paper)
        {
            if (old is not null)
                Children.Remove(old);
            return;
        }
        if (shown is not null)
        {
            shown.Opacity = 0;
            Motion.FadeTo(shown, 1, Motion.Standard);
        }
        if (old is not null)
        {
            old.IsHitTestVisible = false;
            Motion.FadeTo(old, 0, Motion.Standard);
            var gone = DispatcherQueue.CreateTimer();
            gone.Interval = Motion.Standard + TimeSpan.FromMilliseconds(40);
            gone.IsRepeating = false;
            gone.Tick += (_, _) =>
            {
                gone.Stop();
                Children.Remove(old);
            };
            gone.Start();
        }
    }
}
