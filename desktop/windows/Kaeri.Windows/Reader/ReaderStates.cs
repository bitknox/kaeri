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
/// Opening a chapter: nothing for 600 ms, then the cover, the chapter and a ring; after a few
/// seconds of finding a source, a line that counts them.
/// </summary>
sealed class OpeningView : Grid
{
    readonly StackPanel stack;
    readonly TextBlock slow;
    readonly DispatcherQueueTimer first, tick;
    readonly DateTimeOffset since = DateTimeOffset.UtcNow;
    readonly ReaderSession reader;

    public OpeningView(Ctx ctx, ReaderSession reader, ReaderPalette palette, string work, string chapter)
    {
        this.reader = reader;
        Background = new SolidColorBrush(palette.Ground);
        stack = new StackPanel { Spacing = 16, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center, Opacity = 0 };
        if (ctx.Known.Reading.Series.TryGetValue(work, out var series) && series.Cover is { } cover)
        {
            var image = new Image { Stretch = Stretch.UniformToFill };
            AutomationProperties.SetAccessibilityView(image, AccessibilityView.Raw);
            ArtFade.Watch(image);
            ctx.Art.Bind(cover, art => image.Source = art);
            stack.Children.Add(new Border
            {
                Width = 160,
                Height = 240,
                CornerRadius = new CornerRadius(8),
                Background = new SolidColorBrush(palette.Raised),
                Child = image,
                HorizontalAlignment = HorizontalAlignment.Center,
            });
        }
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10, HorizontalAlignment = HorizontalAlignment.Center };
        row.Children.Add(new ProgressRing { IsActive = true, Width = 18, Height = 18, Foreground = new SolidColorBrush(palette.Haze) });
        var words = string.IsNullOrEmpty(chapter) ? "" : T.WindowsReaderOpening(chapter);
        row.Children.Add(ReaderText.Line(words, 15, palette.Silk));
        stack.Children.Add(row);
        slow = ReaderText.Line("", 13, palette.Haze);
        slow.HorizontalAlignment = HorizontalAlignment.Center;
        slow.Visibility = Visibility.Collapsed;
        stack.Children.Add(slow);
        Children.Add(stack);
        AutomationProperties.SetName(this, words);
        AutomationProperties.SetLiveSetting(this, AutomationLiveSetting.Polite);
        first = Timers.Weak(this, TimeSpan.FromMilliseconds(600), false, o => Motion.FadeIn(o.stack));
        tick = Timers.Weak(this, TimeSpan.FromSeconds(1), true, o => o.Tick());
        Loaded += (_, _) =>
        {
            first.Start();
            tick.Start();
        };
        Unloaded += (_, _) =>
        {
            first.Stop();
            tick.Stop();
        };
    }

    void Tick()
    {
        var seconds = (int)(DateTimeOffset.UtcNow - since).TotalSeconds;
        if (seconds < 4 || reader.Opening?.Wait != ReaderWait.FindingSource)
            return;
        slow.Text = T.WindowsReaderOpeningSlow(seconds);
        slow.Visibility = Visibility.Visible;
    }
}

/// <summary>A chapter that would not open: the engine's words, Try again, the way to another source, and Close.</summary>
sealed class ChapterFailedView : Grid
{
    public ChapterFailedView(ReaderOverlay owner, string chapter, ReadingFailure? failure)
    {
        var palette = owner.Palette;
        Background = new SolidColorBrush(palette.Ground);
        var stack = new StackPanel { Spacing = 10, MaxWidth = 420, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(32) };
        var glyph = new FontIcon { Glyph = "", FontSize = 22, Foreground = new SolidColorBrush(palette.Vermilion), HorizontalAlignment = HorizontalAlignment.Left };
        AutomationProperties.SetAccessibilityView(glyph, AccessibilityView.Raw);
        stack.Children.Add(glyph);
        var heading = ReaderText.Line(string.IsNullOrEmpty(chapter) ? T.ReaderTryAgain : T.WindowsReaderChapterFailed(chapter), 20, palette.Silk, strong: true);
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level1);
        stack.Children.Add(heading);
        if (failure is not null)
            stack.Children.Add(ReaderText.Line(failure.Words(), 15, palette.Haze));
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 8, 0, 0) };
        var again = ReaderButtons.Pill(T.ReaderTryAgain, palette, (_, _) => owner.Reader.Retry(null));
        again.Background = new SolidColorBrush(palette.Amber);
        again.Foreground = new SolidColorBrush(palette.OnAmber);
        again.Resources["ButtonBackground"] = new SolidColorBrush(palette.Amber);
        again.Resources["ButtonBackgroundPointerOver"] = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Amber, 0.9));
        again.Resources["ButtonBackgroundPressed"] = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Amber, 0.8));
        again.Resources["ButtonForeground"] = new SolidColorBrush(palette.OnAmber);
        again.Resources["ButtonForegroundPointerOver"] = new SolidColorBrush(palette.OnAmber);
        again.Resources["ButtonForegroundPressed"] = new SolidColorBrush(palette.OnAmber);
        buttons.Children.Add(again);
        buttons.Children.Add(ReaderButtons.Pill(T.WindowsReaderChooseSource, palette, (_, _) => owner.ToSeries()));
        buttons.Children.Add(ReaderButtons.Pill(T.ReaderClose, palette, (_, _) => owner.Close()));
        stack.Children.Add(buttons);
        Children.Add(stack);
        Loaded += (_, _) => again.Focus(FocusState.Programmatic);
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
        words.Children.Add(ServerLine(owner));

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

    /// <summary>
    /// The book server's line: saved, waiting, or not saved with Try again. Two lines' height is
    /// kept for it, so nothing moves when it comes in.
    /// </summary>
    static FrameworkElement ServerLine(ReaderOverlay owner)
    {
        var palette = owner.Palette;
        var holder = new StackPanel { MinHeight = 44, Margin = new Thickness(0, 6, 0, 0), Spacing = 6 };
        if (owner.Reader.ServerProgress is not { } sent)
            return holder;
        var host = sent.Host;
        string? text = sent.Result switch
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
        if (text is null)
            return holder;
        var line = ReaderText.Line(text, 13, palette.Haze);
        AutomationProperties.SetLiveSetting(line, AutomationLiveSetting.Polite);
        holder.Children.Add(line);
        if (sent.Result is WriteBackResult.NotSaved && owner.Reader.ServerProgressServer is { } server)
        {
            var again = ReaderButtons.Pill(T.ReaderTryAgain, palette, (_, _) => owner.RetryProgress(server), height: 32);
            again.HorizontalAlignment = HorizontalAlignment.Left;
            holder.Children.Add(again);
        }
        return holder;
    }
}
