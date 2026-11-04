using System.Numerics;
using Kaeri.App;
using Kaeri.Engine.Model;
using Kaeri.Windows.Pages;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;

namespace Kaeri.Windows.Player;

/// <summary>
/// The party's chat over the video: lines in the top leading column under the player's alert,
/// three at most and the rest folded into "n more in chat"; a reaction's moment bottom trailing,
/// above the corner's card; the four reaction buttons before the people button; ticks on the
/// seek bar; and the field C opens above the controls. Hide chat (Settings, Playback) takes all
/// of it away, in every party, and a server without chat leaves only its notice.
/// </summary>
public sealed partial class PlayerView
{
    /// <summary>How long a reaction's moment stays.</summary>
    static readonly TimeSpan Moment = TimeSpan.FromMilliseconds(1500);

    /// <summary>The most moments up at once.</summary>
    const int Moments = 4;

    /// <summary>How far a line or a moment rises as it fades in, in epx.</summary>
    const float Rise = 6;

    /// <summary>Reactions within this many epx of the tick before them share it.</summary>
    const double TickNear = 4;

    readonly List<Border> chatShown = [];
    readonly Dictionary<Border, (string Who, bool You, string Text)> chatSaid = [];
    readonly Queue<(string Who, bool You, string Text)> chatWaiting = new();
    /// <summary>Lines a notice pushed out of the column, oldest first: they come back above the rest.</summary>
    readonly List<(string Who, bool You, string Text)> chatPushed = [];
    ulong chatSeen;
    bool chatPopover, reactionsBuilt;
    ChatField? chatField;
    PartyReactionMark[] chatMarks = [];
    readonly List<(double X, Chat.Tick Tick)> tickPlaces = [];

    bool ChatHidden => ChatSetting.Hidden;
    bool ChatAllowed => ctx?.Known.PartyChatState?.Allowed ?? true;
    readonly HashSet<Microsoft.UI.Dispatching.DispatcherQueueTimer> chatTimers = [];

    bool ChatLive => InParty && !ChatHidden && ChatAllowed;

    void BuildReactions()
    {
        if (reactionsBuilt)
            return;
        reactionsBuilt = true;
        foreach (var reaction in Chat.All)
        {
            var button = new Button { Style = (Style)Application.Current.Resources["PlayerButtonStyle"], Content = ReactionGlyph.Make(reaction, 20) };
            ToolTipService.SetToolTip(button, Chat.Label(reaction));
            AutomationProperties.SetName(button, Chat.Label(reaction));
            button.Click += (_, _) =>
            {
                still.Input();
                ctx?.Send(new Command.Party(new PartyCommand.React(reaction)));
            };
            Reactions.Children.Add(button);
        }
        ChatSetting.Changed += () => DispatcherQueue.TryEnqueue(ChatReflect);
        ChatLines.ChildrenTransitions = new TransitionCollection { new RepositionThemeTransition() };
        ChatColumn.ChildrenTransitions = new TransitionCollection { new RepositionThemeTransition() };
    }

    /// <summary>The buttons, the field and everything on the video as the party, the server and Hide chat now allow.</summary>
    void ChatReflect()
    {
        BuildReactions();
        var live = ChatLive;
        Reactions.Visibility = live && !mini ? Visibility.Visible : Visibility.Collapsed;
        if (ChatHidden || !InParty)
        {
            ChatClear();
            CloseChatField();
            chatMarks = [];
        }
        else
        {
            if (!live)
                CloseChatField();
            chatMarks = ctx?.Known.PartyChat?.Marks ?? [];
        }
        DrawTimeline();
    }

    /// <summary>Everything on the video goes at once: leaving the party, Hide chat, the player closing.</summary>
    void ChatClear()
    {
        chatShown.Clear();
        chatSaid.Clear();
        ChatLines.Children.Clear();
        chatWaiting.Clear();
        chatPushed.Clear();
        ChatFold.Visibility = Visibility.Collapsed;
        ChatMoments.Children.Clear();
        momentsLeaving.Clear();
        LiftCorner();
    }

    /// <summary>
    /// Whether a player notice is a chat line's echo: the engine still says each typed line as
    /// "Mika: words" for players without a column, and here the column shows it.
    /// </summary>
    bool ChatEcho(string text)
    {
        if (!InParty || ctx is null)
            return false;
        var lines = ctx.Known.PartyChat?.Lines ?? [];
        if (lines.TakeLast(4).Any(l => l.Body is PartyChatBody.Said said && text.Contains(said.Text, StringComparison.Ordinal)))
            return true;
        return (ctx.Known.PartyMembers ?? []).Any(m => text.StartsWith(m.Name + ": ", StringComparison.Ordinal));
    }

    void ChatHeard(Event e)
    {
        switch (e)
        {
            case Event.PartyJoined:
                // A room joined afresh starts its conversation afresh; the same room again keeps its place.
                if (ctx?.Known.PartyChat is null)
                    chatSeen = 0;
                ChatReflect();
                break;
            case Event.PartyLeft:
                chatSeen = 0;
                ChatReflect();
                break;
            case Event.PartyChat { V1: var conversation }:
            {
                var (fresh, newest) = Chat.Fresh(conversation.Lines, chatSeen);
                chatSeen = newest;
                if (ChatHidden)
                    break;
                if (Showing && !mini && !chatPopover)
                    foreach (var line in fresh)
                        Arrive(line);
                if (chatMarks.Length != conversation.Marks.Length || !chatMarks.SequenceEqual(conversation.Marks))
                {
                    chatMarks = conversation.Marks;
                    DrawTimeline();
                }
                if (ChatFieldSlot.Visibility == Visibility.Visible)
                    chatField?.Lost(conversation.Failed);
                break;
            }
            case Event.PartyChatState { V1: var state }:
                ChatReflect();
                foreach (var button in Reactions.Children.OfType<Button>())
                {
                    button.IsEnabled = state.ReactWaitSeconds == 0;
                    button.Opacity = state.ReactWaitSeconds == 0 ? 1 : 0.4;
                }
                chatField?.Typed();
                break;
            case Event.PartyReaction { V1: var reaction }:
                if (Showing && !mini && !ChatHidden)
                    ShowMoment(reaction);
                break;
            case Event.PartyChatRefused { V1: PartyChatRefusal.Off }:
                if (Showing)
                    Say(T.PartyChatOff);
                break;
        }
    }

    // The column.

    void Arrive(PartyChatLine line)
    {
        if (line.Body is not PartyChatBody.Said said)
            return;
        var who = Chat.Who(line);
        // Said to a screen reader as it comes; reactions are not.
        if (FrameworkElementAutomationPeer.CreatePeerForElement(ChatLines) is { } peer)
            peer.RaiseNotificationEvent(AutomationNotificationKind.Other, AutomationNotificationProcessing.All, T.PartyChatAnnounce(who, said.Text), "party-chat-line");
        if (chatShown.Count < ChatRoom && chatWaiting.Count == 0 && chatPushed.Count == 0)
            ShowLine(who, line.You, said.Text);
        else
        {
            chatWaiting.Enqueue((who, line.You, said.Text));
            ShowFold();
        }
    }

    void ShowFold()
    {
        var count = chatWaiting.Count + chatPushed.Count;
        if (count == 0)
        {
            if (ChatFold.Visibility == Visibility.Visible)
                Motion.Hide(ChatFold);
            return;
        }
        ChatFoldText.Text = T.PartyChatMore(count);
        AutomationProperties.SetName(ChatFold, T.PartyChatMore(count));
        if (ChatFold.Visibility != Visibility.Visible && !chatPopover)
            Motion.Show(ChatFold);
    }

    void OnChatFold(object sender, RoutedEventArgs e) => ShowParty();

    /// <summary>A line on the raised fill with a hair ring: the name in semibold, an en space, the words, three rows at most.</summary>
    static Border LineCard(string who, bool you, string text)
    {
        var words = ChatPane.Said(who, you, text);
        words.IsTextSelectionEnabled = false;
        words.MaxLines = 3;
        words.TextTrimming = TextTrimming.CharacterEllipsis;
        words.TextWrapping = TextWrapping.WrapWholeWords;
        words.MaxWidth = 480 - 32;
        return new Border
        {
            Child = words,
            Background = (Brush)Application.Current.Resources["RaisedBrush"],
            BorderBrush = (Brush)Application.Current.Resources["HairBrush"],
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(8),
            Padding = new Thickness(15, 9, 15, 9),
            HorizontalAlignment = HorizontalAlignment.Left,
            IsHitTestVisible = false,
        };
    }

    void ShowLine(string who, bool you, string text, bool above = false)
    {
        var card = LineCard(who, you, text);
        chatShown.Insert(above ? 0 : chatShown.Count, card);
        chatSaid[card] = (who, you, text);
        ChatLines.Children.Insert(above ? 0 : ChatLines.Children.Count, card);
        Arrives(card);
        ShowFold();
        var hold = Chat.Hold(text);
        ChatAfter(hold, () => LeaveLine(card));
    }

    /// <summary>Fades in (200 ms) with a 6 epx rise; with animation effects off, the fade alone.</summary>
    static void Arrives(UIElement element)
    {
        element.Opacity = 0;
        element.Visibility = Visibility.Visible;
        if (Motion.Enabled)
        {
            element.Translation = new Vector3(0, Rise, 0);
            element.TranslationTransition = new Vector3Transition { Duration = Motion.Standard };
            element.DispatcherQueue.TryEnqueue(() => element.Translation = Vector3.Zero);
        }
        Motion.Show(element);
    }

    /// <summary>A line has held its time: it fades out (300 ms), the column closes up, and the next waiting line takes its place.</summary>
    void LeaveLine(Border card)
    {
        if (!chatShown.Contains(card))
            return;
        Motion.Hide(card);
        ChatAfter(Motion.FadeOutTime + TimeSpan.FromMilliseconds(20), () =>
        {
            if (!chatShown.Remove(card))
                return;
            chatSaid.Remove(card);
            ChatLines.Children.Remove(card);
            if (!Motion.Enabled)
            {
                // Nothing slides: the others fade up at their new places as the gap closes.
                foreach (var other in chatShown)
                {
                    other.Opacity = 0.4;
                    Motion.Show(other);
                }
            }
            DrainChat();
        });
    }

    /// <summary>How many chat lines the column has room for: a notice above them takes one of the three.</summary>
    int ChatRoom => alertUp ? Chat.Column - 1 : Chat.Column;

    /// <summary>Waiting lines take the room the column has, and the fold counts the rest.</summary>
    void DrainChat()
    {
        // A pushed line is older than any shown, so it comes back at the top, newest of them first.
        while (chatShown.Count < ChatRoom && chatPushed.Count > 0)
        {
            var back = chatPushed[^1];
            chatPushed.RemoveAt(chatPushed.Count - 1);
            ShowLine(back.Who, back.You, back.Text, above: true);
        }
        while (chatShown.Count < ChatRoom && chatWaiting.TryDequeue(out var next))
            ShowLine(next.Who, next.You, next.Text);
        ShowFold();
    }

    /// <summary>
    /// A notice came up over the lines: the oldest lines past the room fade out into the fold, and
    /// come back at the top, for a full hold, once the notice has gone.
    /// </summary>
    void MakeChatRoom()
    {
        var pushed = false;
        while (chatShown.Count > ChatRoom)
        {
            var oldest = chatShown[0];
            chatShown.RemoveAt(0);
            if (chatSaid.Remove(oldest, out var said))
                chatPushed.Add(said);
            pushed = true;
            Motion.Hide(oldest);
            ChatAfter(Motion.FadeOutTime + TimeSpan.FromMilliseconds(20), () => ChatLines.Children.Remove(oldest));
        }
        if (pushed)
            ShowFold();
    }

    /// <summary>The people flyout opened or closed: the column steps aside while the same lines are in it.</summary>
    void ChatPopover(bool open)
    {
        chatPopover = open;
        if (open)
        {
            CloseChatField();
            Motion.Hide(ChatLines);
            if (ChatFold.Visibility == Visibility.Visible)
                Motion.Hide(ChatFold);
            // What waited is in the flyout now.
            chatWaiting.Clear();
            chatPushed.Clear();
        }
        else
        {
            Motion.Show(ChatLines);
            ShowFold();
        }
    }

    // Moments.

    void ShowMoment(PartyReaction reaction)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        row.Children.Add(ReactionGlyph.Make(reaction.Reaction, 20));
        row.Children.Add(new TextBlock
        {
            Text = reaction.You ? T.PartyChatYou : reaction.From,
            FontSize = 14,
            FontWeight = FontWeights.SemiBold,
            VerticalAlignment = VerticalAlignment.Center,
        });
        var pill = new Border
        {
            Child = row,
            Height = 40,
            Background = (Brush)Application.Current.Resources["RaisedBrush"],
            BorderBrush = (Brush)Application.Current.Resources["HairBrush"],
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(8),
            Padding = new Thickness(10, 0, 15, 0),
            HorizontalAlignment = HorizontalAlignment.Right,
        };
        // The oldest steps aside when four are up; it is the top one, so nothing below it moves.
        var up = ChatMoments.Children.Where(c => !momentsLeaving.Contains(c)).ToList();
        foreach (var oldest in up.Take(Math.Max(0, up.Count - (Moments - 1))))
            MomentLeaves(oldest);
        // The stack grows from the bottom: the moments already up rise one place to make room.
        foreach (var other in ChatMoments.Children)
        {
            if (Motion.Enabled)
            {
                other.TranslationTransition = null;
                other.Translation = new Vector3(0, 46, 0);
                other.TranslationTransition = new Vector3Transition { Duration = Motion.Standard };
                var lifted = other;
                DispatcherQueue.TryEnqueue(() => lifted.Translation = Vector3.Zero);
            }
            else if (!momentsLeaving.Contains(other))
            {
                // Nothing slides: they fade up at their new places, as the column's lines do.
                other.Opacity = 0.4;
                Motion.Show(other);
            }
        }
        ChatMoments.Children.Add(pill);
        LiftCorner();
        Arrives(pill);
        ChatAfter(Moment, () => MomentLeaves(pill));
    }

    readonly HashSet<UIElement> momentsLeaving = [];

    /// <summary>A moment fades out (300 ms) and goes; it is the top one, so the others stay where they are.</summary>
    void MomentLeaves(UIElement pill)
    {
        if (!momentsLeaving.Add(pill))
            return;
        Motion.Hide(pill);
        ChatAfter(Motion.FadeOutTime + TimeSpan.FromMilliseconds(20), () =>
        {
            momentsLeaving.Remove(pill);
            if (ChatMoments.Children.Remove(pill))
                LiftCorner();
        });
    }

    /// <summary>
    /// Runs <paramref name="then"/> once after <paramref name="wait"/>. The timer is held until it
    /// fires: a dispatcher timer nothing refers to can be collected and never tick.
    /// </summary>
    void ChatAfter(TimeSpan wait, Action then)
    {
        var timer = DispatcherQueue.CreateTimer();
        timer.Interval = wait;
        timer.IsRepeating = false;
        chatTimers.Add(timer);
        timer.Tick += (t, _) =>
        {
            chatTimers.Remove(t);
            then();
        };
        timer.Start();
    }

    /// <summary>The corner's cards rise above the moments while any are up.</summary>
    void LiftCorner()
    {
        var count = ChatMoments.Children.Count;
        Corner.Margin = new Thickness(0, 0, 24, 80 + (count > 0 ? count * 46 : 0));
    }

    // The field.

    /// <summary>C: the field opens above the controls and takes focus, or the room's server says it has no chat.</summary>
    bool OpenChatField()
    {
        if (ctx is null || !InParty || ChatHidden || mini)
            return false;
        if (!ChatAllowed)
        {
            Say(T.PartyChatOff);
            return true;
        }
        if (chatField is null)
        {
            chatField = new ChatField(ctx, overVideo: true) { Height = 32 };
            chatField.Escaped += () =>
            {
                CloseChatField();
                Focus(FocusState.Programmatic);
            };
            chatField.Box.LostFocus += (_, _) => idle.Start();
            ChatFieldSlot.Child = chatField;
        }
        if (ChatFieldSlot.Visibility != Visibility.Visible)
            Arrives(ChatFieldSlot);
        chatField.Lost(ctx.Known.PartyChat?.Failed);
        chatField.Typed();
        ShowControls();
        DispatcherQueue.TryEnqueue(() => chatField.Box.Focus(FocusState.Programmatic));
        return true;
    }

    /// <summary>Escape, or the controls going with nothing typed: the field fades away.</summary>
    void CloseChatField()
    {
        if (ChatFieldSlot.Visibility != Visibility.Visible)
            return;
        Motion.Hide(ChatFieldSlot);
    }

    /// <summary>Whether the field holds the controls up: it has focus, or something typed in it.</summary>
    bool ChatFieldHolds() =>
        ChatFieldSlot.Visibility == Visibility.Visible && chatField is { } field
        && (field.Box.FocusState != FocusState.Unfocused || field.Box.Text.Length > 0);

    /// <summary>Whether focus is in the field or in any other text box, where keys are typing.</summary>
    bool Typing() =>
        XamlRoot is { } root && Microsoft.UI.Xaml.Input.FocusManager.GetFocusedElement(root) is TextBox;

    void OnKeyChat(Microsoft.UI.Xaml.Input.KeyboardAccelerator s, Microsoft.UI.Xaml.Input.KeyboardAcceleratorInvokedEventArgs e)
    {
        if (!Showing || Typing())
            return;
        if (OpenChatField())
            e.Handled = true;
    }

    // Ticks.

    /// <summary>A 2 epx tick in haze above the seek line at each reaction, and where each is for the hover.</summary>
    void DrawTicks(Func<double, double> x, double top, double width)
    {
        tickPlaces.Clear();
        if (ChatHidden || !InParty || chatMarks.Length == 0 || duration <= 0)
            return;
        var haze = (Brush)Application.Current.Resources["HazeBrush"];
        foreach (var tick in Chat.Ticks(chatMarks, duration, width, TickNear))
        {
            var at = x(tick.At);
            var mark = new Microsoft.UI.Xaml.Shapes.Rectangle { Width = 2, Height = 5, Fill = haze };
            Canvas.SetLeft(mark, at - 1);
            Canvas.SetTop(mark, top - 3 - 5);
            TimelineUnder.Children.Add(mark);
            tickPlaces.Add((at, tick));
        }
    }

    /// <summary>The tick within reach of the pointer on the seek line, x in the ticks' canvas.</summary>
    Chat.Tick? TickAt(double x)
    {
        Chat.Tick? best = null;
        var nearest = 6.0;
        foreach (var (at, tick) in tickPlaces)
        {
            if (Math.Abs(at - x) <= nearest)
            {
                nearest = Math.Abs(at - x);
                best = tick;
            }
        }
        return best;
    }

    /// <summary>
    /// Near a tick, the time label names who reacted there: the preview's own label when it is up,
    /// else a small label of its own above the tick.
    /// </summary>
    void ShowTickLabel(double x)
    {
        if (TickAt(x) is not { } tick)
        {
            TickLabel.Visibility = Visibility.Collapsed;
            return;
        }
        if (PreviewBox.Visibility == Visibility.Visible)
        {
            PreviewTime.Text = tick.Label();
            TickLabel.Visibility = Visibility.Collapsed;
            return;
        }
        TickLabelText.Text = tick.Label();
        TickLabel.Visibility = Visibility.Visible;
        TickLabel.UpdateLayout();
        var place = TimelineUnder.TransformToVisual(VideoPanel).TransformPoint(new global::Windows.Foundation.Point(x, 0));
        var bottom = VideoPanel.ActualHeight - place.Y + 8 + 8;
        var left = Math.Clamp(place.X - TickLabel.ActualWidth / 2, 8, Math.Max(8, VideoPanel.ActualWidth - TickLabel.ActualWidth - 8));
        TickLabel.Margin = new Thickness(left, 0, 0, bottom);
    }

#if KAERI_TESTING
    /// <summary>A shot's: the field open with this typed.</summary>
    internal void ShotChatType(string text)
    {
        OpenChatField();
        if (chatField is { } field)
        {
            field.Box.Text = text;
            field.Box.SelectionStart = text.Length;
        }
    }

    /// <summary>A shot's: send what is typed.</summary>
    internal void ShotChatSend() => chatField?.Submit();

    /// <summary>A shot's: the pointer over the seek line at a moment, for the tick's label.</summary>
    internal void ShotTickHover(double at)
    {
        ShowControls();
        foreach (var (x, tick) in tickPlaces)
            if (Math.Abs(tick.At - at) < 1)
                ShowTickLabel(x);
    }
#endif
}
