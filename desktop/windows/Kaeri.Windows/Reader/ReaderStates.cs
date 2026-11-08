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
/// page. From 4 s a line under the chapter says it is opening, or counts the wait while a source
/// is found. Narrator hears "Opening Chapter N" once.
/// </summary>
sealed class OpeningView : Grid
{
    readonly PagePlate plate;
    readonly DispatcherQueueTimer tick;
    readonly DateTimeOffset since = DateTimeOffset.UtcNow;
    readonly ReaderSession reader;

    public OpeningView(ReaderSession reader, ReaderPalette palette, string chapter)
    {
        this.reader = reader;
        Background = new SolidColorBrush(palette.Page);
        var words = string.IsNullOrEmpty(chapter) ? "" : T.WindowsReaderOpening(chapter);
        plate = new PagePlate(chapter, words, palette, atOnce: true);
        AutomationProperties.SetAccessibilityView(plate, AccessibilityView.Raw);
        Children.Add(plate);
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

    void Tick()
    {
        var seconds = (int)(DateTimeOffset.UtcNow - since).TotalSeconds;
        if (seconds < 4 || reader.Opening?.Wait != ReaderWait.FindingSource)
            return;
        plate.Loading(T.WindowsReaderOpeningSlow(seconds));
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
/// The page after the last: what ended, whether it counted and where it was sent, the next
/// chapter with the one amber Continue, or why there is none. In a reading demo with no next
/// chapter, the demo's credit and the way to add a source. On a wide window the next chapter's
/// first page shows beside the words.
/// </summary>
sealed class EndPageView : Grid
{
    public EndPageView(ReaderOverlay owner, double width)
    {
        var reader = owner.Reader;
        var palette = owner.Palette;
        Background = new SolidColorBrush(palette.Page);
        var chapter = reader.View?.Chapter.Label ?? "";
        var next = reader.End?.Next ?? reader.View?.Next;
        var demo = reader.View?.DemoCredit;
        var wide = width >= 900 && next is not null && reader.NextPreview is not null;

        var words = new StackPanel { MaxWidth = 440, VerticalAlignment = VerticalAlignment.Center, HorizontalAlignment = wide ? HorizontalAlignment.Left : HorizontalAlignment.Center, Margin = new Thickness(28, 72, 28, 72) };
        var heading = ReaderText.Line(T.WindowsReaderEndOfChapter(chapter), 24, palette.Silk, strong: true);
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

        if (next is not null)
        {
            var block = new StackPanel { Spacing = 4, Margin = new Thickness(0, 28, 0, 0) };
            block.Children.Add(ReaderText.Line(T.ReaderEndNext, 13, palette.Haze, strong: true));
            block.Children.Add(ReaderText.Line(next.Title ?? next.Label, 20, palette.Silk, strong: true));
            block.Children.Add(ReaderText.Line(next.Pages is { } pages ? T.WindowsReaderEndNextDetail(next.Label, (int)pages) : next.Label, 15, palette.Haze));
            if (reader.End?.NextSource is { Length: > 0 } source)
                block.Children.Add(ReaderText.Line(T.WindowsReaderEndFrom(source), 13, palette.Haze));
            words.Children.Add(block);
            var go = ReaderButtons.Amber(T.WindowsReaderContinue, palette, (_, _) => reader.OpenChapter(next.Id));
            ToolTipService.SetToolTip(go, T.TooltipWithKey(T.WindowsReaderContinue, "Space"));
            go.Margin = new Thickness(0, 40, 0, 0);
            words.Children.Add(go);
        }
        else if (demo is not null)
        {
            var block = new StackPanel { Spacing = 6, Margin = new Thickness(0, 28, 0, 0) };
            block.Children.Add(ReaderText.Line(T.ReaderDemoEnd, 20, palette.Silk, strong: true));
            block.Children.Add(ReaderText.Line(T.ReaderDemoEndLine, 15, palette.Haze));
            var card = new Border
            {
                Background = new SolidColorBrush(palette.Raised),
                CornerRadius = new CornerRadius(8),
                Padding = new Thickness(16),
                Margin = new Thickness(0, 12, 0, 0),
                Child = ReaderText.Line(T.ReaderDemoCredit(demo.Series, demo.Author, demo.Licence ?? ""), 13, palette.Haze),
            };
            block.Children.Add(card);
            words.Children.Add(block);
            var add = ReaderButtons.Amber(T.ReaderDemoAddSource, palette, (_, _) => owner.AddSource());
            add.Margin = new Thickness(0, 32, 0, 0);
            words.Children.Add(add);
            var again = ReaderButtons.Pill(T.ReaderDemoAgain, palette, (_, _) => owner.FromStart());
            again.Margin = new Thickness(0, 12, 0, 0);
            words.Children.Add(again);
        }
        else if (reader.End?.Why is { } why)
        {
            var block = new StackPanel { Spacing = 6, Margin = new Thickness(0, 28, 0, 0) };
            var title = why switch
            {
                NoNext.NotOutYet => T.WindowsReaderEndNotOut(chapter),
                NoNext.CaughtUp => T.ReaderEndCaughtUp,
                NoNext.Finished => T.WindowsReaderEndFinished,
                NoNext.NoSourceHasIt => T.WindowsReaderEndNoSource(chapter),
                _ => T.WindowsReaderEndOffline(chapter),
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

        if (wide)
        {
            ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            Children.Add(words);
            var preview = new PageImageView { Margin = new Thickness(28, 48, 48, 48), Dim = palette.Paper };
            preview.Show(reader.NextPreview!, owner.Scale);
            AutomationProperties.SetAccessibilityView(preview, AccessibilityView.Raw);
            SetColumn(preview, 1);
            Children.Add(preview);
            words.HorizontalAlignment = HorizontalAlignment.Right;
        }
        else
            Children.Add(words);
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
            Swap("sending|" + sending.Host, () => Line(T.WindowsReaderServerSending(sending.Host)), animate: true);
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
        WriteBackResult.Saved => T.WindowsReaderServerSaved(host),
        WriteBackResult.Waiting { Problem: WriteBackProblem.Offline } => T.WindowsReaderServerOffline(host),
        WriteBackResult.Waiting => T.WindowsReaderServerRetrying(host),
        WriteBackResult.NotSaved { Problem: WriteBackProblem.NotSetToReceive } => null,
        WriteBackResult.NotSaved { Problem: var problem } => problem switch
        {
            WriteBackProblem.RefusedKey => T.WindowsReaderServerFailed(host, T.WindowsReaderWhyKey),
            WriteBackProblem.RefusedPassword => T.WindowsReaderServerFailed(host, T.WindowsReaderWhyPassword),
            WriteBackProblem.SeriesGone => T.WindowsReaderServerFailed(host, T.WindowsReaderWhyGone),
            WriteBackProblem.Offline => T.WindowsReaderServerOffline(host),
            _ => T.WindowsReaderServerFailed(host, T.WindowsReaderWhyGaveUp),
        },
        _ => null,
    };

    TextBlock Line(string text)
    {
        var line = ReaderText.Line(text, 13, owner.Palette.Haze);
        AutomationProperties.SetLiveSetting(line, AutomationLiveSetting.Polite);
        return line;
    }

    FrameworkElement Result(string text, string? retry)
    {
        var stack = new StackPanel { Spacing = 6 };
        stack.Children.Add(Line(text));
        if (retry is not null)
        {
            var again = ReaderButtons.Pill(T.ReaderTryAgain, owner.Palette, (_, _) => owner.RetryProgress(retry), height: 32);
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
