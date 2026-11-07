using Medium = Kaeri.Engine.Core.Medium;
using Microsoft.UI.Xaml.Automation.Peers;
using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Send to e-reader: which e-reader, which chapters and what they come to, then Prepare. The
/// button gives way to the progress, the engine's waiting line with Cancel, the files to share
/// or save, or why it failed. Closing while it runs does not cancel it: the series page keeps a
/// status row. The pre-flight checks the engine makes come back as the failure's words. The twin
/// of the Apple app's SendSheet.
/// </summary>
static class SendDialog
{
    /// <summary>The e-readers the board lists by name; every other screen sits under Other.</summary>
    static readonly string[] Listed = ["kobo", "kindle"];
    /// <summary>The size where a file is split, near 200 MB.</summary>
    const ulong PartsOver = 200UL * 1_000_000;

    /// <summary>The send the open dialog shows, which the series page's status rows leave out.</summary>
    public static int? Showing { get; private set; }

    /// <summary>
    /// Opens the dialog for <paramref name="chapters"/> of <paramref name="series"/>. With
    /// <paramref name="send"/> it shows a send already under way (a shot's staged one).
    /// </summary>
    public static Task Show(Ctx ctx, XamlRoot root, SeriesView series, IReadOnlyList<string> chapters, int? send = null) =>
        Show(ctx, root, series.Work, series.Titles.Display, chapters, send);

    /// <summary>
    /// Opens the dialog for <paramref name="chapters"/> of <paramref name="work"/> where no series
    /// page is open, as Downloads does for a series' downloaded chapters. The series' details are
    /// asked for if not known yet; whether it is a novel follows once they come.
    /// </summary>
    public static async Task Show(Ctx ctx, XamlRoot root, string work, string? title, IReadOnlyList<string> chapters, int? send = null)
    {
        var reading = ctx.Known.Reading;
        var sending = reading.Sending;
        bool Novel() => reading.Series.GetValueOrDefault(work)?.Medium == Medium.Novel;
        var novel = Novel();
        string Title() => reading.Series.GetValueOrDefault(work)?.Titles.Display is { Length: > 0 } known ? known : title ?? "";
        var ids = new List<string>(chapters);
        string? chosen = sending.Get(send ?? -1)?.Target;
        int? sendId = send;
        var waited = false;

        var body = new StackPanel { Spacing = 6 };
        var targetsPart = new StackPanel { Spacing = 4 };
        var whatPart = new StackPanel { Spacing = 4 };
        var notes = new StackPanel { Spacing = 6 };
        var progressPart = new StackPanel { Spacing = 8, Margin = new Thickness(0, 8, 0, 0) };
        if (Title() is { Length: > 0 } named)
            body.Children.Add(Ui.Haze(named));
        body.Children.Add(Heading(T.SendFor));
        body.Children.Add(targetsPart);
        body.Children.Add(Heading(T.SendWhat));
        body.Children.Add(whatPart);
        body.Children.Add(notes);
        body.Children.Add(progressPart);

        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.SendTitle,
            Content = SeriesWords.Scroll(body, 480),
            DefaultButton = ContentDialogButton.Primary,
        };

        ExportTarget? Target() => sending.Targets.FirstOrDefault(t => t.Id == chosen) ?? sending.DefaultTarget;
        EReaderSend? Send() => sendId is { } id ? sending.Get(id) : null;

        // For: the listed e-readers, then Other with a box of the rest.
        var radios = new RadioButtons();
        AutomationProperties.SetName(radios, T.SendFor);
        var otherBox = new ComboBox { MinWidth = 260, Margin = new Thickness(28, 0, 0, 4) };
        AutomationProperties.SetName(otherBox, T.SendFor);
        var drawingTargets = false;
        string? drawnTargets = null;
        void DrawTargets()
        {
            var targets = sending.Targets;
            var target = Target();
            var print = Drawn.Fingerprint((targets.Select(t => t.Id).ToList(), target?.Id, Send()?.IsRunning, novel));
            if (print == drawnTargets)
                return;
            drawnTargets = print;
            drawingTargets = true;
            targetsPart.Children.Clear();
            radios.Items.Clear();
            otherBox.Items.Clear();
            var primary = targets.Where(t => Listed.Contains(t.Id)).ToList();
            var others = targets.Where(t => !Listed.Contains(t.Id)).ToList();
            foreach (var item in primary)
                radios.Items.Add(TargetRadio(item.Name, SeriesWords.TargetLine(item, novel), item.Id));
            var keptOther = others.FirstOrDefault(t => t.Id == target?.Id);
            if (others.Count > 0)
            {
                var shown = keptOther ?? others.FirstOrDefault(t => t.Id == "other") ?? others[0];
                var name = keptOther is { } k && k.Id != "other" ? k.Name : others.FirstOrDefault(t => t.Id == "other")?.Name ?? shown.Name;
                radios.Items.Add(TargetRadio(name, SeriesWords.TargetLine(shown, novel), "\u0001other"));
                foreach (var item in others)
                {
                    var entry = new ComboBoxItem { Content = SeriesWords.Pair(item.Name, SeriesWords.TargetLine(item, novel)), Tag = item.Id };
                    AutomationProperties.SetName(entry, item.Name);
                    otherBox.Items.Add(entry);
                }
                otherBox.SelectedIndex = keptOther is null ? -1 : others.IndexOf(keptOther);
            }
            radios.SelectedIndex = target is null ? -1 : keptOther is not null ? radios.Items.Count - 1 : primary.FindIndex(t => t.Id == target.Id);
            radios.IsEnabled = otherBox.IsEnabled = Send()?.IsRunning != true;
            targetsPart.Children.Add(radios);
            if (keptOther is not null)
                targetsPart.Children.Add(otherBox);
            drawingTargets = false;
        }
        // The handlers below draw through this, set once Draw exists further down.
        Action redraw = () => { };
        Action refill = () => { };
        radios.SelectionChanged += (_, _) =>
        {
            if (drawingTargets || radios.SelectedItem is not RadioButton { Tag: string id })
                return;
            if (id == "\u0001other")
            {
                var others = sending.Targets.Where(t => !Listed.Contains(t.Id)).ToList();
                chosen = (others.FirstOrDefault(t => t.Id == chosen) ?? others.FirstOrDefault(t => t.Id == "other") ?? others.FirstOrDefault())?.Id;
            }
            else
            {
                chosen = id;
            }
            redraw();
        };
        otherBox.SelectionChanged += (_, _) =>
        {
            if (drawingTargets || otherBox.SelectedItem is not ComboBoxItem { Tag: string id })
                return;
            chosen = id;
            redraw();
        };

        // What: the chapters, which open in place to change, and what they come to.
        var picker = new StackPanel { Spacing = 0 };
        var chaptersButton = new Button
        {
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Padding = new Thickness(12, 8, 12, 8),
        };
        var chaptersValue = Ui.Text("", "HazeBodyStyle", wrap: false);
        var chevron = new FontIcon { Glyph = "", FontSize = 12 };
        var chaptersLine = new Grid { ColumnSpacing = 8 };
        chaptersLine.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        chaptersLine.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        chaptersLine.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        chaptersLine.Children.Add(Ui.Text(T.SendChapters, null, wrap: false));
        Grid.SetColumn(chaptersValue, 1);
        chaptersLine.Children.Add(chaptersValue);
        Grid.SetColumn(chevron, 2);
        chaptersLine.Children.Add(chevron);
        chaptersButton.Content = chaptersLine;
        var picking = false;
        chaptersButton.Click += (_, _) =>
        {
            picking = !picking;
            refill();
            redraw();
        };
        var sizeValue = Ui.Text("", "HazeBodyStyle", wrap: false);
        AutomationProperties.SetLiveSetting(sizeValue, AutomationLiveSetting.Polite);
        var sizeLine = new Grid { ColumnSpacing = 8, Padding = new Thickness(12, 6, 12, 6) };
        sizeLine.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        sizeLine.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        sizeLine.Children.Add(Ui.Text(T.SendSize, null, wrap: false));
        Grid.SetColumn(sizeValue, 1);
        sizeLine.Children.Add(sizeValue);
        whatPart.Children.Add(chaptersButton);
        whatPart.Children.Add(picker);
        whatPart.Children.Add(sizeLine);

        void FillPicker()
        {
            picker.Children.Clear();
            if (!picking)
                return;
            var volumes = reading.Chapters.GetValueOrDefault(work)?.Volumes ?? [];
            var list = new StackPanel { Spacing = 0 };
            var byVolume = volumes.Length > 1;
            foreach (var group in volumes)
            {
                if (byVolume)
                    list.Children.Add(Ui.Text(SeriesWords.Volume(group, novel).Name, "BodyStrongTextBlockStyle"));
                foreach (var row in group.Chapters)
                {
                    var box = new CheckBox { Content = SeriesWords.ChapterName(row), IsChecked = ids.Contains(row.Id) };
                    var id = row.Id;
                    box.Checked += (_, _) =>
                    {
                        if (!ids.Contains(id))
                            ids.Add(id);
                        Picked();
                    };
                    box.Unchecked += (_, _) =>
                    {
                        ids.Remove(id);
                        Picked();
                    };
                    list.Children.Add(box);
                }
            }
            picker.Children.Add(new ScrollViewer { Content = list, MaxHeight = 220, Padding = new Thickness(28, 0, 12, 0) });
        }

        var estimateClock = ctx.Window.DispatcherQueue.CreateTimer();
        estimateClock.Interval = TimeSpan.FromMilliseconds(150);
        estimateClock.IsRepeating = false;
        var workingClock = ctx.Window.DispatcherQueue.CreateTimer();
        workingClock.Interval = TimeSpan.FromMilliseconds(600);
        workingClock.IsRepeating = false;
        estimateClock.Tick += (_, _) =>
        {
            reading.EstimateDownload(work, new DownloadPick.Chapters([.. Ordered()]));
            workingClock.Start();
        };
        workingClock.Tick += (_, _) =>
        {
            waited = true;
            redraw();
        };
        void Picked()
        {
            waited = false;
            estimateClock.Stop();
            workingClock.Stop();
            estimateClock.Start();
            redraw();
        }

        // The chapters in reading order.
        List<ChapterRow> Rows() => [.. (reading.Chapters.GetValueOrDefault(work)?.All ?? []).Where(c => ids.Contains(c.Id))];
        List<string> Ordered() => [.. Rows().Select(r => r.Id)];

        ulong? Size()
        {
            var rows = Rows();
            if (rows.Count == 0)
                return null;
            var held = rows.Count(r => r.Download is ChapterDownloadState.Done);
            ulong bytes = 0;
            if (held > 0)
            {
                if (reading.Downloads?.Series.FirstOrDefault(s => s.Work == work) is not { Chapters: > 0 } kept)
                    return null;
                bytes += kept.Bytes / kept.Chapters * (ulong)held;
            }
            if (held < rows.Count)
            {
                if (!reading.Estimates.TryGetValue(work, out var estimate) || estimate.Pick is not DownloadPick.Chapters picked || !picked.Ids.SequenceEqual(Ordered()))
                    return null;
                bytes += estimate.Bytes;
            }
            return bytes;
        }

        string ChaptersWords()
        {
            var rows = Rows();
            if (rows.Count == 0)
                return "–";
            string Label(ChapterRow r) => r.Number is { Length: > 0 } n ? n : r.Label;
            return rows.Count == 1 ? T.SendChaptersOne(Label(rows[0])) : T.SendChaptersRange(Label(rows[0]), Label(rows[^1]));
        }

        // The foot: Prepare, then the send's progress where it was.
        var bar = new ProgressBar { Minimum = 0, Maximum = 100 };
        var progressWords = Ui.Text("", "BodyTextBlockStyle");
        AutomationProperties.SetLiveSetting(progressWords, AutomationLiveSetting.Polite);
        var sizedFor = Ui.Text("", "HazeCaptionStyle");
        var cancelSend = Ui.Label(T.SendWaitingCancel, (_, _) =>
        {
            if (sendId is { } id)
                sending.Cancel(id);
            sendId = null;
            redraw();
        });
        string? drawnFoot = null;

        void Draw()
        {
            novel = Novel();
            DrawTargets();
            var target = Target();
            var running = Send();
            chaptersValue.Text = ChaptersWords();
            chevron.Glyph = picking ? "" : "";
            AutomationProperties.SetName(chaptersButton, $"{T.SendChapters}, {chaptersValue.Text}, {(picking ? T.SendChaptersOpen : T.SendChaptersClosed)}");
            chaptersButton.IsEnabled = running?.IsRunning != true;
            var size = Size();
            sizeValue.Text = size is { } bytes ? T.SendSizeAbout(Wording.Bytes(bytes)) : waited ? T.SendSizeWorking : "";
            notes.Children.Clear();
            if (size is { } total && target is { Split: true } && !novel && total > PartsOver)
                notes.Children.Add(Ui.Text(T.SendInParts(Wording.Bytes(PartsOver)), "HazeCaptionStyle"));
            notes.Children.Add(Ui.Text(T.SendNoPlace, "HazeCaptionStyle"));

            // The buttons: Prepare; Close while it runs; Share, Save files and Done when ready; Try again on failure.
            var print = Drawn.Fingerprint((running?.Phase, running?.Files.Length, running?.Line, target?.Id, ids.Count));
            var footChanged = print != drawnFoot;
            drawnFoot = print;
            switch (running?.Phase)
            {
                case null:
                    dialog.PrimaryButtonText = T.SendPrepare;
                    dialog.PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle");
                    dialog.IsPrimaryButtonEnabled = target is not null && ids.Count > 0 && SendState.Format(target, novel) is not null;
                    dialog.SecondaryButtonText = "";
                    dialog.CloseButtonText = T.ActionCancel;
                    dialog.DefaultButton = ContentDialogButton.Primary;
                    progressPart.Children.Clear();
                    break;
                case SendPhase.Preparing:
                case SendPhase.Waiting:
                    dialog.PrimaryButtonText = "";
                    dialog.SecondaryButtonText = "";
                    dialog.CloseButtonText = T.ActionClose;
                    dialog.DefaultButton = ContentDialogButton.Close;
                    if (footChanged || progressPart.Children.Count == 0)
                    {
                        progressPart.Children.Clear();
                        if (running.Phase == SendPhase.Waiting)
                        {
                            progressWords.Text = running.Line;
                            progressPart.Children.Add(progressWords);
                        }
                        else
                        {
                            progressPart.Children.Add(bar);
                            progressPart.Children.Add(progressWords);
                            if (target is not null)
                            {
                                sizedFor.Text = T.SendSizedFor(target.Name);
                                progressPart.Children.Add(sizedFor);
                            }
                        }
                        progressPart.Children.Add(cancelSend);
                    }
                    if (running.Phase == SendPhase.Preparing)
                    {
                        var percent = (int)Math.Round((running.Fraction ?? 0) * 100);
                        bar.IsIndeterminate = running.Fraction is null;
                        bar.Value = percent;
                        var all = Math.Max(running.Total, ids.Count);
                        progressWords.Text = T.SendPreparing(Math.Min(running.Done + 1, all), all);
                        AutomationProperties.SetName(bar, T.SendProgressSpoken(percent));
                    }
                    break;
                case SendPhase.Ready:
                    dialog.PrimaryButtonText = T.ActionShare;
                    dialog.PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle");
                    dialog.IsPrimaryButtonEnabled = true;
                    dialog.SecondaryButtonText = T.SendSaveFilesWindows;
                    dialog.CloseButtonText = T.ActionDone;
                    dialog.DefaultButton = ContentDialogButton.Primary;
                    progressPart.Children.Clear();
                    progressPart.Children.Add(Ui.Text(T.SendReady(running.Files.Length), "BodyTextBlockStyle"));
                    break;
                case SendPhase.Failed:
                    dialog.PrimaryButtonText = T.ActionTryAgain;
                    dialog.PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle");
                    dialog.IsPrimaryButtonEnabled = target is not null;
                    dialog.SecondaryButtonText = "";
                    dialog.CloseButtonText = T.ActionCancel;
                    dialog.DefaultButton = ContentDialogButton.Primary;
                    progressPart.Children.Clear();
                    var failed = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
                    failed.Children.Add(States.ErrorGlyph());
                    failed.Children.Add(Ui.Text(running.Line, "BodyTextBlockStyle"));
                    progressPart.Children.Add(failed);
                    break;
            }
        }

        void Start()
        {
            if (Target() is not { } target)
                return;
            if (sendId is { } old)
                sending.Dismiss(old);
            sendId = sending.Begin(work, Ordered(), target, novel);
            Showing = sendId;
            Draw();
        }

        dialog.PrimaryButtonClick += async (_, args) =>
        {
            var running = Send();
            if (running?.Phase == SendPhase.Ready)
            {
                // The share sheet comes up over the window; the dialog closes once something took the files.
                args.Cancel = true;
                if (await SendFiles.Share(ctx, running, Title()))
                    dialog.Hide();
                return;
            }
            args.Cancel = true;
            Start();
        };
        dialog.SecondaryButtonClick += async (_, args) =>
        {
            if (Send() is not { Phase: SendPhase.Ready } running)
                return;
            args.Cancel = true;
            if (await SendFiles.Save(ctx, running))
                dialog.Hide();
        };

        void Changed(ReadingPart part)
        {
            if (part is ReadingPart.Send or ReadingPart.Estimate or ReadingPart.Chapters or ReadingPart.Downloads or ReadingPart.Series)
                Draw();
        }
        reading.Changed += Changed;
        sending.LoadTargets();
        reading.LoadSeriesInfo(work);
        if (!reading.Chapters.ContainsKey(work))
            reading.LoadChapters(work);
        if (reading.Downloads is null)
            reading.LoadDownloads();
        Showing = sendId;
        redraw = Draw;
        refill = FillPicker;
        Draw();
        Picked();
        await Dialogs.Show(dialog);
        reading.Changed -= Changed;
        estimateClock.Stop();
        workingClock.Stop();
        Showing = null;
        // A send that ended without being shared or saved is let go; one still running keeps its row.
        if (Send() is { IsRunning: false, Phase: SendPhase.Failed } ended)
            sending.Dismiss(ended.Id);
        reading.EstimateDownload(work, new DownloadPick.Chapters([]));
    }

    static TextBlock Heading(string text)
    {
        var heading = Ui.Heading(text);
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
        return heading;
    }

    static RadioButton TargetRadio(string name, string line, string tag)
    {
        var radio = new RadioButton { Content = SeriesWords.Pair(name, line), Tag = tag };
        AutomationProperties.SetName(radio, line.Length > 0 ? $"{name}, {line}" : name);
        return radio;
    }
}

/// <summary>
/// The status rows a send keeps on its series page: the progress while it is made, the waiting
/// line with Cancel, the files to share or save, or why it failed with Try again. Downloads keeps
/// every send's rows, each naming its series.
/// </summary>
sealed partial class SendStatusRows : StackPanel
{
    readonly Ctx ctx;
    readonly Func<string?> work;
    readonly Func<SeriesView?> series;
    string? drawn;
    readonly Dictionary<int, ProgressBar> bars = [];

    public SendStatusRows(Ctx ctx, Func<string?> work, Func<SeriesView?> series)
    {
        this.ctx = ctx;
        this.work = work;
        this.series = series;
        Spacing = 8;
        Margin = new Thickness(0, 8, 0, 0);
        Draw();
    }

    /// <summary>Every send, whatever its series, as Downloads shows them.</summary>
    public SendStatusRows(Ctx ctx) : this(ctx, () => null, () => null)
    {
    }

    /// <summary>Whether the rows show every send rather than one series' own.</summary>
    bool All => work() is null;

    /// <summary>The send's series: the page's own, else what the engine said about it.</summary>
    SeriesView? SeriesOf(EReaderSend send) => (All ? null : series()) ?? ctx.Known.Reading.Series.GetValueOrDefault(send.Work);

    /// <summary>The send's series title, from its details or its Downloads row.</summary>
    string TitleOf(EReaderSend send) =>
        SeriesOf(send)?.Titles.Display is { Length: > 0 } known
            ? known
            : ctx.Known.Reading.Downloads?.Series.FirstOrDefault(s => s.Work == send.Work)?.Title ?? "";

    public void Draw()
    {
        var sends = ctx.Known.Reading.Sending.Sends.Where(s => (All || s.Work == work()) && s.Id != SendDialog.Showing).ToList();
        var print = Drawn.Fingerprint(sends.Select(s => (s.Id, s.Phase, s.Done, s.Total, s.Line, s.Files.Length, s.Target, All ? TitleOf(s) : "")).ToList());
        if (print == drawn)
        {
            // Only the progress moved: the bars take it in place.
            foreach (var s in sends)
                if (bars.TryGetValue(s.Id, out var bar) && s.Fraction is { } f)
                    bar.Value = f * 100;
            return;
        }
        drawn = print;
        var focus = Ui.MarkFocus(this);
        Children.Clear();
        bars.Clear();
        foreach (var send in sends)
            Children.Add(Row(send));
        Visibility = Children.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        Ui.RestoreFocus(this, focus);
    }

    UIElement Row(EReaderSend send)
    {
        var sending = ctx.Known.Reading.Sending;
        var device = send.Target.Length > 0 ? SeriesWords.Device(ctx, send.Target) : "";
        var row = new Grid { ColumnSpacing = 12, Padding = new Thickness(12, 8, 8, 8), CornerRadius = new CornerRadius(8), Background = Ui.Res<Brush>("RaisedBrush"), MaxWidth = 560, HorizontalAlignment = HorizontalAlignment.Left };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var words = new StackPanel { Spacing = 4, VerticalAlignment = VerticalAlignment.Center };
        words.Children.Add(Ui.Text(device.Length == 0 ? T.SendTitle : T.SendBanner(device), "BodyStrongTextBlockStyle", wrap: false));
        if (All && TitleOf(send) is { Length: > 0 } title)
            words.Children.Add(Ui.Text(title, "HazeCaptionStyle", wrap: false));
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, VerticalAlignment = VerticalAlignment.Center };
        Grid.SetColumn(buttons, 1);
        switch (send.Phase)
        {
            case SendPhase.Preparing:
                var percent = (int)Math.Round((send.Fraction ?? 0) * 100);
                var bar = new ProgressBar { Name = $"bar{send.Id}", Minimum = 0, Maximum = 100, Value = percent, IsIndeterminate = send.Fraction is null, MinWidth = 220 };
                AutomationProperties.SetName(bar, T.SendProgressSpoken(percent));
                bars[send.Id] = bar;
                words.Children.Add(bar);
                var all = Math.Max(send.Total, send.Chapters.Length);
                words.Children.Add(Ui.Text(T.SendPreparing(Math.Min(send.Done + 1, Math.Max(all, 1)), Math.Max(all, 1)), "HazeCaptionStyle", wrap: false));
                buttons.Children.Add(Ui.Icon("", T.SendWaitingCancel, (_, _) => Cancel(send)));
                break;
            case SendPhase.Waiting:
                words.Children.Add(Ui.Text(send.Line, "HazeCaptionStyle"));
                buttons.Children.Add(Ui.Label(T.SendWaitingCancel, (_, _) => Cancel(send)));
                break;
            case SendPhase.Ready:
                words.Children.Add(Ui.Text(T.SendReady(send.Files.Length), "HazeCaptionStyle", wrap: false));
                buttons.Children.Add(Ui.Label(T.ActionShare, async (_, _) =>
                {
                    if (await SendFiles.Share(ctx, send, TitleOf(send)))
                        Draw();
                }));
                buttons.Children.Add(Ui.Label(T.SendSaveFilesWindows, async (_, _) =>
                {
                    if (await SendFiles.Save(ctx, send))
                        Draw();
                }));
                buttons.Children.Add(Ui.Icon("", T.ActionClose, (_, _) =>
                {
                    sending.Dismiss(send.Id);
                    Draw();
                }));
                break;
            case SendPhase.Failed:
                var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
                line.Children.Add(States.ErrorGlyph(14));
                line.Children.Add(Ui.Text(send.Line, "HazeCaptionStyle"));
                words.Children.Add(line);
                buttons.Children.Add(Ui.Label(T.ActionTryAgain, (_, _) => Retry(send)));
                buttons.Children.Add(Ui.Icon("", T.ActionClose, (_, _) =>
                {
                    sending.Dismiss(send.Id);
                    Draw();
                }));
                break;
        }
        row.Children.Add(words);
        row.Children.Add(buttons);
        return row;
    }

    void Cancel(EReaderSend send)
    {
        ctx.Known.Reading.Sending.Cancel(send.Id);
        Draw();
    }

    void Retry(EReaderSend send)
    {
        var sending = ctx.Known.Reading.Sending;
        if (sending.Targets.FirstOrDefault(t => t.Id == send.Target) is not { } target)
            return;
        var novel = SeriesOf(send)?.Medium == Medium.Novel;
        sending.Dismiss(send.Id);
        sending.Begin(send.Work, send.Chapters, target, novel);
        Draw();
    }
}

/// <summary>
/// The files a send made, to the e-reader: shared through the Windows share sheet on the app's
/// window, or saved where the person picks. Either way the engine then keeps that the chapters
/// were sent, and the series page asks where you stopped.
/// </summary>
static class SendFiles
{
    /// <summary>The share sheet with the files; true once an app took them.</summary>
    public static async Task<bool> Share(Ctx ctx, EReaderSend send, string title)
    {
        try
        {
            var files = new List<global::Windows.Storage.IStorageItem>();
            foreach (var file in send.Files)
                files.Add(await global::Windows.Storage.StorageFile.GetFileFromPathAsync(file.Path));
            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(ctx.Window);
            var manager = global::Windows.ApplicationModel.DataTransfer.DataTransferManagerInterop.GetForWindow(hwnd);
            var taken = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
            global::Windows.Foundation.TypedEventHandler<global::Windows.ApplicationModel.DataTransfer.DataTransferManager, global::Windows.ApplicationModel.DataTransfer.DataRequestedEventArgs>? handler = null;
            handler = (_, e) =>
            {
                manager.DataRequested -= handler;
                var data = e.Request.Data;
                data.Properties.Title = title.Length > 0 ? title : T.SendTitle;
                data.SetStorageItems(files);
                data.ShareCompleted += (_, _) => taken.TrySetResult(true);
                data.ShareCanceled += (_, _) => taken.TrySetResult(false);
            };
            manager.DataRequested += handler;
            global::Windows.ApplicationModel.DataTransfer.DataTransferManagerInterop.ShowShareUIForWindow(hwnd);
            if (!await taken.Task)
                return false;
            ctx.Known.Reading.Sending.Shared(send.Id);
            return true;
        }
        catch (Exception)
        {
            // No share sheet here: the files are saved instead.
            return await Save(ctx, send);
        }
    }

    /// <summary>
    /// One file goes where the save picker says; several go into the folder picked, each keeping
    /// its name, never over a file already there. True once they are all saved.
    /// </summary>
    public static async Task<bool> Save(Ctx ctx, EReaderSend send)
    {
        if (send.Files.Length == 0)
            return false;
        try
        {
            if (send.Files.Length == 1)
            {
                var only = send.Files[0];
                var picker = new global::Windows.Storage.Pickers.FileSavePicker
                {
                    SuggestedStartLocation = global::Windows.Storage.Pickers.PickerLocationId.DocumentsLibrary,
                    SuggestedFileName = Path.GetFileNameWithoutExtension(only.Name),
                };
                var extension = Path.GetExtension(only.Name);
                picker.FileTypeChoices.Add(extension.TrimStart('.').ToUpperInvariant(), [extension.Length > 0 ? extension : ".epub"]);
                WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(ctx.Window));
                if (await picker.PickSaveFileAsync() is not { } target)
                    return false;
                var source = await global::Windows.Storage.StorageFile.GetFileFromPathAsync(only.Path);
                await source.CopyAndReplaceAsync(target);
            }
            else
            {
                if (await ctx.Window.PickFolder() is not { Length: > 0 } folder)
                    return false;
                foreach (var file in send.Files)
                    File.Copy(file.Path, Free(Path.Combine(folder, file.Name)));
            }
            ctx.Known.Reading.Sending.Shared(send.Id);
            ctx.Window.Say(T.SendSavedWindows(send.Files.Length));
            return true;
        }
        catch (Exception)
        {
            ctx.Window.Say(T.SendSaveFailedWindows);
            return false;
        }
    }

    /// <summary>The path, or "name (2).ext" and on when a file is already there.</summary>
    static string Free(string path)
    {
        if (!File.Exists(path))
            return path;
        var folder = Path.GetDirectoryName(path) ?? "";
        var name = Path.GetFileNameWithoutExtension(path);
        var extension = Path.GetExtension(path);
        for (var n = 2; ; n++)
        {
            var next = Path.Combine(folder, $"{name} ({n}){extension}");
            if (!File.Exists(next))
                return next;
        }
    }
}

/// <summary>
/// Where did you stop: after chapters went to an e-reader, which chapter you reached there and
/// whether you finished it. Finished counts it read; stopped inside it counts the one before.
/// Keep leaves the place as it is.
/// </summary>
static class StoppedDialog
{
    public static async Task Show(Ctx ctx, XamlRoot root, SeriesView series, SentChapters sent)
    {
        var reading = ctx.Known.Reading;
        var work = series.Work;
        var all = reading.Chapters.GetValueOrDefault(work)?.All.ToList() ?? [];
        var rows = all.Where(c => sent.Chapters.Contains(c.Id)).ToList();
        if (rows.Count == 0)
        {
            reading.LoadChapters(work);
            return;
        }
        string Label(ChapterRow r) => r.Number is { Length: > 0 } n ? n : r.Label;
        var device = SeriesWords.Device(ctx, sent.Target);
        var date = SeriesWords.Date(sent.SentAt);
        var body = new StackPanel { Spacing = 10 };
        body.Children.Add(Ui.Text(rows.Count == 1
            ? T.StopSentOne(Label(rows[0]), device, date)
            : T.StopSentRange(Label(rows[0]), Label(rows[^1]), device, date), "BodyTextBlockStyle"));
        var picks = new RadioButtons();
        AutomationProperties.SetName(picks, T.StopTitle);
        foreach (var row in rows)
        {
            var name = row.Title is { Length: > 0 } title ? $"{row.Label} · {title}" : row.Label;
            var radio = new RadioButton { Content = name, Tag = row.Id };
            picks.Items.Add(radio);
        }
        picks.SelectedIndex = rows.Count - 1;
        body.Children.Add(picks);
        var finished = new ToggleSwitch { IsOn = true };
        var finishedCard = new Grid { ColumnSpacing = 12 };
        finishedCard.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        finishedCard.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var finishedWords = Ui.Text("", "BodyTextBlockStyle");
        finishedWords.VerticalAlignment = VerticalAlignment.Center;
        finishedCard.Children.Add(finishedWords);
        Grid.SetColumn(finished, 1);
        finishedCard.Children.Add(finished);
        body.Children.Add(finishedCard);
        ChapterRow? At() => picks.SelectedIndex >= 0 && picks.SelectedIndex < rows.Count ? rows[picks.SelectedIndex] : rows[^1];
        void Words()
        {
            var at = At();
            finishedWords.Text = at is null ? "" : T.StopFinished(Label(at));
            AutomationProperties.SetName(finished, finishedWords.Text);
        }
        picks.SelectionChanged += (_, _) => Words();
        Words();

        var keep = series.Action is SeriesAction.Continue go ? T.StopKeep(go.Label, go.Page) : T.StopKeepPlain;
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.StopTitle,
            Content = SeriesWords.Scroll(body, 440),
            PrimaryButtonText = T.StopSet,
            PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle"),
            SecondaryButtonText = keep,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
        };
        switch (await Dialogs.Show(dialog))
        {
            case ContentDialogResult.Primary when At() is { } at:
                if (finished.IsOn)
                {
                    reading.Sending.StoppedAt(work, at.Id);
                }
                else
                {
                    var index = rows.FindIndex(r => r.Id == at.Id);
                    reading.Sending.StoppedAt(work, index > 0 ? rows[index - 1].Id : null);
                }
                reading.LoadSeries(work);
                break;
            case ContentDialogResult.Secondary:
                reading.Sending.StoppedAt(work, null);
                reading.LoadSeries(work);
                break;
        }
    }
}
