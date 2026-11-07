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
/// </summary>
static class ReportDialog
{
    /// <summary>The crashes a report may carry: the last seven days.</summary>
    static readonly TimeSpan Window = TimeSpan.FromDays(7);

    public static async void Show(Ctx ctx, XamlRoot root, bool latestCrash = false)
    {
        var saving = SettingsCatalog.IsOn(ctx.Known.Setting(SettingKey.SaveCrashReports));
        var crashes = saving
            ? KaeriFfiMethods.CrashList().Where(c => DateTimeOffset.UtcNow - DateTimeOffset.FromUnixTimeMilliseconds(c.AtMs) < Window).ToList()
            : [];
        ContentDialog? dialog = null;
        var body = new StackPanel { Spacing = 10, Width = 480 };
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

        var sent = new TextBox
        {
            IsReadOnly = true,
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
            FontFamily = new FontFamily("Cascadia Mono"),
            FontSize = 12,
            MaxHeight = 200,
            Visibility = Visibility.Collapsed,
        };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(sent, T.ReportShowSent);
        var toggle = new HyperlinkButton { Content = T.ReportShowSent, Style = Ui.Res<Style>("SilkLinkLeadingStyle") };
        toggle.Click += (_, _) =>
        {
            var open = sent.Visibility == Visibility.Collapsed;
            sent.Visibility = open ? Visibility.Visible : Visibility.Collapsed;
            toggle.Content = open ? T.ReportHideSent : T.ReportShowSent;
            if (open)
                sent.Text = KaeriFfiMethods.ReportText(Report());
        };
        body.Children.Add(toggle);
        body.Children.Add(sent);
        body.Children.Add(Ui.Haze(T.ReportPrivacy, wrap: true));

        BugReport Report() => new(
            Updater.Current,
            "Windows",
            Environment.OSVersion.Version.ToString(),
            DeviceClass.Computer,
            ctx.Options.Fixture ? "fixtures" : Build,
            ctx.Known.Providers,
            error is { IsChecked: true } ? ctx.Known.LastError : null,
            [],
            crashBoxes.Where(c => c.Box.IsChecked == true).Select(c => c.Id).ToArray());

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
