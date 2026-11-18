using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Report's Player log row, last in Included: what the player did in the last few playbacks, as
/// the engine kept it, saved by hand to a file the reporter attaches to the issue. Nothing is
/// sent. Ticking the row opens a line under it with Save log… and what to do with the file. Save
/// log… is the system's Save As with the engine's file name, and once the redacted text is
/// written the line says the file's name, with Show in folder beside it.
/// </summary>
sealed class ReportPlayerLog
{
    /// <summary>The line under the row: the CheckBox's box and its gap, so the line starts under the words.</summary>
    const double Indent = 28;

    readonly MainWindow window;
    readonly PlayerLogStats stats;
    readonly Func<BugReport> report;
    readonly Fold fold;
    readonly Grid unsaved, saved;
    readonly Button save;
    readonly TextBlock failed, savedAs;
    readonly HyperlinkButton reveal;
    string? savedPath;
    bool written, saving, quiet;

    /// <summary>The row: its title, and how much the log holds or that nothing has played.</summary>
    public CheckBox Box { get; }

    /// <summary>The line under the row, shown while it is ticked.</summary>
    public FrameworkElement Line { get; }

    /// <summary>The row was ticked or unticked, or the file was written: What is sent follows.</summary>
    public event Action? Changed;

    public bool Ticked => Box.IsChecked == true;

    /// <summary>Ticked and saved: the report says the log is attached by hand.</summary>
    public bool Attached => Ticked && written;

    /// <summary>Ticked and not yet saved: Open on GitHub asks for the file first.</summary>
    public bool Unsaved => Ticked && !written;

    public ReportPlayerLog(MainWindow window, PlayerLogStats stats, Func<BugReport> report, double width)
    {
        this.window = window;
        this.stats = stats;
        this.report = report;
        var any = stats.Playbacks > 0;

        var words = new StackPanel();
        words.Children.Add(new TextBlock { Text = T.ReportPlayerLog, TextWrapping = TextWrapping.WrapWholeWords });
        var detail = any ? T.ReportPlayerLogDetail(stats.Playbacks, Ui.Bytes(stats.Bytes)) : T.ReportPlayerLogEmpty;
        words.Children.Add(new TextBlock { Text = detail, Style = Ui.Res<Style>("HazeCaptionStyle"), TextWrapping = TextWrapping.WrapWholeWords });
        Box = new CheckBox { Content = words, IsChecked = false, IsEnabled = any };
        AutomationProperties.SetName(Box, T.ReportPlayerLog);
        AutomationProperties.SetFullDescription(Box, detail);
        AutomationProperties.SetHelpText(Box, T.ReportPlayerLogA11yHint);

        // Save log…, with the failure beside it so another folder is one press away.
        save = new Button { Content = T.ReportPlayerLogSave, VerticalAlignment = VerticalAlignment.Center };
        AutomationProperties.SetHelpText(save, T.ReportPlayerLogA11yHint);
        save.Click += async (_, _) => await Save();
        failed = new TextBlock
        {
            Text = T.ReportPlayerLogFailed,
            Style = Ui.Res<Style>("CaptionTextBlockStyle"),
            Foreground = Ui.Res<Brush>("SystemFillColorCriticalBrush"),
            TextWrapping = TextWrapping.WrapWholeWords,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(10, 0, 0, 0),
            Visibility = Visibility.Collapsed,
        };
        AutomationProperties.SetLiveSetting(failed, AutomationLiveSetting.Assertive);
        unsaved = new Grid { ColumnDefinitions = { new() { Width = GridLength.Auto }, new() { Width = new GridLength(1, GridUnitType.Star) } } };
        unsaved.Children.Add(save);
        Grid.SetColumn(failed, 1);
        unsaved.Children.Add(failed);

        // Saved as …, with Show in folder at its end.
        var tick = new FontIcon { Glyph = "", FontSize = 16, Foreground = Ui.Res<Brush>("WaterBrush"), VerticalAlignment = VerticalAlignment.Center };
        AutomationProperties.SetAccessibilityView(tick, AccessibilityView.Raw);
        savedAs = new TextBlock
        {
            Style = Ui.Res<Style>("CaptionTextBlockStyle"),
            TextTrimming = TextTrimming.CharacterEllipsis,
            TextWrapping = TextWrapping.NoWrap,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(8, 0, 8, 0),
            IsTextSelectionEnabled = true,
        };
        reveal = new HyperlinkButton { Content = T.DownloadsShowInFolder, Style = Ui.Res<Style>("SilkLinkStyle"), VerticalAlignment = VerticalAlignment.Center };
        reveal.Click += (_, _) => ShowInFolder();
        saved = new Grid
        {
            ColumnDefinitions = { new() { Width = GridLength.Auto }, new() { Width = new GridLength(1, GridUnitType.Star) }, new() { Width = GridLength.Auto } },
            Visibility = Visibility.Collapsed,
        };
        saved.Children.Add(tick);
        Grid.SetColumn(savedAs, 1);
        saved.Children.Add(savedAs);
        Grid.SetColumn(reveal, 2);
        saved.Children.Add(reveal);

        // Both share one height, so the dialog holds still while one crossfades to the other.
        var swap = new Grid { MinHeight = 32 };
        swap.Children.Add(unsaved);
        swap.Children.Add(saved);
        var content = new StackPanel { Padding = new Thickness(0, 0, 0, 10), VerticalAlignment = VerticalAlignment.Top };
        content.Children.Add(swap);
        content.Children.Add(new TextBlock { Text = T.ReportPlayerLogAttach, Style = Ui.Res<Style>("HazeCaptionStyle"), TextWrapping = TextWrapping.WrapWholeWords, Margin = new Thickness(0, 6, 0, 0) });
        var host = new Grid { Margin = new Thickness(Indent, 0, 0, 0), Visibility = Visibility.Collapsed };
        host.Children.Add(content);
        Line = host;
        fold = new Fold(host, content, width - Indent);

        Box.Checked += (_, _) => OnTicked(true);
        Box.Unchecked += (_, _) => OnTicked(false);
    }

    /// <summary>Sets the row as Report opens, its line open at once when it starts ticked.</summary>
    public void Start(bool ticked)
    {
        if (!ticked || !Box.IsEnabled)
            return;
        quiet = true;
        Box.IsChecked = true;
        quiet = false;
    }

    void OnTicked(bool on)
    {
        fold.Set(on, animate: !quiet);
        if (!on)
        {
            // A file already saved stays where it is; ticked again, the row asks for one again.
            written = false;
            savedPath = null;
            failed.Visibility = Visibility.Collapsed;
            Motion.Set(saved, false);
            Motion.Set(unsaved, true);
        }
        Changed?.Invoke();
    }

    /// <summary>
    /// Save As with the engine's name filled in, then the redacted log written there. Whether it
    /// was written: false when Save As was cancelled (nothing changes) or the write failed (the
    /// line says so beside Save log…).
    /// </summary>
    public async Task<bool> Save()
    {
        if (saving)
            return false;
        saving = true;
        save.IsEnabled = false;
        try
        {
            global::Windows.Storage.StorageFile? file;
            try
            {
                var picker = new global::Windows.Storage.Pickers.FileSavePicker
                {
                    SuggestedFileName = Path.GetFileNameWithoutExtension(stats.FileName),
                };
                picker.FileTypeChoices.Add(T.ReportPlayerLog, [".txt"]);
                WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(window));
                file = await picker.PickSaveFileAsync();
            }
            catch (Exception e)
            {
                KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"Save As for the player log did not open: {e.Message}");
                Failed();
                return false;
            }
            if (file is null)
                return false;
            try
            {
                var asked = report();
                var text = await Task.Run(() => KaeriFfiMethods.PlayerLog(asked));
                await global::Windows.Storage.FileIO.WriteTextAsync(file, text, global::Windows.Storage.Streams.UnicodeEncoding.Utf8);
            }
            catch (Exception e)
            {
                KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"the player log was not written: {e.GetType().Name}");
                Failed();
                return false;
            }
            // Unticked while Save As was up: the file is there, but the row no longer asks for it.
            if (!Ticked)
                return false;
            Saved(file.Name, file.Path);
            return true;
        }
        finally
        {
            saving = false;
            save.IsEnabled = true;
        }
    }

    void Failed()
    {
        Motion.FadeIn(failed);
    }

    void Saved(string name, string path)
    {
        written = true;
        savedPath = string.IsNullOrEmpty(path) ? null : path;
        failed.Visibility = Visibility.Collapsed;
        savedAs.Text = T.ReportPlayerLogSaved(name);
        ToolTipService.SetToolTip(savedAs, savedPath ?? name);
        reveal.Visibility = savedPath is null ? Visibility.Collapsed : Visibility.Visible;
        var focused = save.FocusState != FocusState.Unfocused;
        // Save log… crossfades to the saved line, both on one height.
        saved.Opacity = 0;
        saved.Visibility = Visibility.Visible;
        Motion.FadeTo(saved, 1, Motion.Standard);
        Motion.FadeOutThen(unsaved, () =>
        {
            if (written)
                unsaved.Visibility = Visibility.Collapsed;
        });
        if (focused)
            (savedPath is null ? (Control)Box : reveal).Focus(FocusState.Programmatic);
        var peer = FrameworkElementAutomationPeer.FromElement(Box) ?? FrameworkElementAutomationPeer.CreatePeerForElement(Box);
        peer?.RaiseNotificationEvent(AutomationNotificationKind.ActionCompleted, AutomationNotificationProcessing.ImportantMostRecent,
            T.ReportPlayerLogA11ySaved, "kaeri-player-log");
        Changed?.Invoke();
    }

    /// <summary>Explorer at the file's folder, with the file selected.</summary>
    void ShowInFolder()
    {
        if (savedPath is not { } path)
            return;
        try
        {
            System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo("explorer.exe", $"/select,\"{path}\"") { UseShellExecute = true });
        }
        catch (Exception e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"the player log's folder did not open: {e.Message}");
        }
    }
}

/// <summary>
/// A part that opens under a row: its height on the standard spring, then its words fading in
/// 60 ms later, so the row never shows words in a gap too small for them. Closing runs it
/// backwards: the words fade out, then the height closes. With animation effects off the height
/// changes at once and the words still fade.
/// </summary>
sealed class Fold(FrameworkElement host, FrameworkElement content, double width)
{
    int turn;

    public bool Open { get; private set; }

    public async void Set(bool open, bool animate)
    {
        var at = ++turn;
        Open = open;
        if (open)
        {
            var from = host.Visibility == Visibility.Visible ? host.ActualHeight : 0;
            host.Visibility = Visibility.Visible;
            if (!animate)
            {
                host.Height = double.NaN;
                content.Opacity = 1;
                return;
            }
            if (from <= 0)
                content.Opacity = 0;
            content.Measure(new global::Windows.Foundation.Size(width, double.PositiveInfinity));
            var to = content.DesiredSize.Height;
            host.Height = from;
            Motion.Ease(this, from, to, v =>
            {
                if (turn == at)
                    host.Height = Math.Abs(v - to) < 0.01 ? double.NaN : v;
            });
            await Task.Delay(60);
            if (turn == at)
                Motion.FadeTo(content, 1, Motion.FadeInTime);
            return;
        }
        if (host.Visibility == Visibility.Collapsed)
            return;
        if (!animate)
        {
            host.Visibility = Visibility.Collapsed;
            host.Height = double.NaN;
            return;
        }
        Motion.FadeTo(content, 0, Motion.FadeOutTime);
        await Task.Delay(80);
        if (turn != at)
            return;
        Motion.Ease(this, host.ActualHeight, 0, v =>
        {
            if (turn != at)
                return;
            host.Height = v;
            if (v <= 0.01)
            {
                host.Visibility = Visibility.Collapsed;
                host.Height = double.NaN;
            }
        });
    }
}
