using Kaeri.App;
using Kaeri.Engine.Core;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Report a bug: a sheet that says what a report carries before anything leaves. The versions
/// always go, the last error and each saved crash from the last seven days are boxes, and Show
/// what is sent reads out the exact text. Nothing is sent by the app: Open on GitHub opens the
/// pre-filled public issue in the browser, and Copy report puts the whole text on the clipboard.
/// The Player log row saves the last playbacks' log as a file to attach by hand (<see cref="ReportPlayerLog"/>).
/// </summary>
static class ReportDialog
{
    /// <summary>The crashes a report may carry: the last seven days.</summary>
    static readonly TimeSpan Window = TimeSpan.FromDays(7);

    /// <summary>The width of the dialog's words.</summary>
    const double Width = 480;

    /// <summary>
    /// Opens Report. <paramref name="latestCrash"/>: from the crash notice, with that crash ticked.
    /// <paramref name="fromError"/>: from an error's Report action, so a playback error starts
    /// with the Player log ticked.
    /// </summary>
    public static async void Show(Ctx ctx, XamlRoot root, bool latestCrash = false, bool fromError = false)
    {
        var saving = SettingsCatalog.IsOn(ctx.Known.Setting(SettingKey.SaveCrashReports));
        var crashes = saving
            ? KaeriFfiMethods.CrashList().Where(c => DateTimeOffset.UtcNow - DateTimeOffset.FromUnixTimeMilliseconds(c.AtMs) < Window).ToList()
            : [];
        ContentDialog? dialog = null;
        var body = new StackPanel { Spacing = 10, Width = Width };
        body.Children.Add(Ui.Text(T.ReportDetail, "BodyTextBlockStyle"));
        body.Children.Add(new TextBlock { Text = T.ReportIncluded, Style = (Style)Application.Current.Resources["BodyStrongTextBlockStyle"], Margin = new Thickness(0, 6, 0, 0) });

        var versions = new CheckBox { Content = T.ReportVersions, IsChecked = true, IsEnabled = false };
        body.Children.Add(versions);
        CheckBox? error = null;
        if (ctx.Known.LastError is { } last)
        {
            error = new CheckBox { Content = T.ReportLastError, IsChecked = true };
            body.Children.Add(error);
        }
        var crashBoxes = new List<(CheckBox Box, string Id)>();
        foreach (var crash in crashes)
        {
            var when = DateTimeOffset.FromUnixTimeMilliseconds(crash.AtMs).ToLocalTime().ToString("d MMMM, HH:mm");
            var box = new CheckBox
            {
                Content = T.ReportCrashItem(when, Place(crash.Place)),
                // On when the sheet opened from the crash notice, and for the crash it was about.
                IsChecked = latestCrash && crash == crashes[0],
            };
            crashBoxes.Add((box, crash.Id));
            body.Children.Add(box);
        }
        // Player log, last in Included.
        ReportPlayerLog? log = null;
        var stats = KaeriFfiMethods.PlayerLogStats();
        var row = new ReportPlayerLog(ctx.Window, stats, () => Report(), Width);
        log = row;
        body.Children.Add(row.Box);
        body.Children.Add(row.Line);

        if (!saving)
        {
            body.Children.Add(Ui.Haze(T.ReportCrashOff));
            var link = new HyperlinkButton { Content = T.ReportCrashSettings, Style = Ui.Res<Style>("SilkLinkLeadingStyle") };
            body.Children.Add(link);
            link.Click += (_, _) =>
            {
                Settings = true;
                dialog?.Hide();
            };
        }

        // What is sent: the report, and while the log is ticked, Report and Player log segments.
        var sent = new SentBox(() => Report(), row, Width);
        body.Children.Add(sent.Toggle);
        body.Children.Add(sent.Panel);
        body.Children.Add(Ui.Haze(T.ReportPrivacy, wrap: true));

        BugReport Report() => Made(error is { IsChecked: true } ? ctx.Known.LastError : null, crashBoxes.Where(c => c.Box.IsChecked == true).Select(c => c.Id).ToArray(), log?.Attached == true);

        BugReport Made(BugError? withError, string[] withCrashes, bool attached) => new(
            Updater.Current,
            "Windows",
            Environment.OSVersion.Version.ToString(),
            DeviceClass.Computer,
            ctx.Options.Fixture ? "fixtures" : Build,
            ctx.Known.Providers,
            withError,
            [],
            withCrashes,
            attached);

        // Ticked from a playback error's Report or with a crash in the player ticked; from Settings, never.
        var startsTicked = stats.Playbacks > 0 && KaeriFfiMethods.PlayerLogStartsTicked(Made(
            fromError ? ctx.Known.LastError : null,
            crashBoxes.Where(c => c.Box.IsChecked == true).Select(c => c.Id).ToArray(),
            false));
        row.Start(startsTicked);
        sent.Follow();
        row.Changed += sent.Follow;

        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.ReportTitle,
            Content = body,
            PrimaryButtonText = T.ReportOpenGithub,
            SecondaryButtonText = T.ReportCopy,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
        };
        var shown = dialog;
        dialog.SecondaryButtonClick += (_, e) =>
        {
            e.Cancel = true;
            if (Ui.TryCopy(KaeriFfiMethods.ReportText(Report())))
                shown.SecondaryButtonText = T.ReportCopied;
        };
        // With the log ticked and not saved, Save As comes first: saved, GitHub opens; cancelled
        // or failed, the dialog stays and nothing opens.
        dialog.PrimaryButtonClick += async (_, e) =>
        {
            if (!row.Unsaved)
                return;
            var deferral = e.GetDeferral();
            try
            {
                if (!await row.Save())
                    e.Cancel = true;
            }
            finally
            {
                deferral.Complete();
            }
        };
        // Started ticked: keyboard focus on the row, where Space unticks it.
        if (startsTicked)
            dialog.Opened += (_, _) => row.Box.Focus(FocusState.Programmatic);
        Settings = false;
        var result = await Dialogs.Show(dialog);
        if (result == ContentDialogResult.Primary)
        {
            try
            {
                await global::Windows.System.Launcher.LaunchUriAsync(new Uri(KaeriFfiMethods.BugReportUrl(Report())));
            }
            catch (Exception e)
            {
                KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"the bug report would not open: {e.Message}");
            }
        }
        else if (Settings)
            ctx.Window.ShowSettings("about");
    }

    /// <summary>The link to Crash report settings was used: the sheet closes and About opens.</summary>
    static bool Settings;

    static string Build =>
#if DEBUG
        "debug";
#else
        "release";
#endif

    /// <summary>Where it was, in the viewer's language.</summary>
    internal static string Place(CrashPlace place) => place switch
    {
        CrashPlace.Home => T.CrashPlaceHome,
        CrashPlace.Player => T.CrashPlacePlayer,
        CrashPlace.Title => T.CrashPlaceTitle,
        CrashPlace.Search => T.CrashPlaceSearch,
        CrashPlace.Settings => T.CrashPlaceSettings,
        _ => T.CrashPlaceOther,
    };
}

/// <summary>
/// What is sent: Show what is sent opens the report's exact text. While the Player log is ticked,
/// Report and Player log segments sit over the text, and Player log shows the redacted file from
/// its first line. The box keeps one height between the two, so nothing under it moves.
/// </summary>
sealed class SentBox
{
    const double BoxHeight = 200;

    readonly Func<BugReport> report;
    readonly ReportPlayerLog log;
    readonly TextBox reportText, logText;
    readonly SelectorBarItem reportItem, logItem;
    readonly Grid box;
    readonly Fold fold;
    bool onLog, logRead;
    int reads;

    public HyperlinkButton Toggle { get; }

    /// <summary>The segments and the text, shown by <see cref="Toggle"/>.</summary>
    public StackPanel Panel { get; }

    bool Shown => Panel.Visibility == Visibility.Visible;

    public SentBox(Func<BugReport> report, ReportPlayerLog log, double width)
    {
        this.report = report;
        this.log = log;
        reportText = Mono(T.ReportShowSent);
        logText = Mono(T.ReportPlayerLog);
        logText.Visibility = Visibility.Collapsed;
        box = new Grid();
        box.Children.Add(reportText);
        box.Children.Add(logText);

        reportItem = new SelectorBarItem { Text = T.ReportSentReport, IsSelected = true };
        logItem = new SelectorBarItem { Text = T.ReportPlayerLog };
        var segments = new SelectorBar { Margin = new Thickness(0, 0, 0, 10) };
        segments.Items.Add(reportItem);
        segments.Items.Add(logItem);
        segments.SelectionChanged += (bar, _) => Select(bar.SelectedItem == logItem);
        var segmentsHost = new Grid { Visibility = Visibility.Collapsed };
        segmentsHost.Children.Add(segments);
        fold = new Fold(segmentsHost, segments, width);

        Panel = new StackPanel { Visibility = Visibility.Collapsed };
        Panel.Children.Add(segmentsHost);
        Panel.Children.Add(box);

        Toggle = new HyperlinkButton { Content = T.ReportShowSent, Style = Ui.Res<Style>("SilkLinkLeadingStyle") };
        Toggle.Click += (_, _) =>
        {
            var open = !Shown;
            Panel.Visibility = open ? Visibility.Visible : Visibility.Collapsed;
            Toggle.Content = open ? T.ReportHideSent : T.ReportShowSent;
            if (open)
                Refresh();
        };
    }

    static TextBox Mono(string name)
    {
        var text = new TextBox
        {
            IsReadOnly = true,
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
            FontFamily = new FontFamily("Cascadia Mono"),
            FontSize = 12,
            MaxHeight = BoxHeight,
        };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(text, name);
        return text;
    }

    /// <summary>The Player log row changed: the segments come or go, and the report's text follows.</summary>
    public void Follow()
    {
        fold.Set(log.Ticked, animate: Shown);
        box.Height = log.Ticked ? BoxHeight : double.NaN;
        if (!log.Ticked && onLog)
            reportItem.IsSelected = true;
        if (Shown)
            Refresh();
    }

    void Refresh()
    {
        reportText.Text = KaeriFfiMethods.ReportText(report());
        if (onLog)
            ReadLog();
    }

    /// <summary>Report or Player log: the text crossfades, the box holding its height.</summary>
    void Select(bool toLog)
    {
        if (toLog == onLog)
            return;
        onLog = toLog;
        var (from, to) = toLog ? (reportText, logText) : (logText, reportText);
        to.Opacity = 0;
        to.Visibility = Visibility.Visible;
        Motion.FadeTo(to, 1, Motion.Standard);
        Motion.FadeOutThen(from, () =>
        {
            if (onLog == toLog)
                from.Visibility = Visibility.Collapsed;
        });
        if (toLog)
            ReadLog();
    }

    /// <summary>The redacted file, read off the UI thread once: it redacts up to a megabyte.</summary>
    async void ReadLog()
    {
        if (logRead)
            return;
        logRead = true;
        var at = ++reads;
        var asked = report();
        try
        {
            var text = await Task.Run(() => KaeriFfiMethods.PlayerLog(asked));
            if (at == reads)
                logText.Text = text;
        }
        catch (Exception e)
        {
            logRead = false;
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"the player log could not be read: {e.GetType().Name}");
        }
    }
}
