using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// The title page while the next episode has aired and no source lists it: Check again and the
/// episode before it lead the button row, and the status under it says which episode, when it
/// aired and where the looking stands. Check again follows the waiting rule (nothing for 600 ms,
/// then Checking in the button for at least 0.3 s). When a source lists the episode, the status
/// and the two buttons crossfade to the amber Play in place while the lead's width settles, and
/// the episode's row lifts at the same moment.
/// </summary>
public sealed partial class TitlePage
{
    /// <summary>The missing episode the page shows, or null.</summary>
    MissingEpisode? missing;
    /// <summary>A Check again under way: the page holds what it shows until the answer is shown.</summary>
    bool checking;
    /// <summary>When Checking appeared in the button, if it did.</summary>
    DateTimeOffset? checkingShown;
    /// <summary>A table that came while a check was under way, drawn with its answer.</summary>
    bool tableHeld;
    DispatcherQueueTimer? checkingTimer, answerTimer, minuteTimer;

    static long Now() => DateTimeOffset.UtcNow.ToUnixTimeSeconds();

    /// <summary>The missing episode at this moment: while a check runs, the one shown stays.</summary>
    MissingEpisode? MissingNow(Entry entry) =>
        checking ? missing : MissingWords.Of(entry, ctx.Known.Episodes.GetValueOrDefault(id), Now());

    /// <summary>
    /// The lead and the status for <paramref name="now"/>, after the main action has been laid:
    /// a missing episode takes the amber action's place, and one that was missing and is found
    /// crossfades to it.
    /// </summary>
    void ShowMissing(MissingEpisode? now)
    {
        var was = missing;
        missing = now;
        if (now is null)
        {
            StopMinute();
            if (was is not null && IsLoaded && PrimaryAction.Visibility == Visibility.Visible)
                Found(was);
            else
            {
                MissingButtons.Visibility = Visibility.Collapsed;
                MissingLine.Visibility = Visibility.Collapsed;
            }
            return;
        }
        PrimaryAction.Visibility = Visibility.Collapsed;
        WaitingLine.Visibility = Visibility.Collapsed;
        TvAction.Visibility = Visibility.Collapsed;
        Motion.Set(MissingButtons, true);
        Lead.Width = double.NaN;
        SayMissing(crossfade: IsLoaded && was is not null && was.Episode == now.Episode && MissingLine.Visibility == Visibility.Visible);
        Motion.Set(MissingLine, true);
        MissingLine.Height = double.NaN;
        MissingLine.Margin = new Thickness(0, 4, 0, 0);
        Minute();
    }

    /// <summary>The status's words and the buttons it offers. A change of words crossfades on the standard token.</summary>
    void SayMissing(bool crossfade)
    {
        if (missing is not { } m)
            return;
        var status = MissingWords.Status(m, Now(), ctx.Known.MissingChecks.Asked(id, m.Episode), ctx.Known.Offline, at => Synopsis.Date(at));
        var head = status.Head;
        var tail = head.Length > 0 ? " " + status.Tail : status.Tail;
        void Apply()
        {
            MissingHead.Text = head;
            MissingTail.Text = tail;
            MissingDot.Fill = Ui.Res<Brush>(status.LongAgo ? "HazeBrush" : "WaterBrush");
        }
        if (crossfade && (MissingHead.Text != head || MissingTail.Text != tail))
            Crossfade.Swap(MissingLine, Apply);
        else
            Apply();
        CheckAgain.Visibility = status.CheckAgain ? Visibility.Visible : Visibility.Collapsed;
        ToolTipService.SetToolTip(CheckAgain, status.Help);
        AutomationProperties.SetHelpText(CheckAgain, status.Help ?? "");
        if (m.Previous is { } previous)
        {
            PlayPreviousText.Text = T.TitleButtonPlay(previous);
            PlayPrevious.Visibility = Visibility.Visible;
        }
        else
            PlayPrevious.Visibility = Visibility.Collapsed;
    }

    /// <summary>
    /// The episode is found: the status and the two buttons crossfade to the amber Play over
    /// 0.3 s while the lead's width and the status's height settle on the standard token, so
    /// the row never goes through empty. Focus on the buttons that go moves to Play.
    /// </summary>
    void Found(MissingEpisode was)
    {
        var focused = FocusManager.GetFocusedElement(XamlRoot) is { } f && (f == CheckAgain || f == PlayPrevious);
        var from = Lead.ActualWidth;
        PrimaryAction.Opacity = 0;
        PrimaryAction.Measure(new global::Windows.Foundation.Size(double.PositiveInfinity, double.PositiveInfinity));
        var to = PrimaryAction.DesiredSize.Width;
        Lead.Width = from;
        Motion.Ease(Lead, from, to, width =>
        {
            Lead.Width = width;
            if (Math.Abs(width - to) < 0.5)
                Lead.Width = double.NaN;
        });
        Motion.Reveal(PrimaryAction, 1);
        Motion.Reveal(MissingButtons, 0, () =>
        {
            if (missing is not null)
                return;
            Motion.Set(MissingButtons, false);
            Lead.Width = double.NaN;
        });
        // The status goes with them, its height closing with its margin and the spacing over it.
        var height = MissingLine.ActualHeight;
        if (MissingLine.Visibility == Visibility.Visible && height > 0)
        {
            MissingLine.Height = height;
            Motion.Ease(MissingLine, height, 0, h =>
            {
                var left = h / height;
                MissingLine.Height = h;
                MissingLine.Margin = new Thickness(0, 4 * left, 0, -6 * (1 - left));
            });
            Motion.Reveal(MissingLine, 0, () =>
            {
                if (missing is not null)
                    return;
                Motion.Set(MissingLine, false);
                MissingLine.Height = double.NaN;
                MissingLine.Margin = new Thickness(0, 4, 0, 0);
            });
        }
        else
            MissingLine.Visibility = Visibility.Collapsed;
        if (focused)
            PrimaryAction.Focus(FocusState.Keyboard);
        Announce(T.EpisodeFoundAnnouncement(was.Episode));
    }

    /// <summary>
    /// Check again: the sources are asked for the episode now. Nothing shows for 600 ms, then
    /// Checking in the button. Offline it answers at once.
    /// </summary>
    void OnCheckAgain(object sender, RoutedEventArgs e)
    {
        if (checking || missing is not { } m || m.Phase == MissingPhase.LongAgo)
            return;
        checking = true;
        checkingShown = null;
        ctx.Send(new Command.Missing(new MissingCommand.CheckAgain(id, m.Episode)));
        checkingTimer?.Stop();
        checkingTimer = Timers.Weak(this, CheckWait.Quiet, false, static page => page.ShowChecking());
        checkingTimer.Start();
    }

    void OnPlayPrevious(object sender, RoutedEventArgs e)
    {
        if (missing?.Previous is { } previous)
            ctx.Play(id, previous.ToString(System.Globalization.CultureInfo.InvariantCulture));
    }

    /// <summary>600 ms and no answer: a spinner and Checking fade in inside the button, which keeps its width.</summary>
    void ShowChecking()
    {
        checkingTimer?.Stop();
        if (!checking)
            return;
        checkingShown = DateTimeOffset.UtcNow;
        CheckingRing.IsActive = true;
        Motion.FadeTo(CheckingFace, 1, Motion.FadeInTime);
        Motion.FadeTo(CheckFace, 0, Motion.FadeInTime);
        AutomationProperties.SetName(CheckAgain, T.ActionChecking);
        Announce(T.ActionChecking);
    }

    /// <summary>
    /// A check's answer. One the viewer asked for shows once Checking has stood its 0.3 s: the
    /// held table is drawn, the page follows it, and Narrator hears the answer. A quiet recheck
    /// that found the episode came with its table, which the page has already followed.
    /// </summary>
    void Answered(EpisodeCheck check)
    {
        if (!check.Asked || !checking)
            return;
        checkingTimer?.Stop();
        var wait = CheckWait.Before(checkingShown, DateTimeOffset.UtcNow);
        answerTimer?.Stop();
        if (wait <= TimeSpan.Zero)
        {
            ShowAnswer(check.Outcome);
            return;
        }
        var outcome = check.Outcome;
        answerTimer = DispatcherQueue.CreateTimer();
        answerTimer.Interval = wait;
        answerTimer.IsRepeating = false;
        var weak = new WeakReference<TitlePage>(this);
        answerTimer.Tick += (timer, _) =>
        {
            timer.Stop();
            if (weak.TryGetTarget(out var page))
                page.ShowAnswer(outcome);
        };
        answerTimer.Start();
    }

    void ShowAnswer(CheckOutcome outcome)
    {
        checking = false;
        if (checkingShown is not null)
        {
            Motion.Reveal(CheckingFace, 0, () => CheckingRing.IsActive = false);
            Motion.Reveal(CheckFace, 1);
            CheckAgain.ClearValue(AutomationProperties.NameProperty);
        }
        checkingShown = null;
        if (tableHeld)
        {
            tableHeld = false;
            ShowEpisodes();
        }
        if (Current() is { } entry)
            ShowAction(entry);
        // Found is announced as the page crossfades to Play; the rest are said here, once.
        if (missing is not null && MissingWords.Answered(outcome) is { } said)
            Announce(said);
    }

    /// <summary>Back online or gone offline: the status's last sentence follows while the window lasts.</summary>
    void MissingConnectivity()
    {
        if (missing is not null)
            SayMissing(crossfade: true);
    }

    /// <summary>
    /// The time since airing, on the minute: the status and the unsourced rows read it again with
    /// no motion and nothing announced. A phase that moved on changes the page's words.
    /// </summary>
    void Minute()
    {
        minuteTimer?.Stop();
        if (missing?.AiredAt is not { } aired)
            return;
        minuteTimer = Timers.Weak(this, MissingWords.NextTick(aired, Now()) + TimeSpan.FromMilliseconds(50), false, static page => page.MinuteTicked());
        minuteTimer.Start();
    }

    void StopMinute() => minuteTimer?.Stop();

    void MinuteTicked()
    {
        foreach (var item in Episodes().Where(i => i.Row.Unsourced))
            item.Tick();
        if (Current() is not { } entry || missing is not { } shown)
            return;
        var now = MissingNow(entry);
        if (now is null || now.Phase != shown.Phase)
        {
            ShowAction(entry);
            return;
        }
        missing = now;
        SayMissing(crossfade: false);
        Minute();
    }

    /// <summary>The page asks something else in Play's place (the match question): no status, no motion.</summary>
    void HideMissing()
    {
        missing = null;
        StopMinute();
        Motion.Set(MissingButtons, false);
        Motion.Set(MissingLine, false);
        Lead.Width = double.NaN;
    }

    /// <summary>A season picked in place starts with nothing missing and nothing under way.</summary>
    void ForgetMissing()
    {
        missing = null;
        checking = false;
        checkingShown = null;
        tableHeld = false;
        checkingTimer?.Stop();
        answerTimer?.Stop();
        StopMinute();
        Motion.Set(CheckingFace, true);
        CheckingFace.Opacity = 0;
        CheckingRing.IsActive = false;
        Motion.Set(CheckFace, true);
        CheckAgain.ClearValue(AutomationProperties.NameProperty);
        Motion.Set(MissingButtons, false);
        Motion.Set(MissingLine, false);
        Lead.Width = double.NaN;
    }

    /// <summary>A polite notification event, said once: Narrator reads it after what it is saying.</summary>
    void Announce(string text)
    {
        var peer = FrameworkElementAutomationPeer.FromElement(this) ?? FrameworkElementAutomationPeer.CreatePeerForElement(this);
        peer?.RaiseNotificationEvent(AutomationNotificationKind.ActionCompleted, AutomationNotificationProcessing.MostRecent, text, "kaeri-missing-episode"); // i18n-skip
    }
}
