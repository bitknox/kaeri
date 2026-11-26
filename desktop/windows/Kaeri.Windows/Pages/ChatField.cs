using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.System;
using Windows.UI;

namespace Kaeri.Windows.Pages;

/// <summary>
/// The room's one-line chat field: a TextBox that sends on Enter, capped at the server's length,
/// with the characters left from 20 or the wait at its end. Chat never takes the amber: the
/// focus line is silk. Over the video it sits on the raised fill with the line always drawn;
/// in the flyout and the lobby on the faint fill, the line silk only while typing.
/// </summary>
sealed class ChatField : Grid
{
    readonly Ctx ctx;
    public TextBox Box { get; }
    readonly TextBlock tail = new()
    {
        Style = Ui.Res<Style>("HazeCaptionStyle"),
        HorizontalAlignment = HorizontalAlignment.Right,
        VerticalAlignment = VerticalAlignment.Center,
        Margin = new Thickness(0, 0, 12, 0),
        IsHitTestVisible = false,
        Visibility = Visibility.Collapsed,
    };
    bool toldLeft;
    uint toldWait;

    /// <summary>The text of the last lost send already put back, so a later snapshot doesn't put it back again.</summary>
    string? returned;

    static readonly Color Hair = Color.FromArgb(0x14, 0xE4, 0xE0, 0xD6);
    static readonly Color Silk = Color.FromArgb(0xFF, 0xE4, 0xE0, 0xD6);
    static readonly Color Haze = Color.FromArgb(0xFF, 0xA3, 0xA9, 0xC4);

    public ChatField(Ctx ctx, bool overVideo)
    {
        this.ctx = ctx;
        Typography.SetNumeralAlignment(tail, FontNumeralAlignment.Tabular);
        Box = new TextBox
        {
            PlaceholderText = RoomWords.SaySomething,
            MaxLength = 150,
            IsSpellCheckEnabled = true,
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetName(Box, RoomWords.SaySomething);
        var fill = new SolidColorBrush(overVideo ? Color.FromArgb(0xF0, 0x22, 0x26, 0x38) : Color.FromArgb(0x0F, 0xE4, 0xE0, 0xD6));
        foreach (var key in new[] { "TextControlBackground", "TextControlBackgroundPointerOver", "TextControlBackgroundFocused" })
            Box.Resources[key] = fill;
        var rest = overVideo ? Underline(Silk, 2) : Underline(Haze, 1);
        Box.Resources["TextControlBorderBrush"] = rest;
        Box.Resources["TextControlBorderBrushPointerOver"] = rest;
        Box.Resources["TextControlBorderBrushFocused"] = Underline(Silk, 2);
        Box.Resources["TextControlForegroundFocused"] = new SolidColorBrush(Silk);
        Box.KeyDown += OnKey;
        Box.TextChanged += (_, _) => Typed();
        // The field's end holds the count and the wait, so the template's clear button stays out.
        Box.Loaded += (_, _) =>
        {
            if (Find(Box, "DeleteButton") is Button clear)
            {
                clear.Width = 0;
                clear.MinWidth = 0;
                clear.Opacity = 0;
                clear.IsTabStop = false;
                clear.IsHitTestVisible = false;
            }
        };
        Children.Add(Box);
        Children.Add(tail);
    }

    static FrameworkElement? Find(DependencyObject root, string name)
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(root); i++)
        {
            var child = VisualTreeHelper.GetChild(root, i);
            if (child is FrameworkElement { Name: var n } found && n == name)
                return found;
            if (Find(child, name) is { } deeper)
                return deeper;
        }
        return null;
    }

    /// <summary>The control's edge in the hair ring with a line along the bottom, as Fluent draws its focus line.</summary>
    static LinearGradientBrush Underline(Color line, double px)
    {
        var brush = new LinearGradientBrush
        {
            MappingMode = BrushMappingMode.Absolute,
            StartPoint = new global::Windows.Foundation.Point(0, 0),
            EndPoint = new global::Windows.Foundation.Point(0, px),
            RelativeTransform = new ScaleTransform { ScaleY = -1, CenterY = 0.5 },
        };
        brush.GradientStops.Add(new GradientStop { Offset = 1, Color = line });
        brush.GradientStops.Add(new GradientStop { Offset = 1, Color = Hair });
        return brush;
    }

    /// <summary>Enter sends; Escape is the owner's.</summary>
    public event Action? Escaped;

    void OnKey(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == VirtualKey.Enter)
        {
            e.Handled = true;
            Submit();
        }
        else if (e.Key == VirtualKey.Escape && Escaped is { } escaped)
        {
            e.Handled = true;
            escaped();
        }
    }

    uint Wait => ctx.Known.RoomChatState?.WaitSeconds ?? 0;

    public void Submit()
    {
        var text = Box.Text.Trim();
        if (text.Length == 0 || Wait > 0)
            return;
        ctx.Send(new Command.Room(new RoomCommand.Say(text)));
        Box.Text = "";
    }

    /// <summary>What the engine allows now: the length, and the wait or the count at the end.</summary>
    public void Typed()
    {
        var state = ctx.Known.RoomChatState;
        var max = state?.MaxLength ?? 150;
        if (Box.MaxLength != (int)max)
            Box.MaxLength = (int)max;
        var wait = state?.WaitSeconds ?? 0;
        var text = Chat.Tail(Box.Text, max, wait);
        tail.Text = text ?? "";
        tail.Visibility = text is null ? Visibility.Collapsed : Visibility.Visible;
        Box.Padding = new Thickness(11, 5, text is null ? 11 : 24 + text.Length * 8, 6);
        // Said once to a screen reader: the count at 20 left, and a wait as it starts.
        var left = Math.Max(0, (int)max - Box.Text.Length);
        if (left <= Chat.CountFrom && Box.Text.Trim().Length > 0)
        {
            if (!toldLeft)
                Tell(T.RoomChatCharactersLeft(left));
            toldLeft = true;
        }
        else
            toldLeft = false;
        if (wait > 0)
        {
            if (toldWait == 0)
                Tell(T.RoomChatWaitSpoken(wait));
            toldWait = wait;
        }
        else
            toldWait = 0;
    }

    void Tell(string text) =>
        FrameworkElementAutomationPeer.FromElement(Box)?.RaiseNotificationEvent(
            AutomationNotificationKind.Other, AutomationNotificationProcessing.ImportantMostRecent, text, "party-chat");

    /// <summary>A send the server never echoed: its text goes back into an empty field, once.</summary>
    public void Lost(string? failed)
    {
        if (failed is null)
        {
            returned = null;
            return;
        }
        if (returned == failed)
            return;
        returned = failed;
        if (Box.Text.Length == 0)
        {
            Box.Text = failed;
            Box.SelectionStart = failed.Length;
        }
    }

    /// <summary>A typed line: the name in semibold ("You" in haze), an en space, then the words as one sentence.</summary>
    public static TextBlock Said(string who, bool you, string text)
    {
        var block = Line();
        var name = new Run { Text = who, FontWeight = FontWeights.SemiBold };
        if (you)
            name.Foreground = Ui.Res<Brush>("HazeBrush");
        block.Inlines.Add(name);
        block.Inlines.Add(new Run { Text = " " + text });
        AutomationProperties.SetName(block, T.RoomChatAnnounce(who, text));
        return block;
    }

    static TextBlock Line() => new()
    {
        FontSize = 14,
        LineHeight = 20,
        TextWrapping = TextWrapping.Wrap,
        IsTextSelectionEnabled = true,
        Foreground = Ui.Res<Brush>("SilkBrush"),
    };

}
