using Kaeri.App;
using Kaeri.Engine.Model;
using Kaeri.Windows.Pages;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows;

/// <summary>
/// Private watching and reading, the window's side: the title bar's Private badge left of the
/// caption buttons with its flyout, Ctrl+Shift+N anywhere in the app, the notice when the
/// shortcut turns the mode on, and what screen readers hear when it turns on or off. The engine
/// holds the mode; every part here draws from what it last said.
/// </summary>
public sealed partial class MainWindow
{
    /// <summary>How long the notice stays when the shortcut turns the mode on, as the board has it.</summary>
    static readonly TimeSpan PrivateNoticeTime = TimeSpan.FromSeconds(8);

    /// <summary>The title bar's end: the status (or the way back to a TV's card), then the badge.</summary>
    readonly ContentControl titleStatus = new() { VerticalAlignment = VerticalAlignment.Center, HorizontalContentAlignment = HorizontalAlignment.Stretch };
    Button? privateBadge;
    /// <summary>What the window last drew of the mode, null before the engine's first word at launch.</summary>
    bool? privateDrawn;
    /// <summary>The notice the shortcut showed, so turning the mode off takes it away.</summary>
    int privateToast = -1;

    /// <summary>Puts the status and the badge at the title bar's end.</summary>
    void StartPrivate()
    {
        titleStatus.Content = status;
        privateBadge = new Button
        {
            Height = 32,
            Padding = new Thickness(10, 0, 10, 0),
            Margin = new Thickness(0, 0, 8, 0),
            VerticalAlignment = VerticalAlignment.Center,
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            Visibility = Visibility.Collapsed,
            Flyout = PrivateFlyout(),
        };
        var inner = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        inner.Children.Add(PrivateUi.Eye(16));
        inner.Children.Add(new TextBlock { Text = T.PrivateBadge, VerticalAlignment = VerticalAlignment.Center, Foreground = Ui.Res<Brush>("SilkBrush") });
        privateBadge.Content = inner;
        AutomationProperties.SetName(privateBadge, T.PrivateBadgeLabel);
        ToolTipService.SetToolTip(privateBadge, T.PrivateBadgeLabel);
        privateBadge.SizeChanged += (_, _) => KeepSearchInPlace();
        var end = new StackPanel { Orientation = Orientation.Horizontal, VerticalAlignment = VerticalAlignment.Center };
        end.Children.Add(titleStatus);
        end.Children.Add(privateBadge);
        AppTitleBar.RightHeader = end;
    }

    /// <summary>The badge's flyout: the eye, Private mode, what it means, and Turn off.</summary>
    Flyout PrivateFlyout()
    {
        var body = new StackPanel { Width = 288 };
        var head = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10 };
        head.Children.Add(PrivateUi.Eye(18));
        head.Children.Add(new TextBlock { Text = T.PrivateMode, FontWeight = FontWeights.SemiBold, VerticalAlignment = VerticalAlignment.Center });
        body.Children.Add(head);
        body.Children.Add(new TextBlock { Text = T.PrivateModeOnNotice, TextWrapping = TextWrapping.WrapWholeWords, Foreground = Ui.Res<Brush>("HazeBrush"), Margin = new Thickness(0, 6, 0, 0) });
        var flyout = new Flyout { Placement = FlyoutPlacementMode.BottomEdgeAlignedRight, Content = body };
        var off = new Button { Content = T.ActionTurnOff, HorizontalAlignment = HorizontalAlignment.Right, Margin = new Thickness(0, 14, 0, 0) };
        off.Click += (_, _) =>
        {
            flyout.Hide();
            SetPrivate(false);
        };
        body.Children.Add(off);
        return flyout;
    }

    /// <summary>Asks the engine to turn the mode on or off; the shortcut's turning on shows the notice.</summary>
    internal void SetPrivate(bool on, bool notice = false)
    {
        ctx.Send(new Command.SetPrivate(on));
        if (on && notice)
        {
            ShowToast(InfoBarSeverity.Informational, T.PrivateModeOnNotice, T.ActionTurnOff, () => SetPrivate(false),
                hold: PrivateNoticeTime, icon: new FontIconSource { Glyph = PrivateUi.Glyph });
            privateToast = toastShown;
        }
    }

    /// <summary>Ctrl+Shift+N: the mode turns over, with the notice when it turns on.</summary>
    void TogglePrivate() => SetPrivate(!ctx.Known.Private.On, notice: true);

    /// <summary>The engine said the mode is on or off: the badge, the notice and what screen readers hear.</summary>
    void PrivateHeard(bool on)
    {
        var was = privateDrawn;
        privateDrawn = on;
        if (privateBadge is { } badge)
        {
            if (on)
                Motion.Show(badge);
            else
            {
                if (badge.Flyout is FlyoutBase open && open.IsOpen)
                    open.Hide();
                Motion.Hide(badge);
            }
        }
        if (!on && privateToast == toastShown && Toast.IsOpen)
            Toast.IsOpen = false;
        // Launch says the mode is off; only a change is heard.
        if (was is { } before && before != on
            && (FrameworkElementAutomationPeer.FromElement(Root) ?? FrameworkElementAutomationPeer.CreatePeerForElement(Root)) is { } peer)
            peer.RaiseNotificationEvent(AutomationNotificationKind.ActionCompleted, AutomationNotificationProcessing.ImportantMostRecent,
                Privately.Announcement(on), "kaeri-private"); // i18n-skip
    }

#if KAERI_TESTING
    /// <summary>
    /// --show=private: 12 s in, the mode turns on as Ctrl+Shift+N turns it, with the notice;
    /// --show=private-open also opens the badge's flyout 12 s after that, once the notice is gone.
    /// Slow enough for a shot of each state in a one-core test VM.
    /// </summary>
    void PrivateScene()
    {
        if (options.Show is not ("private" or "private-open"))
            return;
        DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, async () =>
        {
            await Task.Delay(TimeSpan.FromSeconds(12));
            TogglePrivate();
            if (options.Show != "private-open")
                return;
            await Task.Delay(TimeSpan.FromSeconds(12));
            if (Toast.IsOpen)
                Toast.IsOpen = false;
            privateBadge?.Flyout?.ShowAt(privateBadge);
        });
    }
#endif
}
