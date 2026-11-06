using Kaeri.App;
using Kaeri.Engine.Core;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Open link or file: a magnet link, an info hash or a torrent file the viewer has. The engine
/// reads its name and guesses a title and an episode; the dialog asks "Is this …, episode N?"
/// with Change (the titles it could be, and a search for others), Play without tracking and
/// Download. Nothing is recorded until the viewer confirms.
/// </summary>
sealed class OpenedDialog
{
    readonly Ctx ctx;
    readonly ContentDialog dialog;
    readonly StackPanel body = new() { Spacing = 12, MinWidth = 560 };
    readonly TextBox input = new() { PlaceholderText = T.OpenedPastePlaceholder, AcceptsReturn = false };
    readonly string? given;
    OpenedLink? opened;
    Entry? chosen;
    string? guessedEpisode;
    NumberBox? episode;
    StackPanel? choices;
    TextBox? search;
    string lastQuery = "";
    bool reading;
    /// <summary>The engine's job reading the link.</summary>
    ulong? readJob;
    readonly TextBlock problem = Ui.Text("", "HazeCaptionStyle");

    public OpenedDialog(Ctx ctx, XamlRoot root, string? given)
    {
        this.ctx = ctx;
        this.given = given;
        AutomationProperties.SetName(input, T.OpenedPasteTitle);
        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.OpenedPasteTitle,
            Content = body,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
        };
        dialog.Resources["ContentDialogMaxWidth"] = 720.0;
        dialog.PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle");
        dialog.PrimaryButtonClick += OnPrimary;
        dialog.SecondaryButtonClick += OnSecondary;
        input.TextChanged += (_, _) =>
        {
            if (opened is null && !reading)
                dialog.IsPrimaryButtonEnabled = input.Text.Trim().Length > 0;
        };
        if (given is { Length: > 0 })
            Read(given);
        else
            Ask();
    }

    public async Task ShowAsync()
    {
        ctx.Batch += Heard;
        try
        {
            await Dialogs.Show(dialog);
        }
        finally
        {
            ctx.Batch -= Heard;
        }
    }

    /// <summary>Step one: paste a link, or choose a torrent file.</summary>
    void Ask()
    {
        opened = null;
        reading = false;
        body.Children.Clear();
        body.Children.Add(input);
        var file = new Button { Content = T.OpenedChooseFile };
        file.Click += async (_, _) => await ChooseFile();
        body.Children.Add(file);
        // Why the last link or file was not read: said here, where the viewer is looking, as the
        // engine's notice is under the dialog.
        problem.Visibility = problem.Text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
        body.Children.Add(problem);
        dialog.Title = T.OpenedPasteTitle;
        dialog.PrimaryButtonText = T.ActionOpen;
        dialog.SecondaryButtonText = "";
        dialog.IsPrimaryButtonEnabled = input.Text.Trim().Length > 0;
    }

    async Task ChooseFile()
    {
        try
        {
            var picker = new Microsoft.Windows.Storage.Pickers.FileOpenPicker(ctx.Window.AppWindow.Id)
            {
                CommitButtonText = T.ActionOpen,
                SuggestedStartLocation = Microsoft.Windows.Storage.Pickers.PickerLocationId.Downloads,
            };
            picker.FileTypeFilter.Add(".torrent");
            if ((await picker.PickSingleFileAsync())?.Path is { Length: > 0 } path)
                Read(path);
        }
        catch (Exception e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"the file picker failed: {e.Message}");
        }
    }

    /// <summary>Hands the engine what was opened: a torrent file's bytes, or the link as typed.</summary>
    void Read(string text)
    {
        reading = true;
        body.Children.Clear();
        body.Children.Add(new ProgressRing { IsActive = true, Width = 32, Height = 32, HorizontalAlignment = HorizontalAlignment.Center });
        dialog.PrimaryButtonText = "";
        dialog.SecondaryButtonText = "";
        problem.Text = "";
        // Kept in the field, so a link that can't be read can be corrected there.
        input.Text = text.Trim();
        var path = text.Trim().Trim('"');
        if (path.EndsWith(".torrent", StringComparison.OrdinalIgnoreCase) && File.Exists(path))
        {
            byte[] bytes;
            try
            {
                bytes = File.ReadAllBytes(path);
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException)
            {
                Unread(T.OpenedUnreadable);
                return;
            }
            readJob = ctx.Send(new Command.Opened(new OpenedCommand.OpenTorrentFile(bytes)));
        }
        else
            readJob = ctx.Send(new Command.Opened(new OpenedCommand.OpenLink(text.Trim())));
    }

    /// <summary>The link or file was not read: back to step one, saying why, to try another.</summary>
    void Unread(string why)
    {
        readJob = null;
        problem.Text = why;
        Ask();
    }

    void OnPrimary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        if (opened is null)
        {
            args.Cancel = true;
            if (input.Text.Trim().Length > 0)
                Read(input.Text);
            return;
        }
        if (!opened.PlaybackOn)
        {
            args.Cancel = true;
            ctx.Send(new Command.SetSetting(SettingKey.Torrents, new SettingValue.Bool(true)));
            opened = opened with { PlaybackOn = true };
            Confirm(opened);
            return;
        }
        if (chosen is null || Episode() is not { } ep)
        {
            args.Cancel = true;
            return;
        }
        ctx.Send(new Command.Opened(new OpenedCommand.Play(opened.Link, chosen.Id, ep, chosen.Title)));
    }

    void OnSecondary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        if (opened is not null)
            ctx.Send(new Command.Opened(new OpenedCommand.PlayUntracked(opened.Link, opened.Name ?? opened.Link)));
    }

    string? Episode()
    {
        if (episode is { } box && !double.IsNaN(box.Value))
            return ((long)box.Value).ToString(System.Globalization.CultureInfo.InvariantCulture);
        return guessedEpisode;
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.LinkOpened l:
                    readJob = null;
                    Confirm(l.V1);
                    break;
                // A link the engine could not read: it says why in an alert and ends the job, with
                // no answer to wait for.
                case Event.Toast { V1.Kind: NoticeKind.Alert } t when reading:
                    Unread(t.V1.Msg?.Text() is { Length: > 0 } why ? why : T.OpenedUnreadable);
                    break;
                case Event.JobDone d when reading && readJob is { } job && d.V1 == job:
                    Unread(T.OpenedUnreadable);
                    break;
                case Event.OpenedChoices c when opened is not null && c.Link == opened.Link && c.Query == lastQuery:
                    ShowChoices(c.Candidates ?? [], c.Failed);
                    break;
            }
        }
    }

    /// <summary>Step two: what the engine guessed, to confirm, change or play without tracking.</summary>
    void Confirm(OpenedLink link)
    {
        opened = link;
        reading = false;
        chosen = link.Guess?.Title;
        guessedEpisode = link.Guess?.Episode;
        body.Children.Clear();
        episode = null;
        choices = null;
        if (link.Name is { Length: > 0 } name)
            body.Children.Add(Ui.Haze(name, wrap: true));
        if (chosen is null)
            body.Children.Add(Ui.Text(T.OpenedNoGuess, "BodyTextBlockStyle"));
        else if (guessedEpisode is { Length: > 0 } ep)
            body.Children.Add(Ui.Text(T.OpenedConfirm(chosen.Title, ep), "BodyStrongTextBlockStyle"));
        else
        {
            body.Children.Add(Ui.Text(T.OpenedConfirmTitle(chosen.Title), "BodyStrongTextBlockStyle"));
            AddEpisodeBox(1);
        }
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var change = new Button { Content = T.OpenedChange };
        change.Click += (_, _) => OpenChange();
        row.Children.Add(change);
        if (chosen is not null && link.PlaybackOn)
        {
            var download = new Button { Content = T.OpenedDownload };
            download.Click += (_, _) =>
            {
                if (Episode() is { } e)
                {
                    ctx.Send(new Command.Opened(new OpenedCommand.Download(link.Link, chosen.Id, e)));
                    dialog.Hide();
                }
            };
            row.Children.Add(download);
        }
        body.Children.Add(row);
        body.Children.Add(Ui.Haze(T.OpenedUntrackedDetail));
        dialog.Title = T.OpenedPasteTitle;
        dialog.PrimaryButtonText = link.PlaybackOn ? T.OpenedPlay : T.OpenedTurnOn;
        dialog.SecondaryButtonText = T.OpenedPlayUntracked;
        dialog.IsPrimaryButtonEnabled = !link.PlaybackOn || chosen is not null;
        if (chosen is null)
            OpenChange();
    }

    void AddEpisodeBox(double value)
    {
        episode = new NumberBox { Header = T.OpenedEpisode, Minimum = 0, Value = value, SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Compact, Width = 160, HorizontalAlignment = HorizontalAlignment.Left };
        AutomationProperties.SetName(episode, T.OpenedEpisode);
        body.Children.Add(episode);
    }

    /// <summary>Change: the titles the link could be, and a search for others.</summary>
    void OpenChange()
    {
        if (opened is null || choices is not null)
            return;
        body.Children.Add(Ui.Text(T.OpenedChangeTitle, "BodyStrongTextBlockStyle"));
        search = new TextBox { PlaceholderText = T.OpenedChangeSearch };
        AutomationProperties.SetName(search, T.OpenedChangeSearch);
        search.KeyDown += (_, e) =>
        {
            if (e.Key == global::Windows.System.VirtualKey.Enter && search.Text.Trim().Length > 0 && opened is { } o)
            {
                lastQuery = search.Text.Trim();
                ctx.Send(new Command.Opened(new OpenedCommand.Find(o.Link, lastQuery)));
            }
        };
        body.Children.Add(search);
        choices = new StackPanel { Spacing = 4 };
        body.Children.Add(new ScrollViewer { Content = choices, MaxHeight = 220, VerticalScrollBarVisibility = ScrollBarVisibility.Auto });
        ShowChoices(opened.Candidates ?? [], null);
    }

    void ShowChoices(Entry[] candidates, Msg? failed)
    {
        if (choices is null)
            return;
        choices.Children.Clear();
        if (candidates.Length == 0 && failed is not null)
            choices.Children.Add(Ui.Haze(T.OpenedSearchFailed(failed.Text())));
        foreach (var entry in candidates)
        {
            var pick = new Button { HorizontalAlignment = HorizontalAlignment.Stretch, HorizontalContentAlignment = HorizontalAlignment.Left };
            var line = new StackPanel();
            line.Children.Add(Ui.Text(entry.Title, wrap: false));
            if (Wording.CoverCaption(entry) is { Length: > 0 } caption)
                line.Children.Add(Ui.Text(caption, "HazeCaptionStyle", wrap: false));
            pick.Content = line;
            var picked = entry;
            pick.Click += (_, _) => Choose(picked);
            choices.Children.Add(pick);
        }
    }

    /// <summary>A title picked under Change: the question asks for its episode.</summary>
    void Choose(Entry entry)
    {
        if (opened is null)
            return;
        chosen = entry;
        guessedEpisode = null;
        var link = opened;
        body.Children.Clear();
        choices = null;
        episode = null;
        body.Children.Add(Ui.Text(T.OpenedConfirmTitle(entry.Title), "BodyStrongTextBlockStyle"));
        AddEpisodeBox(1);
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var change = new Button { Content = T.OpenedChange };
        change.Click += (_, _) => OpenChange();
        row.Children.Add(change);
        body.Children.Add(row);
        body.Children.Add(Ui.Haze(T.OpenedUntrackedDetail));
        dialog.IsPrimaryButtonEnabled = true;
    }
}
