using System.Diagnostics;
using Kaeri.App;
using Kaeri.Engine.Core;
using Kaeri.Engine.Model;
using Microsoft.UI;
using Microsoft.UI.Text;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Media;
using Windows.Graphics;
using Windows.UI;

namespace Kaeri.Windows.Player;

/// <summary>
/// The room in the player (board v5): the column beside the video, the morph that turns the
/// player into the room, the strip in full screen, and the video area's panels before play and
/// when this device has nothing to show. The room's state comes in through <see cref="ShowRoom"/>
/// and every press goes out through <see cref="RoomSent"/>.
/// </summary>
public sealed partial class PlayerView
{
    RoomColumn? roomColumn;
    RoomStrip? roomStrip;
    Grid? roomPanelLayer;
    RoomPanel? roomPanel;
    RoomScene? roomScene;
    RoomConversation? roomChat;
    SourceCandidate[]? roomSources;
    DispatcherTimer? roomTick;
    long roomChangedAt;
    string roomPrint = "";
    bool roomHidden, roomFull;
    int roomGrownBy;

    /// <summary>Every press in the room, as the engine's room commands.</summary>
    internal event Action<RoomCommand>? RoomSent;

    /// <summary>The column is on screen beside the video: not hidden, not in full screen.</summary>
    bool ColumnShows => roomColumn is not null && !roomHidden && !roomFull;

    /// <summary>In a room: the column, or the strip in full screen, speaks for it.</summary>
    internal bool InRoom => roomScene is not null;

    /// <summary>Join (true; and whether anyway, past ten people) or Not now, on an invite looked into.</summary>
    internal event Action<bool, bool>? RoomAnswered;

    /// <summary>A source picked in the Choosing panel, to play in the room.</summary>
    internal event Action<SourceCandidate>? RoomSourcePicked;

    /// <summary>Open sources or Search titles, from the no-source panel: the window goes there.</summary>
    internal event Action<string>? RoomGo;

    void SendRoom(RoomCommand command) => RoomSent?.Invoke(command);

    /// <summary>
    /// The room as it stands. The first call opens the room: with <paramref name="morph"/>, the
    /// player turns into the room (the window frame widens by the column on the system's curve and
    /// the column's contents fade in at their place), otherwise it is simply there.
    /// </summary>
    internal void ShowRoom(RoomScene scene, RoomConversation? chat, bool morph = false)
    {
        var opening = roomScene is null;
        roomScene = scene;
        roomChat = chat;
        if (opening)
            OpenRoom(morph);
        RenderRoom();
    }

    /// <summary>What the video area shows before play or when this device has nothing to show; null for the video.</summary>
    internal void ShowRoomPanel(RoomPanel? panel)
    {
        roomPanel = panel is { Kind: RoomPanelKind.None } ? null : panel;
        BuildRoomPanel();
    }

    /// <summary>What each source has for the room's episode, for the Choosing panel; null while looking.</summary>
    internal void ShowRoomSources(SourceCandidate[]? found)
    {
        roomSources = found;
        if (roomPanel is { Kind: RoomPanelKind.Choosing })
            BuildRoomPanel();
    }

    /// <summary>Out of the room: the column goes and the window gives back what it grew by.</summary>
    internal void EndRoom()
    {
        if (roomScene is null)
            return;
        roomScene = null;
        roomTick?.Stop();
        roomPanel = null;
        roomSources = null;
        BuildRoomPanel();
        if (roomStrip is { } strip)
        {
            Root.Children.Remove(strip);
            roomStrip = null;
        }
        if (roomColumn is { } column)
        {
            var grown = roomGrownBy;
            roomGrownBy = 0;
            Slide(column, open: false, grown, () =>
            {
                Root.Children.Remove(column);
                if (roomColumn == column)
                    roomColumn = null;
                Root.ColumnDefinitions[1].Width = GridLength.Auto;
            });
        }
        ShowRoomButton();
    }

    void OpenRoom(bool morph)
    {
        discussion?.Close();
        var column = new RoomColumn();
        roomColumn = column;
        column.Asked += ask =>
        {
            switch (ask)
            {
                case RoomAsk.Join or RoomAsk.JoinAnyway:
                    RoomAnswered?.Invoke(true, ask == RoomAsk.JoinAnyway);
                    break;
                case RoomAsk.GoAlong when roomScene?.Offer is { } offer:
                    SendRoom(new RoomCommand.AnswerMove(offer.Id, true));
                    break;
                case RoomAsk.CarryOn or RoomAsk.Start:
                    SendRoom(new RoomCommand.CarryOn());
                    break;
            }
        };
        column.Declined += () =>
        {
            if (roomScene?.Offer is { } offer)
                SendRoom(new RoomCommand.AnswerMove(offer.Id, false));
            else
                RoomAnswered?.Invoke(false, false);
        };
        column.InviteClicked += () => ShowRoomInvite();
        column.DetailsClicked += () => ShowRoomDetails();
        column.LeaveClicked += async () =>
        {
            if (XamlRoot is { } root && await RoomPopups.AskLeave(root))
                SendRoom(new RoomCommand.Leave());
        };
        column.Said += text => SendRoom(new RoomCommand.Say(text));
        column.MuteAsked += (id, on) => SendRoom(new RoomCommand.Mute(id, on));
        Grid.SetColumn(column, 1);
        column.HorizontalAlignment = HorizontalAlignment.Left;
        Root.Children.Add(column);

        roomStrip = new RoomStrip { Margin = new Thickness(0, 16, 16, 0), Visibility = Visibility.Collapsed, Opacity = 0 };
        roomStrip.Asked += _ => SendRoom(new RoomCommand.CarryOn());
        stripWanted = false;
        Root.Children.Add(roomStrip);

        roomFull = FullscreenOn?.Invoke() == true;
        roomHidden = false;
        if (roomFull)
            Root.ColumnDefinitions[1].Width = new GridLength(0);
        else
            Slide(column, open: true, Grow(RoomColumn.PaneWidth), null, morph);
        roomTick ??= new DispatcherTimer { Interval = TimeSpan.FromSeconds(1) };
        roomTick.Tick -= OnRoomTick;
        roomTick.Tick += OnRoomTick;
        roomTick.Start();
        ControlsChanged -= OnRoomControls;
        ControlsChanged += OnRoomControls;
        ShowRoomButton();
    }

    void OnRoomTick(object? sender, object e)
    {
        if (roomScene is null)
            return;
        roomScene = roomScene with { NowMs = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds() };
        RenderRoom();
    }

    void OnRoomControls(bool shown) => PlaceRoomStrip();

    void RenderRoom()
    {
        if (roomScene is not { } scene)
            return;
        var print = Rooms.Fingerprint(scene);
        if (print != roomPrint)
        {
            roomPrint = print;
            roomChangedAt = scene.NowMs;
        }
        roomColumn?.Show(scene, roomChat);
        roomStrip?.Show(scene);
        PlaceRoomStrip();
        if (roomPanel is { Kind: RoomPanelKind.SelfLoading })
            BuildRoomPanel();
        ShowRoomUpNext(scene);
    }

    /// <summary>The player's room button: the side panel glyph, lit while the column is open.</summary>
    void ShowRoomButton()
    {
        if (roomScene is null)
        {
            ShowPartyButton();
            return;
        }
        var name = roomHidden || roomFull ? RoomWords.ShowRoom : RoomWords.HideRoom;
        ToolTipService.SetToolTip(PartyButton, name);
        AutomationProperties.SetName(PartyButton, name);
        PartyDot.Opacity = 0;
    }

    /// <summary>The room button: hides or shows the column; the video takes or gives the room.</summary>
    internal void ToggleRoomColumn()
    {
        if (roomColumn is not { } column || roomFull)
            return;
        roomHidden = !roomHidden;
        Slide(column, open: !roomHidden, 0, null);
        ShowRoomButton();
        PlaceRoomStrip();
    }

    // The morph.

    /// <summary>
    /// How far the window can widen for the column, in physical pixels: by the column's width
    /// when the screen has room, by what is left when it doesn't, and not at all when maximised
    /// or in full screen (the video shrinks instead).
    /// </summary>
    int Grow(double epx)
    {
        if (ctx?.Window.AppWindow is not { } window || window.Presenter is not OverlappedPresenter { State: OverlappedPresenterState.Restored })
            return 0;
        var scale = XamlRoot?.RasterizationScale ?? 1;
        var want = (int)Math.Round(epx * scale);
        var area = DisplayArea.GetFromWindowId(window.Id, DisplayAreaFallback.Nearest).WorkArea;
        return Math.Clamp(area.Width - window.Size.Width, 0, want);
    }

    /// <summary>
    /// The column in or out. In: the window frame widens by <paramref name="grow"/> on the system
    /// curve (0.42, 0, 0.58, 1 over 450 ms) and moves left by half to stay centred, the video
    /// keeping its size; the column's contents fade in at their place from 120 ms. Reduce Motion:
    /// the frame takes its size at once and the room fades in over the player.
    /// </summary>
    void Slide(RoomColumn column, bool open, int grow, Action? done, bool morph = true)
    {
        var window = ctx?.Window.AppWindow;
        var from = window is null ? default : new RectInt32(window.Position.X, window.Position.Y, window.Size.Width, window.Size.Height);
        RectInt32 To(double m)
        {
            var by = (int)Math.Round(grow * m);
            var x = from.X - (open ? by / 2 : -by / 2);
            return new RectInt32(x, from.Y, from.Width + (open ? by : -by), from.Height);
        }
        if (open)
            roomGrownBy += grow;
        var width = Root.ColumnDefinitions[1].ActualWidth;
        // From where it stands now, so the column never shows for a frame before it opens.
        Root.ColumnDefinitions[1].Width = new GridLength(width);
        var target = open ? RoomColumn.PaneWidth : 0;
        if (!Motion.Enabled || !morph)
        {
            if (grow > 0 && window is not null)
                window.MoveAndResize(To(1));
            Root.ColumnDefinitions[1].Width = new GridLength(target);
            column.Opacity = 1;
            if (open && morph)
            {
                // The room fades in over the player: nothing slides or grows.
                column.Opacity = 0;
                Motion.FadeTo(column, 1, Motion.FadeInTime);
            }
            done?.Invoke();
            return;
        }
        var contents = column.Children.Cast<UIElement>().ToList();
        if (open)
            foreach (var c in contents)
                c.Opacity = 0;
        Animate(TimeSpan.FromMilliseconds(450), Curves.System, (t, m) =>
        {
            var w = open ? width + (target - width) * m : width * (1 - m);
            Root.ColumnDefinitions[1].Width = new GridLength(Math.Max(0, w));
            if (grow > 0 && window is not null)
                window.MoveAndResize(To(m));
            var fade = open ? Curves.FadeIn(Math.Clamp((t - 120) / 200.0, 0, 1)) : 1 - Curves.FadeIn(Math.Clamp(t / 200.0, 0, 1));
            foreach (var c in contents)
                c.Opacity = fade;
        }, () =>
        {
            foreach (var c in contents)
                c.Opacity = 1;
            done?.Invoke();
        });
    }

    /// <summary>Runs a step every frame for <paramref name="length"/>, with elapsed ms and the curve's value.</summary>
    void Animate(TimeSpan length, Func<double, double> curve, Action<double, double> step, Action? done)
    {
        var clock = Stopwatch.StartNew();
        var total = Motion.Scaled(length).TotalMilliseconds;
        EventHandler<object>? frame = null;
        frame = (_, _) =>
        {
            var t = clock.Elapsed.TotalMilliseconds;
            var p = Math.Clamp(t / total, 0, 1);
            step(t * length.TotalMilliseconds / Math.Max(1, total), curve(p));
            if (p >= 1)
            {
                Microsoft.UI.Xaml.Media.CompositionTarget.Rendering -= frame;
                done?.Invoke();
            }
        };
        Microsoft.UI.Xaml.Media.CompositionTarget.Rendering += frame;
    }

    // Full screen.

    /// <summary>
    /// The room into full screen and back. In: once the window is full screen, the column slides
    /// 24 epx and fades out on standard as the video grows into the screen, then the strip fades
    /// in. Out: the strip fades out and the column comes back on standard. Reduce Motion: the
    /// column goes and comes at once and the strip crossfades (200 ms); nothing slides.
    /// </summary>
    void RoomFullscreen(bool on)
    {
        if (roomColumn is not { } column || roomFull == on)
            return;
        roomFull = on;
        ShowRoomButton();
        var shift = column.RenderTransform as TranslateTransform ?? new TranslateTransform();
        column.RenderTransform = shift;
        var target = on || roomHidden ? 0 : RoomColumn.PaneWidth;
        if (!Motion.Enabled)
        {
            Root.ColumnDefinitions[1].Width = new GridLength(target);
            column.Opacity = on ? 0 : 1;
            shift.X = 0;
            PlaceRoomStrip();
            return;
        }
        var from = Root.ColumnDefinitions[1].ActualWidth;
        var fromOpacity = column.Opacity;
        if (!on)
            PlaceRoomStrip();
        Animate(Motion.Standard, Curves.Spring, (_, m) =>
        {
            Root.ColumnDefinitions[1].Width = new GridLength(Math.Max(0, from + (target - from) * m));
            column.Opacity = on ? fromOpacity * (1 - m) : fromOpacity + (1 - fromOpacity) * m;
            shift.X = on ? 24 * m : 24 * (1 - m);
        }, () =>
        {
            shift.X = 0;
            if (on)
                PlaceRoomStrip();
        });
    }

    /// <summary>
    /// The strip: only in full screen (or with the column hidden), while anyone isn't watching
    /// and for 8 s after any change, and with the controls. Below the top bar while it shows.
    /// </summary>
    void PlaceRoomStrip()
    {
        if (roomStrip is not { } strip || roomScene is not { } scene)
            return;
        var wanted = (roomFull || roomHidden) && (chromeShown || Rooms.StripShows(scene, roomChangedAt));
        strip.Margin = new Thickness(0, chromeShown ? 64 : 16, 16, 0);
        if (wanted == stripWanted)
            return;
        stripWanted = wanted;
        // Fades both ways, which Reduce Motion keeps: the strip never slides.
        if (wanted)
        {
            strip.Visibility = Visibility.Visible;
            Motion.FadeTo(strip, 1, Motion.FadeInTime);
        }
        else
            Motion.FadeOutThen(strip, () =>
            {
                if (!stripWanted)
                    strip.Visibility = Visibility.Collapsed;
            });
    }

    bool stripWanted;

    // Presses that belong to the room.

    /// <summary>Play during a wait is Carry on; before play it is Play for everyone. True when the room took it.</summary>
    bool RoomTakesPlay()
    {
        if (roomScene is not { } scene)
            return false;
        if ((scene.View.Waiting ?? []).Length == 0 && !scene.View.BeforePlay)
            return false;
        SendRoom(new RoomCommand.Play(true));
        return true;
    }

    /// <summary>Skip opening skips for the room, and the chat says who.</summary>
    bool RoomTakesSkip()
    {
        if (roomScene is null)
            return false;
        SendRoom(new RoomCommand.SkipOpening(position));
        return true;
    }

    /// <summary>Closing the player in a room asks: Leave, Keep the room, or Cancel.</summary>
    async void CloseOrAsk()
    {
        if (roomScene is null || roomScene.Invited is not null || XamlRoot is not { } root)
        {
            Close();
            return;
        }
        switch (await RoomPopups.AskClose(root))
        {
            case RoomCloseAnswer.Leave:
                SendRoom(new RoomCommand.Leave());
                Close();
                break;
            case RoomCloseAnswer.KeepRoom:
                SendRoom(new RoomCommand.ClosePlayer());
                Close();
                break;
        }
    }

    // Invite and Details.

    void ShowRoomInvite(bool moveHighlight = false)
    {
        if (ctx is not { } c || roomColumn is not { } column || roomScene?.View.Link is not { } link)
            return;
        var flyout = RoomPopups.Invite(c, link, () => SendRoom(new RoomCommand.NewInvite()), () => _ = AskRoomMove(), moveHighlight);
        flyout.Opened += OnMenuOpened;
        flyout.Closed += OnMenuClosed;
        flyout.ShowAt(column.InviteAnchor);
    }

    void ShowRoomDetails(bool editing = false)
    {
        if (roomColumn is not { } column || roomScene is not { } scene)
            return;
        var flyout = RoomPopups.Details(scene.View.Relays, relays => SendRoom(new RoomCommand.EditRelays(relays)), editing);
        flyout.Opened += OnMenuOpened;
        flyout.Closed += OnMenuClosed;
        flyout.ShowAt(column.DetailsAnchor);
    }

    async Task AskRoomMove(Action<ContentDialog>? shown = null)
    {
        if (roomScene is not { } scene || XamlRoot is not { } root)
            return;
        var me = scene.Members.FirstOrDefault(m => m.You)?.Name ?? scene.View.Name;
        if (await RoomPopups.AskMove(root, me, scene.View.MoveCandidates, Rooms.Clock(scene.Position), shown) is { } take)
            SendRoom(new RoomCommand.MoveRoom(take));
    }

    // Up next, for the room.

    void ShowRoomUpNext(RoomScene scene)
    {
        if (scene.View.UpNext is not { CancelledBy: null } next || (scene.View.Waiting ?? []).Length > 0)
        {
            if (roomUpNextShown)
            {
                roomUpNextShown = false;
                UpNextCard.Visibility = Visibility.Collapsed;
                UpNextHead.Text = T.PlayerUpNext;
            }
            return;
        }
        roomUpNextShown = true;
        UpNextHead.Text = RoomWords.UpNextForRoom;
        var episode = next.Episode;
        UpNextTitle.Text = episode.Name is { Length: > 0 } name ? T.RoomUpNextEpisode(RoomWords.N(episode.Episode), name) : T.RoomUpNextEpisodePlain(RoomWords.N(episode.Episode));
        // The count, or while someone's file still plays, who the card waits for.
        UpNextLine.Text = next.EndsAtMs is { } ends
            ? RoomWords.PlaysInSeconds(Math.Max(0, (ends - scene.NowMs + 999) / 1000))
            : RoomWords.UpNextWaiting(Rooms.Names(next.WaitingFor ?? []));
        UpNextBar.Visibility = Visibility.Collapsed;
        if (UpNextCard.Visibility != Visibility.Visible)
        {
            UpNextCard.Visibility = Visibility.Visible;
            Motion.FadeIn(UpNextCard);
        }
    }

    bool roomUpNextShown;

    bool RoomTakesNext(bool cancel)
    {
        if (roomScene is null)
            return false;
        SendRoom(cancel ? new RoomCommand.CancelUpNext() : new RoomCommand.PlayNextNow());
        return true;
    }

    // The video area's panels.

    void BuildRoomPanel()
    {
        if (roomPanel is not { } panel)
        {
            if (roomPanelLayer is { } old)
            {
                roomPanelLayer = null;
                Motion.FadeOutThen(old, () => Root.Children.Remove(old));
            }
            return;
        }
        var layer = roomPanelLayer;
        if (layer is null)
        {
            layer = new Grid { Background = new SolidColorBrush(Colors.Black), RequestedTheme = ElementTheme.Dark };
            Grid.SetColumn(layer, 0);
            roomPanelLayer = layer;
            // Over the video, under the column and the strip.
            Root.Children.Insert(Math.Max(1, Root.Children.IndexOf(VideoPanel) + 1), layer);
            Motion.FadeIn(layer);
        }
        layer.Children.Clear();
        if (StartingArt.Source is { } art)
            layer.Children.Add(new Image { Source = art, Stretch = Stretch.UniformToFill });
        var shade = new LinearGradientBrush { StartPoint = new(0, 1), EndPoint = new(0, 0) };
        var deep = panel.Kind is RoomPanelKind.NoSource or RoomPanelKind.SelfLoading;
        shade.GradientStops.Add(new GradientStop { Color = Color.FromArgb(0xEB, 0x0B, 0x0D, 0x18), Offset = 0 });
        shade.GradientStops.Add(new GradientStop { Color = Color.FromArgb(deep ? (byte)0xD0 : (byte)0x8C, 0x0B, 0x0D, 0x18), Offset = 0.6 });
        shade.GradientStops.Add(new GradientStop { Color = Color.FromArgb(deep ? (byte)0xB8 : (byte)0x59, 0x0B, 0x0D, 0x18), Offset = 1 });
        layer.Children.Add(new Microsoft.UI.Xaml.Shapes.Rectangle { Fill = shade });
        var content = panel.Kind == RoomPanelKind.SelfLoading ? SelfLoadingContent(panel) : PanelContent(panel);
        layer.Children.Add(content);
    }

    static TextBlock Words(string text, double size, bool strong = false, bool haze = false, double line = 0) => new()
    {
        Text = text,
        FontSize = size,
        LineHeight = line,
        FontWeight = strong ? FontWeights.SemiBold : FontWeights.Normal,
        Foreground = (Brush)Application.Current.Resources[haze ? "HazeBrush" : "SilkBrush"],
        TextWrapping = TextWrapping.WrapWholeWords,
    };

    FrameworkElement PanelContent(RoomPanel panel)
    {
        var at = new StackPanel { Margin = new Thickness(56, 0, 56, 56), VerticalAlignment = VerticalAlignment.Bottom, HorizontalAlignment = HorizontalAlignment.Left, MaxWidth = 640 };
        switch (panel.Kind)
        {
            case RoomPanelKind.Before or RoomPanelKind.Invited:
                at.Children.Add(Words(panel.Kind == RoomPanelKind.Invited ? RoomWords.RoomIsWatching : RoomWords.UpInTheRoom, 13, strong: true, haze: true));
                var title = new TextBlock { Text = panel.Show, Style = (Style)Application.Current.Resources["ShowTitleDisplayStyle"], Margin = new Thickness(0, 6, 0, 0), TextAlignment = TextAlignment.Left };
                at.Children.Add(title);
                var ep = Words(RoomWords.EpisodeLength(panel.Episode, panel.EpisodeName, panel.Minutes), 17);
                ep.Margin = new Thickness(0, 4, 0, 0);
                at.Children.Add(ep);
                var plays = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 10, 0, 0) };
                plays.Children.Add(new Microsoft.UI.Xaml.Shapes.Ellipse { Width = 7, Height = 7, Fill = (Brush)Application.Current.Resources["WaterBrush"], VerticalAlignment = VerticalAlignment.Center });
                plays.Children.Add(Words(RoomWords.PlaysFromYourSources, 14, haze: true));
                at.Children.Add(plays);
                if (panel.Kind == RoomPanelKind.Before && !panel.Pressed && roomScene is { Start: RoomStart.Before } && (roomScene.View.Waiting ?? []).Length == 0)
                {
                    var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12, Margin = new Thickness(0, 22, 0, 0) };
                    var play = new Button
                    {
                        Style = (Style)Application.Current.Resources["AccentButtonStyle"],
                        Height = 40,
                        Padding = new Thickness(18, 0, 20, 0),
                        Content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Children = { new FontIcon { Glyph = "", FontSize = 16 }, new TextBlock { Text = RoomWords.PlayForEveryone, FontSize = 15 } } },
                    };
                    AutomationProperties.SetName(play, RoomWords.PlayForEveryone);
                    play.Click += (_, _) => SendRoom(new RoomCommand.Play(true));
                    row.Children.Add(play);
                    var note = Words(RoomWords.AnyoneCanStart, 13, haze: true);
                    note.VerticalAlignment = VerticalAlignment.Center;
                    row.Children.Add(note);
                    at.Children.Add(row);
                }
                break;
            case RoomPanelKind.Choosing:
                at.MaxWidth = 520;
                at.Children.Add(new TextBlock { Text = panel.Show, Style = (Style)Application.Current.Resources["ShowTitleStyle"], FontSize = 28 });
                at.Children.Add(Words(RoomWords.ChooseSourceFor(panel.Episode), 15));
                var keeps = Words(RoomWords.RoomKeepsWatching, 13, haze: true);
                keeps.Margin = new Thickness(0, 4, 0, 0);
                at.Children.Add(keeps);
                var list = new ListView { SelectionMode = ListViewSelectionMode.Single, Margin = new Thickness(0, 14, 0, 0), Background = (Brush)Application.Current.Resources["RaisedBrush"], CornerRadius = new CornerRadius(8) };
                AutomationProperties.SetName(list, RoomWords.ChooseSourceFor(panel.Episode));
                // The app's own list of what each source has for the episode, loading in place.
                var found = roomSources ?? [];
                if (roomSources is null)
                    at.Children.Add(new ProgressRing { IsActive = true, Width = 20, Height = 20, HorizontalAlignment = HorizontalAlignment.Left, Margin = new Thickness(0, 14, 0, 0) });
                foreach (var s in found)
                {
                    var item = new StackPanel { Padding = new Thickness(2, 8, 2, 8) };
                    item.Children.Add(Words(s.Title, 14, strong: true));
                    item.Children.Add(Words(ReleasePicker.Detail(s), 12, haze: true));
                    list.Items.Add(new ListViewItem { Content = item });
                }
                list.SelectedIndex = ReleasePicker.Preselected(found);
                if (found.Length > 0)
                    at.Children.Add(list);
                var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10, Margin = new Thickness(0, 14, 0, 0) };
                var inRoom = new Button { Content = RoomWords.PlayInRoom, Style = (Style)Application.Current.Resources["AccentButtonStyle"], Height = 36, IsEnabled = list.SelectedIndex >= 0 };
                list.SelectionChanged += (_, _) => inRoom.IsEnabled = list.SelectedIndex >= 0;
                inRoom.Click += (_, _) =>
                {
                    if (list.SelectedIndex >= 0 && list.SelectedIndex < found.Length)
                        RoomSourcePicked?.Invoke(found[list.SelectedIndex]);
                };
                var notNow = new Button { Content = RoomWords.NotNow, Height = 36 };
                notNow.Click += (_, _) => SendRoom(new RoomCommand.ChoosingSource(false));
                buttons.Children.Add(inRoom);
                buttons.Children.Add(notNow);
                at.Children.Add(buttons);
                break;
            case RoomPanelKind.NoSource:
                at.MaxWidth = 560;
                at.Children.Add(new TextBlock { Text = panel.Show, Style = (Style)Application.Current.Resources["ShowTitleStyle"], FontSize = 28 });
                var none = Words(RoomWords.NoneHas(panel.Episode), 17, strong: true);
                none.Margin = new Thickness(0, 6, 0, 0);
                at.Children.Add(none);
                var looked = Words(RoomWords.LookedIn(panel.SourceCount), 14, haze: true, line: 20);
                looked.Margin = new Thickness(0, 4, 0, 0);
                at.Children.Add(looked);
                var fix = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10, Margin = new Thickness(0, 16, 0, 0) };
                var open = new Button { Content = RoomWords.OpenSources, Style = (Style)Application.Current.Resources["AccentButtonStyle"], Height = 36 };
                open.Click += (_, _) => RoomGo?.Invoke("sources");
                var search = new Button
                {
                    Height = 36,
                    Content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Children = { new FontIcon { Glyph = "", FontSize = 14 }, new TextBlock { Text = RoomWords.SearchTitles } } },
                };
                AutomationProperties.SetName(search, RoomWords.SearchTitles);
                search.Click += (_, _) => RoomGo?.Invoke("search");
                fix.Children.Add(open);
                fix.Children.Add(search);
                at.Children.Add(fix);
                break;
        }
        return at;
    }

    /// <summary>Your own episode opening while the room waits: today's starting screen says so, with Don't wait for me.</summary>
    FrameworkElement SelfLoadingContent(RoomPanel panel)
    {
        var at = new StackPanel { HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center, Spacing = 4 };
        var title = new TextBlock { Text = panel.Show, Style = (Style)Application.Current.Resources["ShowTitleStyle"], FontSize = 22, HorizontalAlignment = HorizontalAlignment.Center };
        at.Children.Add(title);
        var ep = Words(RoomWords.StartingEpisode(panel.Episode), 15);
        ep.HorizontalAlignment = HorizontalAlignment.Center;
        at.Children.Add(ep);
        var secs = roomScene is { } s && (s.View.Waiting ?? []).FirstOrDefault(w => w.You) is { } mine ? Rooms.Seconds(mine.SinceMs, s.NowMs) : 0;
        var note = Words(RoomWords.RoomPausedUntil(secs), 13, haze: true);
        note.HorizontalAlignment = HorizontalAlignment.Center;
        note.Margin = new Thickness(0, 6, 0, 0);
        Typography.SetNumeralAlignment(note, FontNumeralAlignment.Tabular);
        at.Children.Add(note);
        if (roomScene?.View.CanDontWait == true)
        {
            var release = new Button { Content = RoomWords.DontWait, HorizontalAlignment = HorizontalAlignment.Center, Margin = new Thickness(0, 12, 0, 0) };
            release.Click += (_, _) => SendRoom(new RoomCommand.DontWait());
            at.Children.Add(release);
        }
        return at;
    }
}

/// <summary>The board's curves, as functions of progress 0..1.</summary>
static class Curves
{
    /// <summary>The system's window curve: cubic-bezier(0.42, 0, 0.58, 1).</summary>
    public static double System(double p) => Bezier(0.42, 0, 0.58, 1, p);

    /// <summary>Fade-in: cubic-bezier(0, 0, 0.58, 1).</summary>
    public static double FadeIn(double p) => Bezier(0, 0, 0.58, 1, p);

    /// <summary>Standard: the app's spring.</summary>
    public static double Spring(double p) => Motion.SpringAt(p);

    /// <summary>A CSS cubic bezier from (0,0) to (1,1), solved for x by bisection.</summary>
    public static double Bezier(double x1, double y1, double x2, double y2, double p)
    {
        if (p <= 0)
            return 0;
        if (p >= 1)
            return 1;
        double lo = 0, hi = 1, t = p;
        for (var i = 0; i < 30; i++)
        {
            t = (lo + hi) / 2;
            var x = 3 * (1 - t) * (1 - t) * t * x1 + 3 * (1 - t) * t * t * x2 + t * t * t;
            if (x < p)
                lo = t;
            else
                hi = t;
        }
        return 3 * (1 - t) * (1 - t) * t * y1 + 3 * (1 - t) * t * t * y2 + t * t * t;
    }
}

#if KAERI_TESTING
public sealed partial class PlayerView
{
    /// <summary>A shot's: the controls gone, as they are when the pointer rests.</summary>
    internal void ShotHideControls() => HideChrome();

    /// <summary>A shot's: the video paused or playing, as the room is.</summary>
    internal void ShotPaused(bool on)
    {
        if (paused != on)
            TogglePause();
    }

    internal void ShotRoomInvite(bool moveHighlight) => ShowRoomInvite(moveHighlight);

    internal void ShotRoomDetails(bool editing) => ShowRoomDetails(editing);

    internal async void ShotRoomLeave()
    {
        if (XamlRoot is { } root && await RoomPopups.AskLeave(root))
            SendRoom(new RoomCommand.Leave());
    }

    internal void ShotRoomClose() => CloseOrAsk();

    internal void ShotRoomMove() => _ = AskRoomMove();
}
#endif
