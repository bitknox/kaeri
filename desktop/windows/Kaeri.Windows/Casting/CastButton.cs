using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Windows.Pages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Casting;

/// <summary>
/// The cast button before the caption buttons: hidden while no TV is found, the outline once
/// TVs are, the waves lighting in turn while it connects, filled while connected. Its menu is the
/// chooser (Chromecast, then the TVs running Kaeri on this network) or, with a TV held, what plays
/// there, its volume and Stop casting. A TV running Kaeri driven over its link opens its remote.
/// The slot keeps its width while the button is hidden, so the search box never moves.
/// </summary>
sealed class CastButton : Grid
{
    readonly Ctx ctx;
    readonly TvRemote tv;
    readonly Action expand;
    readonly Button button;
    readonly CastGlyph glyph = new(18);
    readonly Flyout flyout = new() { Placement = FlyoutPlacementMode.BottomEdgeAlignedRight };
    readonly StackPanel menu = new() { Spacing = 0 };
    CastButtonState? shown;
    string? drawn;
    bool open;

    public CastButton(Ctx ctx, TvRemote tv, Action expand)
    {
        this.ctx = ctx;
        this.tv = tv;
        this.expand = expand;
        Width = 40;
        Height = 32;
        VerticalAlignment = VerticalAlignment.Center;
        button = new Button
        {
            Content = glyph,
            Width = 40,
            Height = 32,
            Padding = new Thickness(0),
            CornerRadius = new CornerRadius(4),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            Visibility = Visibility.Collapsed,
        };
        button.Click += (_, _) => Clicked();
        Children.Add(button);
        flyout.FlyoutPresenterStyle = PresenterStyle();
        flyout.Content = menu;
        flyout.Opened += (_, _) => open = true;
        flyout.Closed += (_, _) =>
        {
            open = false;
            drawn = null;
        };
    }

    static Style PresenterStyle()
    {
        var style = new Style(typeof(FlyoutPresenter));
        style.Setters.Add(new Setter(Control.PaddingProperty, new Thickness(8)));
        style.Setters.Add(new Setter(FrameworkElement.MinWidthProperty, 320d));
        style.Setters.Add(new Setter(FrameworkElement.MaxWidthProperty, 360d));
        style.Setters.Add(new Setter(Control.CornerRadiusProperty, new CornerRadius(8)));
        return style;
    }

    /// <summary>The button follows the session; an open menu redraws only when it reads differently.</summary>
    public void Update()
    {
        var state = tv.Available ? tv.Button : CastButtonState.Hidden;
        if (shown != state)
        {
            var was = shown;
            shown = state;
            button.Visibility = state == CastButtonState.Hidden ? Visibility.Collapsed : Visibility.Visible;
            glyph.Show(state);
            if (state is CastButtonState.Connecting or CastButtonState.Connected && was is not null)
                Announce();
        }
        var name = state switch
        {
            CastButtonState.Connecting => T.CastButtonConnecting(tv.SessionName),
            CastButtonState.Connected when tv.SessionHeld => T.CastButtonConnected(tv.SessionName),
            CastButtonState.Connected => T.CastButtonConnected(tv.ConnectedTv?.Name ?? ""),
            _ => T.TvPlayOn,
        };
        if (ToolTipService.GetToolTip(button) as string != name)
            CastParts.Name(button, name);
        if (open)
            Draw();
    }

    /// <summary>Connecting and connected, said once each as they come.</summary>
    void Announce()
    {
        var words = AutomationProperties.GetName(button);
        if ((FrameworkElementAutomationPeer.FromElement(button) ?? FrameworkElementAutomationPeer.CreatePeerForElement(button)) is { } peer)
            peer.RaiseNotificationEvent(AutomationNotificationKind.ActionCompleted, AutomationNotificationProcessing.ImportantMostRecent, words, "cast-button");
    }

    /// <summary>The menu, as a click on the button opens it.</summary>
    public void Open()
    {
        if (button.Visibility == Visibility.Visible)
            Clicked();
    }

    void Clicked()
    {
        // A TV running Kaeri driven over its link: its remote, as the old TV entry opened it.
        if (!tv.SessionHeld && tv.Connected is not null && tv.Live)
        {
            tv.OpenRemote();
            return;
        }
        drawn = null;
        Draw();
        flyout.ShowAt(button);
    }

    /// <summary>The chooser, or the TV held; rebuilt only when it reads differently, so focus stays.</summary>
    void Draw()
    {
        var held = tv.SessionHeld;
        var drawing = held ? Drawn() : null;
        var key = held
            ? $"held|{tv.SessionName}|{drawing?.Id}|{drawing?.Title}|{drawing?.EpisodeLine}|{tv.Session?.VolumeControl}|{tv.Session?.Muted}"
            : "choose|" + string.Join("|", tv.CastRows.Select(r => $"{r.Key},{r.Name}")) + "#" + string.Join("|", tv.Rows.Select(r => $"{r.Key},{r.Name},{r.Detail},{r.Kind}"));
        if (key == drawn)
        {
            if (held)
                level?.Follow(tv.Session);
            return;
        }
        drawn = key;
        menu.Children.Clear();
        level = null;
        if (held)
            Connected(drawing);
        else
            Chooser();
    }

    CastDrawn? Drawn() => tv.Drawn();

    // The chooser.

    void Chooser()
    {
        menu.Width = 320;
        menu.Children.Add(Heading(T.TvPlayOn));
        if (tv.CastRows.Length > 0)
        {
            menu.Children.Add(Group(T.TvCastGroup));
            foreach (var row in tv.CastRows)
                menu.Children.Add(Row(row.Name, T.CastRowThroughComputer, enabled: true, () =>
                {
                    flyout.Hide();
                    tv.ConnectCast(row);
                }));
        }
        if (tv.Rows.Length > 0)
        {
            menu.Children.Add(Group(T.TvOnThisNetwork));
            foreach (var row in tv.Rows)
            {
                var line = row.Kind is FfiTvRowKind.New or FfiTvRowKind.NeedsUpdate ? row.Detail : T.TvRowKaeri;
                menu.Children.Add(Row(row.Name, line, row.Kind != FfiTvRowKind.NeedsUpdate, () =>
                {
                    flyout.Hide();
                    tv.ChooseTv(row);
                }));
            }
        }
    }

    static TextBlock Heading(string text)
    {
        var heading = new TextBlock
        {
            Text = text,
            Style = Ui.Res<Style>("BodyStrongTextBlockStyle"),
            Margin = new Thickness(8, 4, 8, 8),
            TextTrimming = TextTrimming.CharacterEllipsis,
        };
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level1);
        return heading;
    }

    static TextBlock Group(string text)
    {
        var group = new TextBlock
        {
            Text = text,
            Style = Ui.Res<Style>("HazeCaptionStyle"),
            Margin = new Thickness(8, 8, 8, 4),
        };
        AutomationProperties.SetHeadingLevel(group, AutomationHeadingLevel.Level2);
        return group;
    }

    /// <summary>A TV: the TV glyph, its name, and the line under it.</summary>
    static Button Row(string name, string line, bool enabled, Action chosen)
    {
        var grid = new Grid { ColumnSpacing = 12 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var icon = CastParts.Glyph("", 18);
        icon.VerticalAlignment = VerticalAlignment.Center;
        grid.Children.Add(icon);
        var words = new StackPanel { Spacing = 0, VerticalAlignment = VerticalAlignment.Center };
        words.Children.Add(new TextBlock { Text = name, TextTrimming = TextTrimming.CharacterEllipsis });
        words.Children.Add(new TextBlock { Text = line, Style = Ui.Res<Style>("HazeCaptionStyle"), TextTrimming = TextTrimming.CharacterEllipsis });
        SetColumn(words, 1);
        grid.Children.Add(words);
        var row = new Button
        {
            Content = grid,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            MinHeight = 48,
            Padding = new Thickness(8, 6, 8, 6),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            CornerRadius = new CornerRadius(4),
            IsEnabled = enabled,
            Opacity = enabled ? 1 : 0.55,
        };
        AutomationProperties.SetName(row, T.TvRowName(name, line));
        row.Click += (_, _) => chosen();
        return row;
    }

    // The TV held.

    VolumeRow? level;

    void Connected(CastDrawn? drawing)
    {
        menu.Width = 340;
        menu.Children.Add(Heading(T.CastButtonConnected(tv.SessionName)));
        if (drawing is { } now)
        {
            // What plays: the cover, the show and the episode line; it opens the controller.
            var grid = new Grid { ColumnSpacing = 12 };
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            FrameworkElement cover = now.Id is { } id ? Ui.Cover(ctx, id, 40, 60) : new Border { Width = 40, Height = 60, CornerRadius = new CornerRadius(4), Background = Ui.Res<Brush>("RaisedBrush") };
            if (cover is Border border)
                border.CornerRadius = new CornerRadius(4);
            grid.Children.Add(cover);
            var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
            words.Children.Add(new TextBlock { Text = now.Title, Style = Ui.Res<Style>("ShowTitleStyle"), TextTrimming = TextTrimming.CharacterEllipsis });
            if (now.EpisodeLine.Length > 0)
                words.Children.Add(new TextBlock { Text = now.EpisodeLine, Style = Ui.Res<Style>("HazeCaptionStyle"), TextTrimming = TextTrimming.CharacterEllipsis });
            SetColumn(words, 1);
            grid.Children.Add(words);
            var playing = new Button
            {
                Content = grid,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Stretch,
                Padding = new Thickness(8),
                Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
                BorderThickness = new Thickness(0),
                CornerRadius = new CornerRadius(4),
            };
            AutomationProperties.SetName(playing, now.EpisodeLine.Length > 0 ? $"{now.Title}, {now.EpisodeLine}" : now.Title);
            playing.Click += (_, _) =>
            {
                flyout.Hide();
                expand();
            };
            menu.Children.Add(playing);
        }
        else
        {
            var idle = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12, Margin = new Thickness(8, 8, 8, 8) };
            var still = new CastGlyph(18, Ui.Res<Brush>("TextFillColorSecondaryBrush"));
            still.Show(CastButtonState.Available);
            idle.Children.Add(still);
            idle.Children.Add(new TextBlock { Text = T.CastIdle, Style = Ui.Res<Style>("HazeBodyStyle"), VerticalAlignment = VerticalAlignment.Center });
            menu.Children.Add(idle);
        }

        level = new VolumeRow(tv);
        level.Follow(tv.Session);
        menu.Children.Add(level.Root);

        var stop = CastParts.Text(T.CastStop);
        stop.HorizontalAlignment = HorizontalAlignment.Stretch;
        stop.Margin = new Thickness(8, 8, 8, 4);
        stop.Click += (_, _) =>
        {
            flyout.Hide();
            tv.DisconnectCast();
        };
        menu.Children.Add(stop);
    }

    /// <summary>The TV's volume in the menu: a slider for a level, or "Set on the TV" and Mute.</summary>
    sealed class VolumeRow
    {
        readonly TvRemote tv;
        readonly Slider slider = new() { Minimum = 0, Maximum = 100, StepFrequency = 1, HorizontalAlignment = HorizontalAlignment.Stretch, VerticalAlignment = VerticalAlignment.Center };
        readonly TextBlock fixedLine = new() { Text = T.TvCastVolumeOnTv, Style = Ui.Res<Style>("HazeBodyStyle"), VerticalAlignment = VerticalAlignment.Center };
        readonly Button mute = CastParts.Text(T.TvCastMute);
        readonly Microsoft.UI.Dispatching.DispatcherQueueTimer rest;
        bool quiet;
        double? asked;

        public VolumeRow(TvRemote tv)
        {
            this.tv = tv;
            Root.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            Root.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            Root.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var icon = CastParts.Glyph("", 16);
            icon.VerticalAlignment = VerticalAlignment.Center;
            Root.Children.Add(icon);
            SetColumn(slider, 1);
            SetColumn(fixedLine, 1);
            SetColumn(mute, 2);
            Root.Children.Add(slider);
            Root.Children.Add(fixedLine);
            Root.Children.Add(mute);
            AutomationProperties.SetName(slider, T.TvVolume);
            mute.Click += (_, _) => tv.SessionCommand(new FfiTvCommand.Mute(!(tv.Session?.Muted ?? false)));
            rest = slider.DispatcherQueue.CreateTimer();
            rest.Interval = TimeSpan.FromMilliseconds(200);
            rest.IsRepeating = false;
            rest.Tick += (_, _) =>
            {
                if (asked is not { } to)
                    return;
                var step = (int)Math.Round(to - Math.Round((tv.Session?.Level ?? 0) * 100));
                if (step != 0)
                    tv.SessionCommand(new FfiTvCommand.Volume(step));
            };
            slider.ValueChanged += (_, e) =>
            {
                if (quiet)
                    return;
                asked = e.NewValue;
                rest.Stop();
                rest.Start();
            };
        }

        public Grid Root { get; } = new() { ColumnSpacing = 12, MinHeight = 40, Padding = new Thickness(8, 4, 8, 4) };

        public void Follow(FfiCastSessionView? view)
        {
            var settable = view?.VolumeControl == FfiVolumeControl.Level;
            slider.Visibility = settable ? Visibility.Visible : Visibility.Collapsed;
            fixedLine.Visibility = mute.Visibility = settable ? Visibility.Collapsed : Visibility.Visible;
            var word = TvWords.MuteWord(view?.Muted == true);
            if (mute.Content as string != word)
            {
                mute.Content = word;
                AutomationProperties.SetName(mute, word);
            }
            if (view?.Level is not { } told)
                return;
            var at = Math.Round(told * 100);
            if (asked is { } want && Math.Abs(want - at) >= 1.5)
                return;
            asked = null;
            quiet = true;
            if (Math.Abs(slider.Value - at) >= 1)
                slider.Value = at;
            quiet = false;
        }
    }
}
