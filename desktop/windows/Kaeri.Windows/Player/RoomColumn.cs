using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.System;
using Windows.UI;

namespace Kaeri.Windows.Player;

/// <summary>
/// The room beside the video (board v5, the Windows frame): a 336 epx pane, not glass. Its
/// header names it and holds Invite, Details and Leave; the room's line says what is happening
/// with the one button it may ask for; the people follow, a face and a word each; then the chat,
/// and the field with Send. Every change of the line is a crossfade of the line over itself;
/// rows fade in where they join, and a row that leaves stays dimmed for 4 s before the rows
/// below close the gap.
/// </summary>
sealed class RoomColumn : Grid
{
    public const double PaneWidth = 336;

    static readonly Color Pane = Color.FromArgb(0xFF, 0x1B, 0x1E, 0x2C);
    static readonly Color Edge = Color.FromArgb(0x14, 0xE4, 0xE0, 0xD6);

    readonly StackPanel lineA = new(), lineB = new() { Opacity = 0 };
    readonly Grid lineHost = new();
    readonly StackPanel asks = new() { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 10, 0, 0), Visibility = Visibility.Collapsed };
    readonly Button askButton = new() { Style = (Style)Application.Current.Resources["AccentButtonStyle"], Height = 32, Padding = new Thickness(12, 0, 12, 0) };
    readonly Button secondButton = new() { Height = 32, Padding = new Thickness(12, 0, 12, 0) };
    readonly StackPanel people = new();
    readonly ScrollViewer peopleScroll;
    readonly StackPanel chat = new() { Spacing = 6, VerticalAlignment = VerticalAlignment.Bottom };
    readonly ScrollViewer chatScroll;
    readonly TextBox entry = new() { PlaceholderText = RoomWords.SaySomething, MaxLength = (int)Chat.MaxLength, Height = 32, VerticalAlignment = VerticalAlignment.Center };
    readonly TextBlock tail = new()
    {
        FontSize = 12,
        HorizontalAlignment = HorizontalAlignment.Right,
        VerticalAlignment = VerticalAlignment.Center,
        Margin = new Thickness(0, 0, 10, 0),
        IsHitTestVisible = false,
        Visibility = Visibility.Collapsed,
    };
    readonly Grid foot;
    /// <summary>What the room allows the chat now: its length and the wait before the next send.</summary>
    RoomChatState? chatState;
    bool joined;
    readonly Button send = new() { Content = RoomWords.Send, Height = 32, Padding = new Thickness(12, 0, 12, 0) };
    readonly Button inviteButton, detailsButton, leaveButton;
    readonly Dictionary<string, PersonRow> rows = [];
    /// <summary>People whose Left row has closed, while the room still lists them as Left.</summary>
    readonly HashSet<string> closed = [];
    readonly TextBlock announcer = new() { Width = 1, Height = 1, Opacity = 0 };
    StackPanel shownLine;
    string lineKind = "";
    RoomAsk ask;
    bool asking;

    public event Action<RoomAsk>? Asked;
    public event Action? Declined;
    public event Action? InviteClicked;
    public event Action? DetailsClicked;
    public event Action? LeaveClicked;
    public event Action<string>? Said;
    public event Action<string, bool>? MuteAsked;

    /// <summary>Where Invite and Details hang their flyouts.</summary>
    public FrameworkElement InviteAnchor => inviteButton;
    public FrameworkElement DetailsAnchor => detailsButton;

    /// <summary>The field, for the player's C key.</summary>
    public TextBox Field => entry;

    public RoomColumn()
    {
        Width = PaneWidth;
        Background = new SolidColorBrush(Pane);
        BorderBrush = new SolidColorBrush(Edge);
        BorderThickness = new Thickness(1, 0, 0, 0);
        // The window's caption buttons sit over the column's top (the title bar is tall, 48 epx): the header starts under them.
        Padding = new Thickness(16, 48, 16, 0);
        RequestedTheme = ElementTheme.Dark;
        AutomationProperties.SetName(this, RoomWords.Title);
        AutomationProperties.SetLandmarkType(this, AutomationLandmarkType.Custom);
        AutomationProperties.SetLocalizedLandmarkType(this, RoomWords.Title);
        foreach (var h in new[] { GridLength.Auto, GridLength.Auto, GridLength.Auto, GridLength.Auto, new GridLength(1, GridUnitType.Star), GridLength.Auto })
            RowDefinitions.Add(new RowDefinition { Height = h });

        // The header: the room's name, then Invite, Details and Leave, always in view.
        var head = new Grid { Height = 52, ColumnSpacing = 6 };
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var title = new TextBlock { Text = RoomWords.Title, FontSize = 14, FontWeight = FontWeights.SemiBold, VerticalAlignment = VerticalAlignment.Center, TextTrimming = TextTrimming.CharacterEllipsis };
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level2);
        head.Children.Add(title);
        inviteButton = HeadIcon("", RoomWords.Invite, 1, () => InviteClicked?.Invoke());
        detailsButton = HeadIcon("", RoomWords.Details, 2, () => DetailsClicked?.Invoke());
        head.Children.Add(inviteButton);
        head.Children.Add(detailsButton);
        leaveButton = new Button
        {
            Content = RoomWords.Leave,
            Height = 32,
            Padding = new Thickness(12, 0, 12, 0),
            VerticalAlignment = VerticalAlignment.Center,
        };
        leaveButton.Click += (_, _) => LeaveClicked?.Invoke();
        Grid.SetColumn(leaveButton, 3);
        head.Children.Add(leaveButton);
        Children.Add(head);

        // The room's line: what is happening, in words, crossfading over itself.
        var lineBlock = new StackPanel { Padding = new Thickness(0, 4, 0, 12) };
        foreach (var line in new[] { lineA, lineB })
        {
            line.Children.Add(new TextBlock { FontSize = 20, LineHeight = 26, FontWeight = FontWeights.SemiBold, TextWrapping = TextWrapping.WrapWholeWords });
            var detail = new TextBlock { FontSize = 13, LineHeight = 18, TextWrapping = TextWrapping.WrapWholeWords, Foreground = Haze };
            Typography.SetNumeralAlignment(detail, FontNumeralAlignment.Tabular);
            line.Children.Add(detail);
            lineHost.Children.Add(line);
        }
        shownLine = lineA;
        lineBlock.Children.Add(lineHost);
        askButton.Click += (_, _) => Asked?.Invoke(ask);
        secondButton.Click += (_, _) => Declined?.Invoke();
        asks.Children.Add(askButton);
        asks.Children.Add(secondButton);
        lineBlock.Children.Add(asks);
        AutomationProperties.SetLiveSetting(announcer, AutomationLiveSetting.Polite);
        lineBlock.Children.Add(announcer);
        Grid.SetRow(lineBlock, 1);
        Children.Add(lineBlock);

        // The people: up to five rows in view (three in a short window), then they scroll.
        peopleScroll = new ScrollViewer
        {
            Content = people,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            MaxHeight = 5 * 44,
            Padding = new Thickness(0, 0, 0, 10),
        };
        AutomationProperties.SetName(people, RoomWords.PeopleList);
        Grid.SetRow(peopleScroll, 2);
        Children.Add(peopleScroll);

        var rule = new Border { Height = 1, Background = (Brush)Application.Current.Resources["FaintRuleBrush"] };
        Grid.SetRow(rule, 3);
        Children.Add(rule);

        // The chat, newest at the bottom.
        chatScroll = new ScrollViewer
        {
            Content = chat,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            Padding = new Thickness(0, 12, 0, 12),
        };
        Grid.SetRow(chatScroll, 4);
        Children.Add(chatScroll);
        chat.SizeChanged += (_, _) => chatScroll.ChangeView(null, chatScroll.ScrollableHeight, null, true);

        foot = new Grid { ColumnSpacing = 8, Padding = new Thickness(0, 0, 0, 14) };
        foot.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        foot.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        AutomationProperties.SetName(entry, RoomWords.SaySomething);
        entry.KeyDown += (_, e) =>
        {
            if (e.Key == VirtualKey.Enter)
            {
                e.Handled = true;
                Send();
            }
        };
        // The field's end holds the characters left from 20, or the wait before the next send.
        Typography.SetNumeralAlignment(tail, FontNumeralAlignment.Tabular);
        tail.Foreground = Haze;
        entry.TextChanged += (_, _) => ShowLimits();
        // The field's end holds the count and the wait, so the template's clear button stays out.
        entry.Loaded += (_, _) =>
        {
            if (Kaeri.Windows.Pages.ChatField.Find(entry, "DeleteButton") is Button clear)
            {
                clear.Width = 0;
                clear.MinWidth = 0;
                clear.Opacity = 0;
                clear.IsTabStop = false;
                clear.IsHitTestVisible = false;
            }
        };
        var field = new Grid();
        field.Children.Add(entry);
        field.Children.Add(tail);
        foot.Children.Add(field);
        send.Click += (_, _) => Send();
        Grid.SetColumn(send, 1);
        foot.Children.Add(send);
        Grid.SetRow(foot, 5);
        Children.Add(foot);
        SizeChanged += (_, _) => peopleScroll.MaxHeight = (XamlRoot?.Size.Height ?? ActualHeight) < 680 ? 3 * 44 : 5 * 44;
    }

    static Brush Haze => (Brush)Application.Current.Resources["HazeBrush"];

    static Button HeadIcon(string glyph, string name, int column, Action click)
    {
        var button = new Button
        {
            Width = 32,
            Height = 32,
            Padding = new Thickness(0),
            Content = new FontIcon { Glyph = glyph, FontSize = 16 },
            VerticalAlignment = VerticalAlignment.Center,
        };
        ToolTipService.SetToolTip(button, name);
        AutomationProperties.SetName(button, name);
        button.Click += (_, _) => click();
        Grid.SetColumn(button, column);
        return button;
    }

    void Send()
    {
        var text = entry.Text.Trim();
        if (text.Length == 0 || (chatState?.WaitSeconds ?? 0) > 0)
            return;
        entry.Text = "";
        Said?.Invoke(text);
    }

    /// <summary>The room as it is now. Called on every change and once a second while anything counts.</summary>
#if KAERI_TESTING
    /// <summary>For the shots: the chat field focused, holding <paramref name="text"/>.</summary>
    internal void ShotType(string text)
    {
        entry.Focus(FocusState.Programmatic);
        entry.Text = text;
        entry.SelectionStart = text.Length;
    }
#endif

    public void Show(RoomScene scene, RoomConversation? conversation, RoomChatState? state, bool chatHidden)
    {
        ShowLine(Rooms.Line(scene));
        ShowPeople(scene);
        ShowChat(conversation);
        // Before joining, the chat waits for the room.
        joined = scene.Invited is null;
        inviteButton.IsEnabled = detailsButton.IsEnabled = joined;
        ShowChatState(state, chatHidden);
    }

    /// <summary>
    /// Hide chat (the app's setting) takes the chat and its field away; the room and its people
    /// stay. Otherwise the field takes the room's length, and Send waits while the room says to.
    /// </summary>
    public void ShowChatState(RoomChatState? state, bool chatHidden)
    {
        chatState = state;
        var shown = chatHidden ? Visibility.Collapsed : Visibility.Visible;
        if (chatScroll.Visibility != shown)
            chatScroll.Visibility = foot.Visibility = shown;
        ShowLimits();
    }

    void ShowLimits()
    {
        var max = chatState?.MaxLength ?? Chat.MaxLength;
        if (entry.MaxLength != (int)max)
            entry.MaxLength = (int)max;
        var wait = chatState?.WaitSeconds ?? 0;
        entry.IsEnabled = joined;
        send.IsEnabled = joined && wait == 0;
        var text = Chat.Tail(entry.Text, max, wait);
        tail.Text = text ?? "";
        tail.Visibility = text is null ? Visibility.Collapsed : Visibility.Visible;
        entry.Padding = new Thickness(10, 5, text is null ? 10 : 20 + text.Length * 7, 6);
        // Said once to a screen reader: the count at 20 left, and a wait as it starts.
        var left = Math.Max(0, (int)max - entry.Text.Length);
        if (left <= Chat.CountFrom && entry.Text.Trim().Length > 0)
        {
            if (!toldLeft)
                Tell(T.RoomChatCharactersLeft(left));
            toldLeft = true;
        }
        else
            toldLeft = false;
        if (wait > 0)
        {
            if (!toldWait)
                Tell(T.RoomChatWaitSpoken(wait));
            toldWait = true;
        }
        else
            toldWait = false;
    }

    bool toldLeft;
    bool toldWait;

    void Tell(string text) =>
        FrameworkElementAutomationPeer.FromElement(entry)?.RaiseNotificationEvent(
            AutomationNotificationKind.Other, AutomationNotificationProcessing.ImportantMostRecent, text, "room-chat");

    void ShowLine(RoomLine line)
    {
        var changed = line.Kind != lineKind;
        var target = changed && lineKind.Length > 0 ? (shownLine == lineA ? lineB : lineA) : shownLine;
        Set(target, line);
        if (changed && lineKind.Length > 0)
        {
            // The new line fades in over the old, which fades out under it: never empty.
            var old = shownLine;
            shownLine = target;
            Canvas.SetZIndex(target, 1);
            Canvas.SetZIndex(old, 0);
            // A fade, so it stays with Reduce Motion: nothing here moves.
            Motion.FadeTo(target, 1, Motion.FadeInTime);
            Motion.FadeTo(old, 0, Motion.FadeOutTime);
            announcer.Text = $"{line.Title}. {line.Detail}";
        }
        else if (lineKind.Length == 0)
            target.Opacity = 1;
        lineKind = line.Kind;
        // Only the line on top is read out and takes space in the reading order.
        foreach (var l in new[] { lineA, lineB })
            AutomationProperties.SetAccessibilityView(l, l == shownLine ? AccessibilityView.Content : AccessibilityView.Raw);

        var wasAsking = asks.Visibility == Visibility.Visible;
        ask = line.Ask;
        if (line.Ask == RoomAsk.None)
        {
            asking = false;
            if (wasAsking && asks.Opacity > 0)
                Motion.FadeOutThen(asks, () =>
                {
                    if (ask == RoomAsk.None)
                        asks.Visibility = Visibility.Collapsed;
                });
            return;
        }
        if (askButton.Content as string != line.AskText)
            askButton.Content = line.AskText;
        secondButton.Visibility = line.SecondText is null ? Visibility.Collapsed : Visibility.Visible;
        if (line.SecondText is { } second && secondButton.Content as string != second)
            secondButton.Content = second;
        if (!asking)
        {
            asks.Visibility = Visibility.Visible;
            asks.Opacity = 0;
            Motion.FadeTo(asks, 1, Motion.FadeInTime);
        }
        asking = true;
    }

    static void Set(StackPanel line, RoomLine words)
    {
        if (line.Children[0] is TextBlock title && title.Text != words.Title)
            title.Text = words.Title;
        if (line.Children[1] is TextBlock detail && detail.Text != words.Detail)
            detail.Text = words.Detail;
    }

    /// <summary>The rows by id, kept as elements so a row that only changes its word keeps its place and focus.</summary>
    void ShowPeople(RoomScene scene)
    {
        var now = Rooms.Rows(scene);
        var seen = new HashSet<string>();
        var index = 0;
        foreach (var r in now.Where(r => !(r.Left && closed.Contains(r.Id))))
        {
            seen.Add(r.Id);
            // Back after leaving: a rejoin gets its row again.
            if (!r.Left)
                closed.Remove(r.Id);
            if (!rows.TryGetValue(r.Id, out var row))
            {
                row = new PersonRow(this, r);
                rows[r.Id] = row;
                people.Children.Insert(Math.Min(index, people.Children.Count), row.Element);
                row.Enter(IsLoaded);
            }
            // Back within the 4 s a leaving row holds: it stays, live, where it was.
            if (!r.Left && row.Leaving)
                row.Stay();
            row.Show(r);
            // Rows keep their place: a row that leaves holds its gap until it closes.
            index = people.Children.IndexOf(row.Element) + 1;
        }
        // Someone gone from the list: their row reads Left, dimmed, for 4 s, then closes.
        foreach (var (id, row) in rows.ToList())
        {
            if (seen.Contains(id) || row.Leaving)
                continue;
            row.ShowLeft();
            row.Leave(() =>
            {
                people.Children.Remove(row.Element);
                rows.Remove(id);
            });
        }
        foreach (var r in now.Where(r => r.Left))
            if (rows.TryGetValue(r.Id, out var row) && !row.Leaving)
                row.Leave(() =>
                {
                    people.Children.Remove(row.Element);
                    rows.Remove(r.Id);
                    closed.Add(r.Id);
                });
    }

    void ShowChat(RoomConversation? conversation)
    {
        var lines = conversation?.Lines ?? [];
        // Lines only ever come at the end, each with its id; a new room starts over.
        if (lines.Length == 0 ? chatLastId > 0 : lines[^1].Id < chatLastId)
        {
            chat.Children.Clear();
            chatLastId = 0;
            failedLine = null;
        }
        var fresh = chatLastId > 0;
        foreach (var line in lines)
        {
            if (line.Id <= chatLastId)
                continue;
            var block = ChatLine(line);
            chat.Children.Add(block);
            if (fresh)
            {
                block.Opacity = 0;
                Motion.FadeTo(block, 1, Motion.FadeInTime);
            }
            chatLastId = line.Id;
        }
        // A line no relay echoed: Couldn't send, with Try again, last.
        if (failedLine is { } old)
            chat.Children.Remove(old);
        failedLine = null;
        if (conversation?.Failed is { } failed)
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            row.Children.Add(new TextBlock { Text = RoomWords.NotSent, FontSize = 13, Foreground = (Brush)Application.Current.Resources["SystemFillColorCriticalBrush"], VerticalAlignment = VerticalAlignment.Center });
            var again = new HyperlinkButton { Content = RoomWords.TryAgain, Padding = new Thickness(4, 0, 4, 0) };
            again.Click += (_, _) => Said?.Invoke(failed);
            row.Children.Add(again);
            failedLine = row;
            chat.Children.Add(row);
        }
    }

    ulong chatLastId;
    FrameworkElement? failedLine;

    static FrameworkElement ChatLine(Kaeri.Engine.Model.RoomChatLine line)
    {
        var text = new TextBlock { FontSize = 14, LineHeight = 20, TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true };
        Typography.SetNumeralAlignment(text, FontNumeralAlignment.Tabular);
        if (line.Muted && line.Body is not RoomChatBody.Room)
        {
            text.Text = T.RoomChatMuted(line.From);
            text.FontSize = 13;
            text.Foreground = Haze;
            return text;
        }
        switch (line.Body)
        {
            case RoomChatBody.Room room:
                text.Text = room.Message.Text;
                text.FontSize = 13;
                text.Foreground = Haze;
                return text;
            case RoomChatBody.Said said:
                var name = new Run { Text = line.From, FontWeight = FontWeights.SemiBold };
                if (line.You)
                    name.Foreground = Haze;
                text.Inlines.Add(name);
                text.Inlines.Add(new Run { Text = "  " + said.Text });
                return text;
            case RoomChatBody.Reacted reacted:
                var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
                row.Children.Add(ReactionGlyph.Make(reacted.Reaction, 17, Haze));
                text.Text = Chat.ReactionLine(line) ?? "";
                text.FontSize = 13;
                text.Foreground = Haze;
                row.Children.Add(text);
                return row;
        }
        return text;
    }

    /// <summary>Moves keyboard focus to the line's button, if it asks anything.</summary>
    public bool FocusAsk() => asks.Visibility == Visibility.Visible && askButton.Focus(FocusState.Keyboard);

    internal void Mute(string name, bool on) => MuteAsked?.Invoke(name, on);

    /// <summary>A person's row: the face, the name with "(you)", the word. Mute from its context menu.</summary>
    sealed class PersonRow
    {
        readonly RoomColumn column;
        public Grid Element { get; }
        readonly RoomFace face = new(28);
        readonly TextBlock name = new() { FontSize = 14, LineHeight = 18, FontWeight = FontWeights.SemiBold, TextTrimming = TextTrimming.CharacterEllipsis, TextWrapping = TextWrapping.NoWrap };
        readonly Run you = new();
        readonly TextBlock word = new() { FontSize = 12, LineHeight = 16, TextTrimming = TextTrimming.CharacterEllipsis, TextWrapping = TextWrapping.NoWrap };
        bool? muted;
        public bool Leaving { get; private set; }

        public PersonRow(RoomColumn column, RoomRow r)
        {
            this.column = column;
            Element = new Grid { Height = 44, ColumnSpacing = 12, Tag = r.Id, RenderTransform = new TranslateTransform() };
            Element.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            Element.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            face.VerticalAlignment = VerticalAlignment.Center;
            Element.Children.Add(face);
            var names = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
            Typography.SetNumeralAlignment(word, FontNumeralAlignment.Tabular);
            names.Children.Add(name);
            names.Children.Add(word);
            Grid.SetColumn(names, 1);
            Element.Children.Add(names);
            if (r.You)
            {
                name.Inlines.Add(new Run { Text = r.Name });
                you.Foreground = Haze;
                you.FontWeight = FontWeights.Normal;
                you.Text = " " + RoomWords.YouMark;
                name.Inlines.Add(you);
            }
            else
                name.Text = r.Name;
        }

        string shownName = "";

        public void ShowLeft()
        {
            face.Show(shownName, new RoomFaceLook(RingKind.None, 0, 0.35, false, false));
            word.Text = RoomWords.StateLeft;
            word.Foreground = Haze;
        }

        public void Show(RoomRow r)
        {
            shownName = r.Name;
            face.Show(r.Name, r.Face);
            if (word.Text != r.Word)
                word.Text = r.Word;
            word.Foreground = r.Watching ? Haze : (Brush)Application.Current.Resources["SilkBrush"];
            AutomationProperties.SetName(Element, r.Spoken);
            var isMuted = r.Muted;
            if (!r.You && muted != isMuted)
            {
                muted = isMuted;
                var item = new MenuFlyoutItem { Text = isMuted ? T.RoomUnmute(r.Name) : T.RoomMute(r.Name) };
                var who = r.Id;
                item.Click += (_, _) => column.Mute(who, !isMuted);
                var menu = new MenuFlyout();
                menu.Items.Add(item);
                Element.ContextFlyout = menu;
                Element.IsTabStop = true;
                Element.UseSystemFocusVisuals = true;
            }
        }

        /// <summary>A row joining fades in at its place and settles from 8 epx above, on standard.</summary>
        public void Enter(bool animate)
        {
            if (!animate)
                return;
            Element.Opacity = 0;
            Motion.FadeTo(Element, 1, Motion.FadeInTime);
            Motion.Ease(Element.RenderTransform, -8, 0, y => ((TranslateTransform)Element.RenderTransform).Y = y);
        }

        /// <summary>Left: dimmed in place for 4 s, then the rows below close the gap.</summary>
        public async void Leave(Action gone)
        {
            Leaving = true;
            var mine = ++leaves;
            await Task.Delay(Rooms.LeftRowHold);
            if (mine != leaves)
                return;
            // The row's contents fade while the row's height closes the gap on standard, or at
            // once with Reduce Motion: two elements, so neither animation stops the other.
            foreach (var part in Element.Children.OfType<FrameworkElement>())
                Motion.FadeTo(part, 0, Motion.FadeOutTime);
            if (Motion.Enabled)
                Motion.Height(Element, Element.ActualHeight, 0);
            await Task.Delay(Motion.Scaled(Motion.FadeOutTime));
            if (mine != leaves)
                return;
            gone();
        }

        /// <summary>Counts Leave and Stay, so a leave that a rejoin took back never closes the row.</summary>
        int leaves;

        /// <summary>Back before the row closed: the leave is called off and the row shows whole again.</summary>
        public void Stay()
        {
            leaves++;
            Leaving = false;
            // Stops a closing height (the same height both ends), then the row's own 44 again.
            Motion.Height(Element, 44, 44);
            Element.Height = 44;
            foreach (var part in Element.Children.OfType<FrameworkElement>())
                Motion.FadeTo(part, 1, Motion.FadeInTime);
        }
    }
}
