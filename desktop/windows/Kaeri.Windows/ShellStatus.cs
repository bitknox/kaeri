using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Kaeri.Windows.Pages;

namespace Kaeri.Windows;

/// <summary>
/// The title bar's status, the Mac toolbar's activity indicator and sync capsule: what has run
/// past 600 ms (a small ring and the newest job's words, faded in and out in a place kept for
/// them), then where sync with the trackers stands as a button with a dot. The button opens a
/// flyout with every running job and how long it has run, the sync pass, and the ways to
/// Accounts, Devices and Messages. Its width never changes with what it says, so the search box
/// beside it never moves; words change in place, only when they differ.
/// </summary>
sealed class ShellStatus : Grid
{
    /// <summary>Width kept for the status in a wide window; a narrow one keeps only the dot.</summary>
    const double WideWidth = 360, NarrowWidth = 44;

    readonly Ctx ctx;
    readonly Action openAccounts, openDevices, openMessages;
    readonly StackPanel activity = new() { Orientation = Orientation.Horizontal, Spacing = 8, HorizontalAlignment = HorizontalAlignment.Right, VerticalAlignment = VerticalAlignment.Center, Opacity = 0, Margin = new Thickness(0, 0, 8, 0) };
    readonly ProgressRing ring = new() { Width = 14, Height = 14, MinWidth = 14, MinHeight = 14, IsActive = false };
    readonly TextBlock activityText = new() { VerticalAlignment = VerticalAlignment.Center, TextTrimming = TextTrimming.CharacterEllipsis, FontSize = 12 };
    readonly Button button = new() { VerticalAlignment = VerticalAlignment.Center, Padding = new Thickness(10, 4, 10, 4), MinHeight = 30 };
    readonly Ellipse dot = new() { Width = 6, Height = 6, VerticalAlignment = VerticalAlignment.Center };
    readonly TextBlock words = new() { VerticalAlignment = VerticalAlignment.Center, TextTrimming = TextTrimming.CharacterEllipsis, MaxWidth = 180, FontSize = 12 };
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer clock;
    readonly Flyout flyout = new() { Placement = FlyoutPlacementMode.BottomEdgeAlignedRight };
    /// <summary>When the device sync pass now running was first seen, so it shows only past 600 ms.</summary>
    DateTimeOffset? passSince;
    bool narrow, activityShown;
    SyncDot? dotDrawn;
    string? jobsDrawn;
    readonly StackPanel flyoutJobs = new() { Spacing = 8 };
    readonly TextBlock flyoutSummary = new() { FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, TextWrapping = TextWrapping.WrapWholeWords };
    readonly Grid flyoutPass = new() { ColumnSpacing = 12 };
    readonly TextBlock passWords = new() { TextWrapping = TextWrapping.WrapWholeWords }, passTime = new();
    readonly Button devices = new() { Content = T.TitlebarOpenDevices };
    readonly List<(Job Job, TextBlock Elapsed, TextBlock Note)> jobRows = [];

    public ShellStatus(Ctx ctx, Action openAccounts, Action openDevices, Action openMessages)
    {
        this.ctx = ctx;
        this.openAccounts = openAccounts;
        this.openDevices = openDevices;
        this.openMessages = openMessages;
        Width = WideWidth;
        VerticalAlignment = VerticalAlignment.Center;
        Margin = new Thickness(0, 0, 8, 0);
        ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        activityText.Foreground = Ui.Res<Brush>("HazeBrush");
        activity.Children.Add(ring);
        activity.Children.Add(activityText);
        AutomationProperties.SetLiveSetting(activity, Microsoft.UI.Xaml.Automation.Peers.AutomationLiveSetting.Polite);
        Children.Add(activity);
        var inner = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        inner.Children.Add(dot);
        inner.Children.Add(words);
        button.Content = inner;
        button.Flyout = flyout;
        button.Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        button.BorderThickness = new Thickness(0);
        AutomationProperties.SetHelpText(button, T.TitlebarStatusHint);
        SetColumn(button, 1);
        Children.Add(button);
        clock = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread().CreateTimer();
        clock.Interval = TimeSpan.FromMilliseconds(250);
        clock.Tick += (_, _) => Tick();
        flyout.Content = FlyoutBody();
        flyout.Opening += (_, _) =>
        {
            Tick();
            clock.Start();
        };
        Tick();
    }

    /// <summary>The engine's word arrived: jobs, sync, the network.</summary>
    public void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.JobStarted or Event.JobNote or Event.JobDone or Event.Sync or Event.SyncStatus or Event.Connectivity))
        {
            Tick();
            if (ctx.Known.Jobs.Running.Count > 0 || ctx.Known.SyncGroup is { Syncing: true })
                clock.Start();
        }
    }

    /// <summary>Opens the flyout, as a click on the status does.</summary>
    public void Open() => flyout.ShowAt(button);

    /// <summary>A narrow window keeps the dot alone, so the search box keeps its room.</summary>
    public void Fit(bool narrow)
    {
        if (this.narrow == narrow)
            return;
        this.narrow = narrow;
        Width = narrow ? NarrowWidth : WideWidth;
        activity.Visibility = narrow ? Visibility.Collapsed : Visibility.Visible;
        words.Visibility = narrow ? Visibility.Collapsed : Visibility.Visible;
    }

    void Tick()
    {
        var now = DateTimeOffset.UtcNow;
        var syncing = ctx.Known.SyncGroup is { Syncing: true };
        if (!syncing)
            passSince = null;
        else
            passSince ??= now;
        var passShown = passSince is { } since && now - since >= Jobs.Delay;
        var visible = ctx.Known.Jobs.Visible(now);
        var summary = Summary.Of(ctx.Known.Sync, ctx.Known.Offline);
        var (text, shade) = passShown ? (T.SyncSyncing, SyncDot.Waiting) : (summary.Text, summary.Dot);
        Set(words, text);
        if (dotDrawn != shade)
        {
            dotDrawn = shade;
            dot.Fill = shade switch
            {
                SyncDot.Synced => Ui.Res<Brush>("WaterBrush"),
                SyncDot.Problem => Ui.Res<Brush>("SystemFillColorCriticalBrush"),
                SyncDot.Waiting => Ui.Res<Brush>("HazeBrush"),
                _ => new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            };
            dot.Stroke = shade == SyncDot.SignedOut ? Ui.Res<Brush>("HazeBrush") : null;
            dot.StrokeThickness = shade == SyncDot.SignedOut ? 1.5 : 0;
        }
        var name = visible.Count > 0 ? $"{visible[0].Words}, {text}" : text;
        if (AutomationProperties.GetName(button) != name)
            AutomationProperties.SetName(button, name);
        if (ToolTipService.GetToolTip(button) as string != text)
            ToolTipService.SetToolTip(button, text);

        // The newest job's words, faded in once it has run 600 ms and out when nothing is left.
        if (visible.Count > 0)
            Set(activityText, visible[0].Words);
        var show = visible.Count > 0;
        if (show != activityShown)
        {
            activityShown = show;
            ring.IsActive = show;
            Motion.FadeTo(activity, show ? 1 : 0, show ? Motion.FadeInTime : Motion.FadeOutTime);
        }
        if (flyout.IsOpen)
            FillFlyout(now, summary.Text, passShown);
        // The clock runs while there is something to count; it stops once all is quiet and the flyout shut.
        if (ctx.Known.Jobs.Running.Count == 0 && !syncing && !flyout.IsOpen)
            clock.Stop();
    }

    static void Set(TextBlock block, string text)
    {
        if (block.Text != text)
            block.Text = text;
    }

    FrameworkElement FlyoutBody()
    {
        var body = new StackPanel { Width = 300, Spacing = 0 };
        var passTimeColumn = new ColumnDefinition { Width = GridLength.Auto };
        flyoutPass.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        flyoutPass.ColumnDefinitions.Add(passTimeColumn);
        passTime.Foreground = Ui.Res<Brush>("HazeBrush");
        SetColumn(passTime, 1);
        flyoutPass.Children.Add(passWords);
        flyoutPass.Children.Add(passTime);
        flyoutPass.Margin = new Thickness(0, 0, 0, 12);
        body.Children.Add(flyoutPass);
        body.Children.Add(flyoutSummary);
        flyoutJobs.Margin = new Thickness(0, 10, 0, 0);
        body.Children.Add(flyoutJobs);
        body.Children.Add(new Border { Height = 1, Background = Ui.Res<Brush>("FaintRuleBrush"), Margin = new Thickness(0, 12, 0, 12) });
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var accounts = new Button { Content = T.AccountsSettings };
        accounts.Click += (_, _) =>
        {
            flyout.Hide();
            openAccounts();
        };
        devices.Click += (_, _) =>
        {
            flyout.Hide();
            openDevices();
        };
        var messages = new Button { Content = T.MessagesTitle };
        messages.Click += (_, _) =>
        {
            flyout.Hide();
            openMessages();
        };
        buttons.Children.Add(accounts);
        buttons.Children.Add(devices);
        buttons.Children.Add(messages);
        body.Children.Add(buttons);
        return body;
    }

    /// <summary>
    /// The flyout as things stand: the rows are made again only when which jobs run changes; the
    /// elapsed times and notes change in place each tick.
    /// </summary>
    void FillFlyout(DateTimeOffset now, string summary, bool passShown)
    {
        flyoutPass.Visibility = passShown ? Visibility.Visible : Visibility.Collapsed;
        if (passShown)
        {
            var partner = ctx.Known.SyncGroup?.Devices.FirstOrDefault(d => !d.ThisDevice)?.Name;
            Set(passWords, partner is null ? T.SyncSyncing : T.SyncSyncingWith(partner));
            Set(passTime, Summary.Elapsed(now - (passSince ?? now)));
        }
        Set(flyoutSummary, summary);
        devices.Visibility = ctx.Known.SyncGroup is { Enabled: true } ? Visibility.Visible : Visibility.Collapsed;
        var running = ctx.Known.Jobs.Running;
        var key = string.Join(",", running.Select(j => j.Id));
        if (key != jobsDrawn)
        {
            jobsDrawn = key;
            flyoutJobs.Children.Clear();
            jobRows.Clear();
            if (running.Count == 0)
                flyoutJobs.Children.Add(new TextBlock { Text = T.TitlebarNothingRunning, Foreground = Ui.Res<Brush>("HazeBrush") });
            foreach (var job in running)
            {
                var row = new Grid { ColumnSpacing = 12 };
                row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
                row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
                var lines = new StackPanel { Spacing = 2 };
                lines.Children.Add(new TextBlock { Text = job.Activity.FirstUpper(), TextWrapping = TextWrapping.WrapWholeWords });
                var note = new TextBlock { Foreground = Ui.Res<Brush>("HazeBrush"), TextWrapping = TextWrapping.WrapWholeWords };
                lines.Children.Add(note);
                var elapsed = new TextBlock { Foreground = Ui.Res<Brush>("HazeBrush") };
                SetColumn(elapsed, 1);
                row.Children.Add(lines);
                row.Children.Add(elapsed);
                flyoutJobs.Children.Add(row);
                jobRows.Add((job, elapsed, note));
            }
        }
        foreach (var (job, elapsed, note) in jobRows)
        {
            Set(elapsed, Summary.Elapsed(now - job.Started));
            var said = job.WaitUntil is { } until && until > now ? $"{job.Note} · {Summary.Elapsed(until - now)}" : job.Note ?? "";
            Set(note, said);
            note.Visibility = said.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
        }
    }
}
