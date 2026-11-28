using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Windows.Pages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Windows.System;

namespace Kaeri.Windows.Casting;

/// <summary>
/// The expanded controller over the page pane: the header with the chevron down and the status
/// line, the art on the left, and on the right the title with its way to the show, the episode
/// line, the seek line, the transport, Up next while the engine counts, subtitles, audio and the
/// TV's volume, and the footer's two places and More. While the connection is made again only
/// Stop casting takes input and everything else dims in place; an end says why in place of the
/// transport, with Play here in the accent. A TV held with nothing on it says so, with Choose
/// another TV and Disconnect. A start dims the transport.
/// </summary>
sealed class CastController : Grid
{
    readonly Ctx ctx;
    readonly TvRemote tv;
    readonly CastActions act;
    public readonly CastParts.Art Art = new(520, 293, 8);
    public readonly Button Collapse;
    readonly CastParts.StatusLine status = new(14, haze: true);
    readonly StackPanel header;
    readonly Grid body = new() { ColumnSpacing = 40, RowSpacing = 24, MaxWidth = 1100, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
    readonly StackPanel side = new() { Spacing = 0, MinWidth = 340, MaxWidth = 460, VerticalAlignment = VerticalAlignment.Center };
    readonly Button titleButton;
    readonly FontIcon titleChevron = CastParts.Glyph("", 14);
    readonly TextBlock title = new() { FontSize = 22, LineHeight = 28, TextWrapping = TextWrapping.Wrap, MaxLines = 2, TextTrimming = TextTrimming.CharacterEllipsis };
    readonly TextBlock episode = new() { Style = Ui.Res<Style>("HazeBodyStyle"), Margin = new Thickness(0, 4, 0, 0), TextTrimming = TextTrimming.CharacterEllipsis };
    readonly StackPanel failed = new() { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center, Visibility = Visibility.Collapsed };
    /// <summary>The transport's row, which says why in its place once the cast ended or failed.</summary>
    readonly Grid transportRow = new() { Margin = new Thickness(0, 12, 0, 0), MinHeight = 48 };
    readonly TextBlock reason = new() { TextWrapping = TextWrapping.Wrap, MaxWidth = 420 };
    readonly SeekLine seek = new(compact: false);
    readonly StackPanel transport = new() { Orientation = Orientation.Horizontal, Spacing = 12 };
    readonly FontIcon playGlyph = CastParts.Glyph("", 20);
    public readonly Button Play;
    readonly StackPanel upNext = new() { Spacing = 8, Margin = new Thickness(0, 20, 0, 0), Visibility = Visibility.Collapsed };
    readonly TextBlock upNextLine = new() { Style = Ui.Res<Style>("BodyStrongTextBlockStyle") };
    readonly TextBlock upNextEpisode = new() { Style = Ui.Res<Style>("HazeBodyStyle"), TextTrimming = TextTrimming.CharacterEllipsis };
    // The corner card's other two: Skip opening or ending while the TV offers it, and the note an
    // automatic skip leaves with Undo, as in the player.
    readonly Button skipOffer = CastParts.Text("", accent: true);
    readonly StackPanel skipped = new() { Orientation = Orientation.Horizontal, Spacing = 12, Margin = new Thickness(0, 20, 0, 0), Visibility = Visibility.Collapsed };
    readonly TextBlock skippedLine = new() { Style = Ui.Res<Style>("BodyStrongTextBlockStyle"), VerticalAlignment = VerticalAlignment.Center };
    readonly StackPanel rows = new() { Spacing = 4, Margin = new Thickness(0, 20, 0, 0) };
    readonly Grid subtitlesRow, audioRow;
    readonly DropDownButton subtitles = new() { MinHeight = 32, MinWidth = 140 };
    readonly DropDownButton audio = new() { MinHeight = 32, MinWidth = 140 };
    readonly Slider level = new() { Minimum = 0, Maximum = 100, StepFrequency = 1, Width = 160, VerticalAlignment = VerticalAlignment.Center };
    readonly TextBlock fixedVolume = new() { Text = T.TvCastVolumeOnTv, Style = Ui.Res<Style>("HazeBodyStyle"), VerticalAlignment = VerticalAlignment.Center };
    readonly Button mute = CastParts.KeepLookWhenOff(CastParts.Text(T.TvCastMute));
    readonly StackPanel footer = new() { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 28, 0, 0) };
    readonly Place first, second;
    readonly Button more;
    readonly MenuFlyout moreMenu = new() { Placement = FlyoutPlacementMode.TopEdgeAlignedLeft };
    CastFooter? footerDrawn;
    CastLine? lineDrawn;
    CastDrawn? lastDrawn;
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer volumeRest;
    string? tracksDrawn;
    bool quietVolume, wide = true;
    double? volumeAsked;
    bool? settableSeen;
    bool mutesSeen;

    public CastController(Ctx ctx, TvRemote tv, CastActions act)
    {
        this.ctx = ctx;
        this.tv = tv;
        this.act = act;
        Background = Ui.Res<Brush>("CastPaneBrush");
        TabFocusNavigation = KeyboardNavigationMode.Local;
        XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled;
        AutomationProperties.SetName(this, T.TvPlayOn);
        RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });

        // The header: the chevron down and the status line.
        header = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(16, 12, 16, 0) };
        Collapse = CastParts.Icon(CastParts.Glyph("", 14), T.ActionClose);
        Collapse.Click += (_, _) => act.Collapse();
        header.Children.Add(Collapse);
        header.Children.Add(status.Root);

        // The title, its way to the show, and the episode line.
        var titleRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        title.FontFamily = Ui.Res<FontFamily>("ShowFont");
        title.FontWeight = Microsoft.UI.Text.FontWeights.Bold;
        titleRow.Children.Add(title);
        titleChevron.VerticalAlignment = VerticalAlignment.Center;
        titleChevron.Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush");
        titleRow.Children.Add(titleChevron);
        titleButton = new Button
        {
            Content = titleRow,
            Padding = new Thickness(0),
            MinHeight = 32,
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            HorizontalContentAlignment = HorizontalAlignment.Left,
        };
        CastParts.KeepLookWhenOff(titleButton);
        titleButton.Click += (_, _) => act.OpenShow();
        side.Children.Add(titleButton);
        side.Children.Add(episode);

        // The seek line, which dims in place while the transport can't be used.
        seek.Root.Margin = new Thickness(0, 20, 0, 0);
        seek.Seek = tv.SessionSeekTo;
        Motion.QuickOpacity(seek.Root);
        side.Children.Add(seek.Root);
        var back = CastParts.Icon(CastParts.Ten(back: true, 20), T.PlayerBack10, 44, 44);
        back.Click += (_, _) => tv.SessionSeekBy(-10);
        Play = CastParts.Round(playGlyph, 48);
        CastParts.Name(Play, T.ActionPause);
        Play.Click += (_, _) => tv.SessionTogglePause();
        var forward = CastParts.Icon(CastParts.Ten(back: false, 20), T.PlayerForward10, 44, 44);
        forward.Click += (_, _) => tv.SessionSeekBy(10);
        var next = CastParts.Icon(CastParts.Glyph("", 18), T.PlayerNextEpisode, 44, 44);
        next.Click += (_, _) => tv.SessionCommand(new FfiTvCommand.Next());
        transport.Children.Add(back);
        transport.Children.Add(Play);
        transport.Children.Add(forward);
        transport.Children.Add(next);
        Motion.QuickOpacity(transport);
        transportRow.Children.Add(transport);

        // A cast that ended or failed says why in the transport's place, with the vermilion dot.
        failed.Children.Add(new Ellipse { Width = 7, Height = 7, Fill = new SolidColorBrush(Ui.Vermilion), VerticalAlignment = VerticalAlignment.Top, Margin = new Thickness(0, 7, 0, 0) });
        failed.Children.Add(reason);
        transportRow.Children.Add(failed);
        side.Children.Add(transportRow);

        // Up next, while the engine counts.
        upNext.Children.Add(upNextLine);
        upNext.Children.Add(upNextEpisode);
        var upNextButtons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var cancel = CastParts.Text(T.ActionCancel);
        cancel.Click += (_, _) => tv.SessionCancelNext();
        var playNow = CastParts.Text(T.ActionPlayNow, accent: true);
        playNow.Click += (_, _) => tv.SessionPlayNext();
        upNextButtons.Children.Add(cancel);
        upNextButtons.Children.Add(playNow);
        upNext.Children.Add(upNextButtons);
        AutomationProperties.SetLiveSetting(upNextLine, AutomationLiveSetting.Polite);
        side.Children.Add(upNext);

        // Skip and the Undo note, in the same corner while Up next is not up.
        skipOffer.Margin = new Thickness(0, 20, 0, 0);
        skipOffer.HorizontalAlignment = HorizontalAlignment.Left;
        skipOffer.Visibility = Visibility.Collapsed;
        skipOffer.Click += (_, _) => tv.SessionCommand(new FfiTvCommand.Skip());
        side.Children.Add(skipOffer);
        var undo = CastParts.Text(T.ActionUndo);
        undo.Click += (_, _) => tv.CastUndoSkip();
        AutomationProperties.SetLiveSetting(skippedLine, AutomationLiveSetting.Polite);
        skipped.Children.Add(skippedLine);
        skipped.Children.Add(undo);
        side.Children.Add(skipped);

        // Subtitles, audio and the TV's volume.
        subtitlesRow = Row(T.PlayerSubtitles, subtitles);
        audioRow = Row(T.PlayerAudio, audio);
        var volume = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        volume.Children.Add(level);
        volume.Children.Add(fixedVolume);
        volume.Children.Add(mute);
        AutomationProperties.SetName(level, T.TvVolume);
        mute.Click += (_, _) => tv.SessionCommand(new FfiTvCommand.Mute(!(tv.Session?.Muted ?? false)));
        rows.Children.Add(subtitlesRow);
        rows.Children.Add(audioRow);
        rows.Children.Add(Row(T.TvVolume, volume));
        Motion.QuickOpacity(rows);
        side.Children.Add(rows);
        volumeRest = DispatcherQueue.CreateTimer();
        volumeRest.Interval = TimeSpan.FromMilliseconds(200);
        volumeRest.IsRepeating = false;
        volumeRest.Tick += (_, _) => SendVolume();
        level.ValueChanged += (_, e) =>
        {
            if (quietVolume)
                return;
            volumeAsked = e.NewValue;
            volumeRest.Stop();
            volumeRest.Start();
        };

        // The footer: two places that keep their spot and change their words, and More.
        first = new Place(Run);
        second = new Place(Run);
        more = CastParts.Icon(CastParts.Glyph("", 16), T.ActionMore, 40, 32);
        more.Flyout = moreMenu;
        Motion.QuickOpacity(more);
        footer.Children.Add(first.Root);
        footer.Children.Add(second.Root);
        footer.Children.Add(more);
        side.Children.Add(footer);

        // The art on the left of the side, or over it when the pane is narrow.
        Art.Root.VerticalAlignment = VerticalAlignment.Center;
        Art.Root.Tapped += (_, e) =>
        {
            e.Handled = true;
            act.OpenShow();
        };
        body.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        body.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        body.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        body.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        body.Children.Add(Art.Root);
        body.Children.Add(side);
        var scroll = new ScrollViewer
        {
            Content = body,
            HorizontalScrollMode = ScrollMode.Disabled,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            Padding = new Thickness(36, 16, 36, 32),
        };
        SetRow(scroll, 1);
        Children.Add(header);
        Children.Add(scroll);
        Layout(wide: true);
        SizeChanged += (_, e) => Layout(e.NewSize.Width >= 1000);

        PreviewKeyDown += OnKey;
    }

    /// <summary>Everything but the art: what fades as the controller opens and closes.</summary>
    public IEnumerable<UIElement> Faded => [header, side];

    /// <summary>The side next to the art, or under it when the pane is narrower than 1000.</summary>
    void Layout(bool wide)
    {
        if (this.wide == wide && side.Parent is not null && GetColumn(side) == (wide ? 1 : 0))
            return;
        this.wide = wide;
        SetColumn(side, wide ? 1 : 0);
        SetRow(side, wide ? 0 : 1);
    }

    static Grid Row(string label, FrameworkElement control)
    {
        var row = new Grid { MinHeight = 40 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.Children.Add(new TextBlock { Text = label, VerticalAlignment = VerticalAlignment.Center });
        control.VerticalAlignment = VerticalAlignment.Center;
        SetColumn(control, 1);
        row.Children.Add(control);
        return row;
    }

    /// <summary>Space plays or pauses, Left and Right seek 10 s, Escape closes into the mini.</summary>
    void OnKey(object sender, KeyRoutedEventArgs e)
    {
        var focused = FocusManager.GetFocusedElement(XamlRoot) as DependencyObject;
        switch (e.Key)
        {
            case VirtualKey.Escape:
                e.Handled = true;
                act.Collapse();
                break;
            case VirtualKey.Space when focused is not (ButtonBase or Slider or MenuFlyoutItem):
                e.Handled = true;
                tv.SessionTogglePause();
                break;
            case VirtualKey.Left or VirtualKey.Right when focused is not (Slider or MenuFlyoutItem):
                e.Handled = true;
                tv.SessionSeekBy(e.Key == VirtualKey.Left ? -10 : 10);
                break;
        }
    }

    /// <summary>Draws the frame in place; the card brings the tracks and Up next.</summary>
    public void Show(CastDrawn drawn, FfiCastSessionView? view, FfiCastCard? card)
    {
        var line = drawn.Line;
        var turned = lineDrawn is { } was && was != line;
        var rejoined = lineDrawn == CastLine.Reconnecting && line is CastLine.Playing or CastLine.Paused;
        var from = lastDrawn?.Position;
        lineDrawn = line;
        lastDrawn = drawn;
        var idle = line == CastLine.Idle;
        Art.Show(ctx, drawn.Id, drawn.Image);
        status.Show(line, drawn.Status);
        var heading = idle ? T.CastIdle : drawn.Title;
        if (title.Text != heading)
        {
            // The show's name and Nothing playing yet trade places as a crossfade.
            if (turned && title.Text.Length > 0)
                Crossfade.Swap(titleButton, () => Heading(heading, idle));
            else
                Heading(heading, idle);
        }
        titleButton.IsEnabled = drawn.Id is not null;
        if (episode.Text != drawn.EpisodeLine)
            episode.Text = drawn.EpisodeLine;
        episode.Visibility = string.IsNullOrEmpty(drawn.EpisodeLine) && !idle ? Visibility.Collapsed : Visibility.Visible;

        // The transport's row: the transport, or why it ended in its place, crossfading.
        var ended = line is CastLine.Failed or CastLine.Stopped;
        var said = ended ? drawn.Reason ?? "" : null;
        if ((failed.Visibility == Visibility.Visible) != ended || ended && reason.Text != said)
        {
            if (turned)
                Crossfade.Swap(transportRow, () => Reason(said));
            else
                Reason(said);
        }

        // Seek, transport and rows take input only while it plays or is paused; a start dims the
        // transport, and a reconnect, an end or nothing playing dims them all in place.
        var live = line is CastLine.Playing or CastLine.Paused;
        if (rejoined && from is { } at && Math.Abs(at - drawn.Position) >= 1)
            seek.Glide(at, drawn);
        seek.Show(drawn);
        Opacity(seek.Root, live || drawn.Dim ? 1 : 0.38);
        CastParts.PlayGlyph(Play, playGlyph, drawn.Paused);
        Opacity(transport, live ? 1 : 0.38);
        foreach (var child in transport.Children.OfType<Control>())
            child.IsEnabled = live;

        // Up next while the engine says so.
        var next = live ? card?.UpNext : null;
        Set(upNext, next is not null);
        if (next is not null)
        {
            var upLine = TvWords.CastUpNext(next);
            if (upNextLine.Text != upLine)
                upNextLine.Text = upLine;
            var nextEpisode = TvWords.CastUpNextEpisode(next);
            if (upNextEpisode.Text != nextEpisode)
                upNextEpisode.Text = nextEpisode;
        }
        // Otherwise the Undo note of an automatic skip, else Skip while the TV offers one.
        var undone = live && next is null ? card?.Undo : null;
        Set(skipped, undone is not null);
        if (undone is not null)
        {
            var skippedSaid = TvWords.CastSkipped(undone);
            if (skippedLine.Text != skippedSaid)
                skippedLine.Text = skippedSaid;
        }
        var offer = live && next is null && undone is null ? card?.Card.Now?.Skip : null;
        Set(skipOffer, offer is not null);
        if (offer is not null)
        {
            var words = TvWords.SkipOffered(offer);
            if (skipOffer.Content as string != words)
            {
                skipOffer.Content = words;
                AutomationProperties.SetName(skipOffer, words);
            }
        }

        Opacity(rows, live ? 1 : 0.38);
        var now = card?.Card.Now;
        Tracks(now, live);
        Volume(view, live);

        Footer(CastFooter.For(line, drawn.Notice), turned);
        first.Live(footerDrawn!.FirstLive && (first.Action is not (CastAction.PlayHere or CastAction.ChooseSource) || tv.SessionPlay is not null));
        second.Live(second.Action is not (CastAction.PlayHere or CastAction.ChooseSource) || tv.SessionPlay is not null);

        // Casting stopped with focus on a control that dimmed or left: focus goes to Play here.
        if (ended && turned && first.Action == CastAction.PlayHere && XamlRoot is { } root
            && FocusManager.GetFocusedElement(root) as DependencyObject is var focused
            && (focused is null || Within(focused) && focused != Collapse && focused != titleButton))
            first.Focus();
    }

    /// <summary>The way on, when the footer's first place has the accent: Play here once it ended.</summary>
    public bool FocusFooter() => footerDrawn is { FirstAccent: true } && first.Focus();

    bool Within(DependencyObject? at)
    {
        for (; at is not null; at = VisualTreeHelper.GetParent(at))
            if (at == this)
                return true;
        return false;
    }

    void Heading(string heading, bool idle)
    {
        title.Text = heading;
        title.FontFamily = idle ? FontFamily.XamlAutoFontFamily : Ui.Res<FontFamily>("ShowFont");
        title.FontWeight = idle ? Microsoft.UI.Text.FontWeights.Normal : Microsoft.UI.Text.FontWeights.Bold;
        title.FontSize = idle ? 20 : 22;
        title.Foreground = idle ? Ui.Res<Brush>("TextFillColorSecondaryBrush") : Ui.Res<Brush>("TextFillColorPrimaryBrush");
        titleChevron.Visibility = idle ? Visibility.Collapsed : Visibility.Visible;
        CastParts.Name(titleButton, idle ? heading : T.CastOpenShow(heading));
    }

    void Reason(string? said)
    {
        Set(failed, said is not null);
        Set(transport, said is null);
        if (said is not null && reason.Text != said)
            reason.Text = said;
    }

    static void Opacity(UIElement element, double to)
    {
        if (element.Opacity != to)
            element.Opacity = to;
    }

    /// <summary>
    /// The footer's places change their words in place as a crossfade, each easing to its new
    /// width on the standard spring; More's items follow.
    /// </summary>
    void Footer(CastFooter want, bool turned)
    {
        if (footerDrawn is { } had && had.First == want.First && had.FirstAccent == want.FirstAccent && had.Second == want.Second && had.More.SequenceEqual(want.More))
        {
            footerDrawn = want;
            Opacity(more, want.MoreLive ? 1 : 0.38);
            more.IsEnabled = want.MoreLive && want.More.Length > 0;
            return;
        }
        footerDrawn = want;
        void Change()
        {
            first.Set(want.First, want.FirstAccent);
            // A reason with one way on leaves the second place out.
            if (want.Second is { } then)
                second.Set(then, accented: false);
            Set(second.Root, want.Second is not null);
        }
        if (turned && footer.ActualWidth > 0)
            Crossfade.Swap(footer, Change);
        else
            Change();
        moreMenu.Items.Clear();
        foreach (var action in want.More)
        {
            var item = new MenuFlyoutItem { Text = CastFooter.Words(action) };
            item.Click += (_, _) => Run(action);
            moreMenu.Items.Add(item);
        }
        Set(more, want.More.Length > 0);
        Opacity(more, want.MoreLive ? 1 : 0.38);
        more.IsEnabled = want.MoreLive && want.More.Length > 0;
    }

    void Run(CastAction action)
    {
        switch (action)
        {
            case CastAction.PlayHere:
                act.PlayHere();
                break;
            case CastAction.ChooseAnother:
                act.ChooseAnother();
                break;
            case CastAction.TryAgain:
                act.TryAgain();
                break;
            case CastAction.ChooseSource:
                act.ChooseSource();
                break;
            default:
                act.Stop();
                break;
        }
    }

    /// <summary>The More menu as it stands, opened, for the board's frame of it.</summary>
    public void ShowMore()
    {
        // Inside the window, so a capture of the window has it.
        moreMenu.ShouldConstrainToRootBounds = true;
        moreMenu.ShowAt(more);
    }

    public void Tick(CastDrawn drawn) => seek.Show(drawn);

    static void Set(UIElement element, bool shown)
    {
        var want = shown ? Visibility.Visible : Visibility.Collapsed;
        if (element.Visibility != want)
            element.Visibility = want;
    }

    void Tracks(FfiNowCard? now, bool live)
    {
        var subs = now?.Subtitles ?? [];
        var sounds = now?.Audio ?? [];
        subtitles.IsEnabled = live && subs.Length > 0;
        audio.IsEnabled = live && sounds.Length > 1;
        var key = string.Join("|", subs.Concat(sounds).Select(t => $"{t.Id},{t.Selected},{t.Language},{t.Title}")) + now?.SubtitlesOff;
        if (key == tracksDrawn)
            return;
        tracksDrawn = key;
        subtitles.Content = subs.Length > 0 ? TvWords.Chosen(subs, now!.SubtitlesOff) : T.ValueOff;
        audio.Content = sounds.Length > 0 ? TvWords.Chosen(sounds, off: false) : "";
        AutomationProperties.SetName(subtitles, T.TvSettingValue(T.PlayerSubtitles, (string)subtitles.Content));
        AutomationProperties.SetName(audio, T.TvSettingValue(T.PlayerAudio, (string)audio.Content));
        subtitles.Flyout = Menu(FfiTvTrackKind.Subtitle, subs, now?.SubtitlesOff ?? true);
        audio.Flyout = Menu(FfiTvTrackKind.Audio, sounds, off: false);
        Set(audioRow, sounds.Length > 0);
    }

    MenuFlyout Menu(FfiTvTrackKind kind, FfiTvTrack[] tracks, bool off)
    {
        var menu = new MenuFlyout { Placement = FlyoutPlacementMode.BottomEdgeAlignedRight };
        var group = kind.ToString();
        if (kind == FfiTvTrackKind.Subtitle)
        {
            var none = new RadioMenuFlyoutItem { Text = T.ValueOff, GroupName = group, IsChecked = off };
            none.Click += (_, _) => tv.SessionCommand(new FfiTvCommand.Track(kind, null));
            menu.Items.Add(none);
        }
        foreach (var track in tracks)
        {
            var item = new RadioMenuFlyoutItem { Text = TvWords.Track(track), GroupName = group, IsChecked = !off && track.Selected };
            item.Click += (_, _) => tv.SessionCommand(new FfiTvCommand.Track(kind, track.Id));
            menu.Items.Add(item);
        }
        return menu;
    }

    /// <summary>
    /// A slider for a TV with a level, and Mute where a mute reaches the TV's sound; "Set on the TV"
    /// alone for one whose own volume is in charge. While it takes no input the row keeps the shape
    /// the TV last reported, so a dim changes nothing else.
    /// </summary>
    void Volume(FfiCastSessionView? view, bool live)
    {
        var settable = view?.VolumeControl == FfiVolumeControl.Level;
        var mutes = view?.CanMute == true;
        if (live || view is null || settableSeen is null)
        {
            settableSeen = view is null ? null : settable;
            mutesSeen = mutes;
        }
        else
        {
            settable = settableSeen.Value;
            mutes = mutesSeen;
        }
        Set(level, settable);
        Set(fixedVolume, !settable);
        Set(mute, mutes);
        level.IsEnabled = mute.IsEnabled = view is not null && live;
        var word = TvWords.MuteWord(view?.Muted == true);
        if (mute.Content as string != word)
        {
            mute.Content = word;
            AutomationProperties.SetName(mute, word);
        }
        if (volumeAsked is null && view?.Level is { } told)
        {
            quietVolume = true;
            var at = Math.Round(told * 100);
            if (Math.Abs(level.Value - at) >= 1)
                level.Value = at;
            quietVolume = false;
        }
    }

    /// <summary>The slider rested: the TV moves by the difference from what it said last.</summary>
    void SendVolume()
    {
        if (volumeAsked is not { } to)
            return;
        var from = Math.Round((tv.Session?.Level ?? 0) * 100);
        var step = (int)Math.Round(to - from);
        if (step != 0)
            tv.SessionCommand(new FfiTvCommand.Volume(step));
        // The slider holds its value until the TV says the new level, or a moment passes.
        var asked = to;
        var settle = DispatcherQueue.CreateTimer();
        settle.Interval = TimeSpan.FromSeconds(3);
        settle.IsRepeating = false;
        settle.Tick += (_, _) =>
        {
            if (volumeAsked == asked)
                volumeAsked = null;
        };
        settle.Start();
    }

    /// <summary>The view said a level near the one asked: the slider follows the TV again.</summary>
    public void Heard(FfiCastSessionView? view)
    {
        if (volumeAsked is { } asked && view?.Level is { } told && Math.Abs(told * 100 - asked) < 1.5)
            volumeAsked = null;
    }
}

/// <summary>What the controller's buttons do, which the layer around it decides.</summary>
sealed class CastActions
{
    public required Action Collapse { get; init; }
    public required Action OpenShow { get; init; }
    public required Action PlayHere { get; init; }
    public required Action Stop { get; init; }
    public required Action ChooseAnother { get; init; }
    public required Action ChooseSource { get; init; }
    public required Action TryAgain { get; init; }
}

/// <summary>
/// One place in the remote's footer: a button that keeps its spot and changes its words, in the
/// accent when it is the way on. A change of words eases its width on the standard spring.
/// </summary>
sealed class Place
{
    readonly Button plain = CastParts.KeepLookWhenOff(CastParts.Text(""));
    readonly Button accent = CastParts.KeepLookWhenOff(CastParts.Text("", accent: true), accent: true);
    bool accented;

    public Place(Action<CastAction> run)
    {
        plain.Click += (_, _) => run(Action);
        accent.Click += (_, _) => run(Action);
        accent.Visibility = Visibility.Collapsed;
        Root.Children.Add(plain);
        Root.Children.Add(accent);
        Motion.QuickOpacity(Root);
    }

    public Grid Root { get; } = new();

    public CastAction Action { get; private set; }

    Button Shown => accented ? accent : plain;

    public void Set(CastAction action, bool accented)
    {
        var before = Root.ActualWidth;
        Action = action;
        this.accented = accented;
        var words = CastFooter.Words(action);
        foreach (var button in new[] { plain, accent })
            if (button.Content as string != words)
                button.Content = words;
        plain.Visibility = accented ? Visibility.Collapsed : Visibility.Visible;
        accent.Visibility = accented ? Visibility.Visible : Visibility.Collapsed;
        Root.Width = double.NaN;
        if (before <= 0)
            return;
        Root.Measure(new global::Windows.Foundation.Size(double.PositiveInfinity, double.PositiveInfinity));
        var after = Root.DesiredSize.Width;
        Root.Width = before;
        Motion.Ease(Root, before, after, width => Root.Width = width);
    }

    /// <summary>Takes input, or dims to 38% and takes none.</summary>
    public void Live(bool live)
    {
        var opacity = live ? 1 : 0.38;
        if (Root.Opacity != opacity)
            Root.Opacity = opacity;
        plain.IsEnabled = accent.IsEnabled = live;
    }

    public bool Focus() => Shown.Focus(FocusState.Programmatic);
}
