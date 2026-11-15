using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Windows.Pages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
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
/// TV's volume, and Play here, Stop casting and More. A failed cast says why in place of the seek
/// line and the transport. Only a start dims.
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
    readonly TextBlock title = new() { FontSize = 22, LineHeight = 28, TextWrapping = TextWrapping.Wrap, MaxLines = 2, TextTrimming = TextTrimming.CharacterEllipsis };
    readonly TextBlock episode = new() { Style = Ui.Res<Style>("HazeBodyStyle"), Margin = new Thickness(0, 4, 0, 0), TextTrimming = TextTrimming.CharacterEllipsis };
    readonly StackPanel failed = new() { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 24, 0, 0), Visibility = Visibility.Collapsed };
    readonly TextBlock reason = new() { TextWrapping = TextWrapping.Wrap, MaxWidth = 420 };
    readonly SeekLine seek = new(compact: false);
    readonly StackPanel transport = new() { Orientation = Orientation.Horizontal, Spacing = 12, Margin = new Thickness(0, 12, 0, 0) };
    readonly FontIcon playGlyph = CastParts.Glyph("", 20);
    public readonly Button Play;
    readonly StackPanel upNext = new() { Spacing = 8, Margin = new Thickness(0, 20, 0, 0), Visibility = Visibility.Collapsed };
    readonly TextBlock upNextLine = new() { Style = Ui.Res<Style>("BodyStrongTextBlockStyle") };
    readonly TextBlock upNextEpisode = new() { Style = Ui.Res<Style>("HazeBodyStyle"), TextTrimming = TextTrimming.CharacterEllipsis };
    readonly StackPanel rows = new() { Spacing = 4, Margin = new Thickness(0, 20, 0, 0) };
    readonly Grid subtitlesRow, audioRow;
    readonly DropDownButton subtitles = new() { MinHeight = 32, MinWidth = 140 };
    readonly DropDownButton audio = new() { MinHeight = 32, MinWidth = 140 };
    readonly Slider level = new() { Minimum = 0, Maximum = 100, StepFrequency = 1, Width = 160, VerticalAlignment = VerticalAlignment.Center };
    readonly TextBlock fixedVolume = new() { Text = T.TvCastVolumeOnTv, Style = Ui.Res<Style>("HazeBodyStyle"), VerticalAlignment = VerticalAlignment.Center };
    readonly Button mute = CastParts.Text(T.TvCastMute);
    readonly StackPanel footer = new() { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 28, 0, 0) };
    readonly Button playHere = CastParts.Text(T.CastPlayHere);
    readonly Button stop = CastParts.Text(T.CastStop);
    readonly Button more;
    readonly Button chooseSource = CastParts.Text(T.PlayerChooseSource, accent: true);
    readonly Button playHereFailed = CastParts.Text(T.CastPlayHere);
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer volumeRest;
    string? tracksDrawn;
    bool quietVolume, wide = true;
    double? volumeAsked;

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
        var chevron = CastParts.Glyph("", 14);
        chevron.VerticalAlignment = VerticalAlignment.Center;
        chevron.Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush");
        titleRow.Children.Add(chevron);
        titleButton = new Button
        {
            Content = titleRow,
            Padding = new Thickness(0),
            MinHeight = 32,
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            HorizontalContentAlignment = HorizontalAlignment.Left,
        };
        titleButton.Click += (_, _) => act.OpenShow();
        side.Children.Add(titleButton);
        side.Children.Add(episode);

        // A cast that failed says why, with the vermilion dot.
        failed.Children.Add(new Ellipse { Width = 7, Height = 7, Fill = new SolidColorBrush(Ui.Vermilion), VerticalAlignment = VerticalAlignment.Top, Margin = new Thickness(0, 7, 0, 0) });
        failed.Children.Add(reason);
        AutomationProperties.SetLiveSetting(reason, AutomationLiveSetting.Assertive);
        side.Children.Add(failed);

        // The seek line and the transport.
        seek.Root.Margin = new Thickness(0, 20, 0, 0);
        seek.Seek = tv.SessionSeekTo;
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
        side.Children.Add(transport);

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

        // Play here, Stop casting and More; Choose source and Play here once it failed.
        playHere.Click += (_, _) => act.PlayHere();
        stop.Click += (_, _) => act.Stop();
        var moreMenu = new MenuFlyout { Placement = FlyoutPlacementMode.TopEdgeAlignedLeft };
        var another = new MenuFlyoutItem { Text = T.TvChooseAnother };
        another.Click += (_, _) => act.ChooseAnother();
        moreMenu.Items.Add(another);
        more = CastParts.Icon(CastParts.Glyph("", 16), T.ActionMore, 40, 32);
        more.Flyout = moreMenu;
        chooseSource.Click += (_, _) => act.ChooseSource();
        playHereFailed.Click += (_, _) => act.PlayHere();
        footer.Children.Add(chooseSource);
        footer.Children.Add(playHereFailed);
        footer.Children.Add(playHere);
        footer.Children.Add(stop);
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
        Art.Show(ctx, drawn.Id, drawn.Image);
        status.Show(drawn.Line, drawn.Status);
        if (title.Text != drawn.Title)
        {
            title.Text = drawn.Title;
            CastParts.Name(titleButton, T.CastOpenShow(drawn.Title));
        }
        titleButton.IsEnabled = drawn.Id is not null;
        if (episode.Text != drawn.EpisodeLine)
            episode.Text = drawn.EpisodeLine;
        episode.Visibility = string.IsNullOrEmpty(drawn.EpisodeLine) ? Visibility.Collapsed : Visibility.Visible;

        var isFailed = drawn.Line == CastLine.Failed;
        Set(failed, isFailed);
        if (isFailed && reason.Text != (drawn.Reason ?? ""))
            reason.Text = drawn.Reason ?? "";
        Set(seek.Root, !isFailed);
        Set(transport, !isFailed);
        seek.Show(drawn);
        CastParts.PlayGlyph(Play, playGlyph, drawn.Paused);
        var opacity = drawn.Dim ? 0.38 : 1;
        if (transport.Opacity != opacity)
            transport.Opacity = opacity;
        foreach (var child in transport.Children.OfType<Control>())
            child.IsEnabled = !drawn.Dim;

        // Up next while the engine says so.
        var next = isFailed || drawn.Dim ? null : card?.UpNext;
        Set(upNext, next is not null);
        if (next is not null)
        {
            var line = TvWords.CastUpNext(next);
            if (upNextLine.Text != line)
                upNextLine.Text = line;
            var nextEpisode = TvWords.CastUpNextEpisode(next);
            if (upNextEpisode.Text != nextEpisode)
                upNextEpisode.Text = nextEpisode;
        }

        // The rows, dimmed while it starts and once it failed.
        var live = !drawn.Dim && !isFailed;
        var rowsOpacity = live ? 1 : 0.38;
        if (rows.Opacity != rowsOpacity)
            rows.Opacity = rowsOpacity;
        var now = card?.Card.Now;
        Tracks(now, live);
        Volume(view, live);

        // The footer.
        Set(chooseSource, isFailed);
        Set(playHereFailed, isFailed);
        Set(playHere, !isFailed);
        Set(stop, !isFailed);
        Set(more, !isFailed);
        playHere.IsEnabled = playHereFailed.IsEnabled = tv.SessionPlay is not null;
        chooseSource.IsEnabled = tv.SessionPlay is not null;
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

    /// <summary>A slider for a TV with a level; "Set on the TV" and Mute for one whose volume is fixed.</summary>
    void Volume(FfiCastSessionView? view, bool live)
    {
        var settable = view?.VolumeControl == FfiVolumeControl.Level;
        Set(level, settable);
        Set(fixedVolume, !settable);
        Set(mute, !settable);
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
}
