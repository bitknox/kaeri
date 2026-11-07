using SettingValue = Kaeri.Engine.Core.SettingValue;
using Medium = Kaeri.Engine.Core.Medium;
using Microsoft.UI.Xaml.Automation.Peers;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using CommunityToolkit.WinUI.Controls;
using K = Kaeri.Engine.Model.SettingKey;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Reading's settings: the Reading pane (reader defaults per format, when a chapter counts as
/// read, which series settings follow you, and the E-readers group with Send to e-reader and the
/// OPDS feed), the Reading group in Downloads, New chapters in Notifications, and What Kaeri is
/// for in General. Groups for a medium that is off do not show.
/// </summary>
public sealed partial class SettingsPage
{
    /// <summary>The OPDS feed's pane, opened from the E-readers group in place of the Reading pane.</summary>
    bool feedOpen;
    /// <summary>The port as typed, kept while the box is being edited.</summary>
    string? portTyped;
    /// <summary>The address or password last copied, which its button says Done for a moment.</summary>
    string? copied;

    /// <summary>Heard from each visit's start: the medium, the reader defaults, the targets and the feed.</summary>
    void HearReading()
    {
        ctx.Known.Medium.Changed += MediumChanged;
        ctx.Known.Reading.Changed += ReadingChanged;
        ShowReadingPane();
    }

    void LeaveReading()
    {
        ctx.Known.Medium.Changed -= MediumChanged;
        ctx.Known.Reading.Changed -= ReadingChanged;
    }

    void MediumChanged() => DispatcherQueue.TryEnqueue(() =>
    {
        ShowReadingPane();
        Show();
    });

    void ReadingChanged(ReadingPart part)
    {
        if (part is ReadingPart.ReaderDefaults or ReadingPart.Feed or ReadingPart.Targets or ReadingPart.Downloads)
            DispatcherQueue.TryEnqueue(() =>
            {
                if (pane.Name is "reading" or "general")
                    Show();
            });
    }

    /// <summary>The Reading pane is listed only while Kaeri reads on this PC.</summary>
    void ShowReadingPane()
    {
        foreach (var item in Panes.Items.OfType<ListViewItem>())
        {
            if (item.Tag is Pane { Name: "reading" })
                item.Visibility = ctx.Known.Medium.Reads ? Visibility.Visible : Visibility.Collapsed;
        }
        if (pane.Name == "reading" && !ctx.Known.Medium.Reads)
            ShowPane("general");
    }

    /// <summary>Whether a group shows for the medium in force: episode groups while watching, chapter groups while reading.</summary>
    bool GroupShown(string group) => group switch
    {
        "ahead" or "new-episodes" => ctx.Known.Medium.Watches,
        "reading" or "new-chapters" => ctx.Known.Medium.Reads,
        _ => true,
    };

    /// <summary>What decides the Reading pane's cards: the feed pane or the groups, and what each says.</summary>
    string ReadingShape()
    {
        var sending = ctx.Known.Reading.Sending;
        if (!feedOpen)
            return Drawn.Fingerprint((
                "groups",
                ctx.Known.Settings is null,
                ctx.Known.Reading.ReaderDefaults.Values.Select(v => v.Rows.Select(r => (r.Key, Drawn.Fingerprint(r.Value))).ToList()).ToList(),
                sending.Targets.Select(t => t.Id).ToList(),
                sending.DefaultTarget?.Id,
                sending.Feed?.On));
        var feed = sending.Feed;
        // The password's hash, not the password: the shape is a string the page keeps.
        return Drawn.Fingerprint((
            "feed",
            feed?.On, feed?.Addresses.FirstOrDefault(), feed?.Port, feed?.PasswordSet,
            feed?.Password?.GetHashCode(), feed?.Pause,
            feed?.Shares.Select(s => (s.Shelf, s.On, s.Count)).ToList(),
            feed?.Clients.Select(c => (c.Name, c.Address, c.Last, c.Files)).ToList(),
            copied?.GetHashCode()));
    }

    void ShowReading()
    {
        var reading = ctx.Known.Reading;
        reading.LoadReaderDefaults();
        reading.Sending.LoadTargets();
        reading.Sending.LoadFeed();
#if KAERI_TESTING
        if (ctx.Options.Feed is not null && !feedStaged)
        {
            feedStaged = true;
            feedOpen = true;
            reading.Sending.StageFeed(StagedFeed(ctx.Options.Feed));
        }
#endif
        if (feedOpen)
            ShowFeed();
        else
            ReadingGroups();
        State.Done();
    }

    // The Reading pane

    void ReadingGroups()
    {
        var reading = ctx.Known.Reading;
        Cards.Children.Add(Heading(T.SettingsReaderDefaults));
        Cards.Children.Add(Note(T.SettingsReaderDefaultsFooter));

        // Manga: one box of mode and direction.
        var manga = new (string Id, string Label)[] { ("rtl", T.SettingsReaderRtl), ("ltr", T.SettingsReaderLtr), ("strip", T.SettingsReaderStrip) };
        var mangaBox = new ComboBox { Name = "ReaderDefaultManga", MinWidth = 200, ItemsSource = manga.Select(m => m.Label).ToList() };
        AutomationProperties.SetName(mangaBox, T.SettingsReaderManga);
        var mangaNow = reading.ReaderDefaults.TryGetValue(Medium.Manga, out var mangaView)
            ? Choice(mangaView, ReaderSettingKey.Mode) == "strip" ? "strip" : Choice(mangaView, ReaderSettingKey.Direction) == "ltr" ? "ltr" : "rtl"
            : null;
        mangaBox.SelectedIndex = mangaNow is null ? -1 : Array.FindIndex(manga, m => m.Id == mangaNow);
        mangaBox.IsEnabled = mangaNow is not null;
        mangaBox.SelectionChanged += (_, _) =>
        {
            if (quiet || mangaBox.SelectedIndex < 0)
                return;
            var id = manga[mangaBox.SelectedIndex].Id;
            if (id == "strip")
            {
                reading.SetReaderDefault(Medium.Manga, ReaderSettingKey.Mode, new ReaderValue.Choice("strip"));
                return;
            }
            reading.SetReaderDefault(Medium.Manga, ReaderSettingKey.Mode, new ReaderValue.Choice("paged"));
            reading.SetReaderDefault(Medium.Manga, ReaderSettingKey.Direction, new ReaderValue.Choice(id));
        };
        Cards.Children.Add(CardFrame(T.SettingsReaderManga, null, mangaBox));

        if (ctx.Known.Setting(K.WebtoonMode) is { } webtoons)
            Cards.Children.Add(Card(webtoons));

        // Novels: pages or one long scroll, and whether lines are justified.
        var novels = new (string Id, string Label)[] { ("paged", T.SettingsReaderPages), ("scrolled", T.SettingsReaderScrolled) };
        var novelBox = new ComboBox { Name = "ReaderDefaultNovel", MinWidth = 200, ItemsSource = novels.Select(n => n.Label).ToList() };
        AutomationProperties.SetName(novelBox, T.SettingsReaderNovels);
        var hasNovel = reading.ReaderDefaults.TryGetValue(Medium.Novel, out var novelView);
        novelBox.SelectedIndex = !hasNovel ? -1 : Choice(novelView!, ReaderSettingKey.Mode) == "scrolled" ? 1 : 0;
        novelBox.IsEnabled = hasNovel;
        novelBox.SelectionChanged += (_, _) =>
        {
            if (!quiet && novelBox.SelectedIndex >= 0)
                reading.SetReaderDefault(Medium.Novel, ReaderSettingKey.Mode, new ReaderValue.Choice(novels[novelBox.SelectedIndex].Id));
        };
        Cards.Children.Add(CardFrame(T.SettingsReaderNovels, null, novelBox));

        // On for computers by the engine's default; a phone's narrow column leaves it off.
        var justifyOn = hasNovel && novelView!.Rows.FirstOrDefault(r => r.Key == ReaderSettingKey.Justify)?.Value is ReaderValue.Toggle { On: true }
            || !hasNovel;
        var justify = new ToggleSwitch { Name = "ReaderDefaultJustify", IsOn = justifyOn, OnContent = "", OffContent = "", MinWidth = 0, IsEnabled = hasNovel };
        AutomationProperties.SetName(justify, T.WindowsReaderJustify);
        justify.Toggled += (_, _) =>
        {
            if (!quiet)
                reading.SetReaderDefault(Medium.Novel, ReaderSettingKey.Justify, new ReaderValue.Toggle(justify.IsOn));
        };
        Cards.Children.Add(CardFrame(T.WindowsReaderJustify, T.SettingsReaderNovels, justify));

        if (ctx.Known.Setting(K.CountsAsRead) is { } counts)
            Cards.Children.Add(Card(counts));

        // What follows you to your other devices.
        var follow = new[] { K.SeriesChoicesFollow, K.LookFollows }.Select(k => ctx.Known.Setting(k)).OfType<SettingRow>().ToList();
        if (follow.Count > 0)
        {
            Cards.Children.Add(Heading(T.SettingsFollowGroup));
            foreach (var row in follow)
            {
                var made = Card(row);
                if (row.Key == K.SeriesChoicesFollow && made is SettingsCard card)
                    card.Header = T.SettingsModeFollows;
                Cards.Children.Add(made);
            }
            Cards.Children.Add(Note(T.SettingsFollowFooter(SyncWords.Kind)));
        }

        // E-readers: where Send to e-reader sends first, and the feed.
        var sending = reading.Sending;
        Cards.Children.Add(Heading(T.EreadersGroup));
        var targets = sending.Targets;
        var targetBox = new ComboBox { Name = "SendTarget", MinWidth = 200, ItemsSource = targets.Select(t => t.Name).ToList() };
        AutomationProperties.SetName(targetBox, T.EreadersSendRow);
        targetBox.SelectedIndex = sending.DefaultTarget is { } chosen ? Array.FindIndex(targets, t => t.Id == chosen.Id) : -1;
        targetBox.IsEnabled = targets.Length > 0;
        targetBox.SelectionChanged += (_, _) =>
        {
            if (!quiet && targetBox.SelectedIndex >= 0)
                sending.Choose(targets[targetBox.SelectedIndex].Id);
        };
        Cards.Children.Add(CardFrame(T.EreadersSendRow, null, targetBox));

        var feedCard = new SettingsCard
        {
            Name = "FeedOpen",
            Header = T.EreadersFeedRow,
            Description = T.OpdsShareNote,
            Content = Ui.Text(sending.Feed?.On == true ? T.ValueOn : T.ValueOff, "HazeBodyStyle", wrap: false),
            IsClickEnabled = true,
            ActionIcon = new FontIcon { Glyph = "" },
        };
        AutomationProperties.SetName(feedCard, T.EreadersFeedRow);
        feedCard.Click += (_, _) => OpenFeed(true);
        Cards.Children.Add(feedCard);
    }

    static string? Choice(ReaderSettingsView view, ReaderSettingKey key) =>
        view.Rows.FirstOrDefault(r => r.Key == key)?.Value is ReaderValue.Choice choice ? choice.Name : null;

    void OpenFeed(bool open)
    {
        feedOpen = open;
        shownPane = null;
        Show();
        // The pane's first control takes the focus, as a new page's does.
        DispatcherQueue.TryEnqueue(() =>
            Cards.Children.OfType<Control>().FirstOrDefault(c => c.Name == (open ? "FeedBack" : "FeedOpen"))?.Focus(FocusState.Programmatic));
    }

    // The OPDS feed

    void ShowFeed()
    {
        var sending = ctx.Known.Reading.Sending;
        var feed = sending.Feed;
        var back = new HyperlinkButton { Name = "FeedBack", Padding = new Thickness(0, 4, 8, 4) };
        var backLine = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        backLine.Children.Add(new FontIcon { Glyph = "", FontSize = 12 });
        backLine.Children.Add(new TextBlock { Text = T.SettingsGroupReading });
        back.Content = backLine;
        AutomationProperties.SetName(back, $"{T.ActionBack}, {T.SettingsGroupReading}");
        back.Click += (_, _) => OpenFeed(false);
        Cards.Children.Add(back);
        Cards.Children.Add(Heading(T.EreadersFeedRow));

        // On and off. The first time, Windows' firewall question is explained before it comes.
        var toggle = new ToggleSwitch { Name = "FeedOn", IsOn = feed?.On == true, OnContent = "", OffContent = "", MinWidth = 0, IsEnabled = feed is not null };
        AutomationProperties.SetName(toggle, T.SettingsOpdsFeed);
        toggle.Toggled += async (_, _) =>
        {
            if (quiet)
                return;
            if (!toggle.IsOn)
            {
                sending.SetFeed(false);
                return;
            }
            if (!await FeedFirewall.Ask(XamlRoot))
            {
                quiet = true;
                toggle.IsOn = false;
                quiet = false;
                return;
            }
            // The feed never starts without a password, so the first time makes one.
            sending.SetFeed(true, sending.Feed?.PasswordSet == true ? null : SendState.NewPassword());
        };
        Cards.Children.Add(CardFrame(T.SettingsOpdsFeed, T.OpdsShareNote, toggle));

        // The address, or why there is none.
        var address = feed?.On == true ? feed.Addresses.FirstOrDefault() : null;
        var addressRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var addressText = new TextBlock
        {
            Name = "FeedAddress",
            Text = AddressWords(feed),
            IsTextSelectionEnabled = address is not null,
            FontFamily = address is not null ? new FontFamily("Cascadia Mono, Consolas") : FontFamily,
            Foreground = Ui.Res<Brush>(address is not null ? "TextFillColorPrimaryBrush" : "TextFillColorSecondaryBrush"),
            TextWrapping = TextWrapping.Wrap,
            MaxWidth = 320,
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetLiveSetting(addressText, AutomationLiveSetting.Polite);
        addressRow.Children.Add(addressText);
        if (address is not null)
            addressRow.Children.Add(CopyButton(address, "FeedAddressCopy", T.OpdsAddress));
        Cards.Children.Add(CardFrame(T.OpdsAddress, null, addressRow));

        // The password, in fours to read off; shown and copied, never written anywhere else.
        var passwordRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        if (feed?.Password is { Length: > 0 } password)
        {
            passwordRow.Children.Add(new TextBlock
            {
                Name = "FeedPassword",
                Text = Spaced(password),
                IsTextSelectionEnabled = true,
                FontFamily = new FontFamily("Cascadia Mono, Consolas"),
                VerticalAlignment = VerticalAlignment.Center,
            });
            passwordRow.Children.Add(CopyButton(password, "FeedPasswordCopy", T.OpdsPassword));
        }
        if (feed?.PasswordSet == true)
        {
            var renew = new Button { Name = "FeedNewPassword", Content = T.OpdsNewPassword };
            renew.Click += async (_, _) =>
            {
                if (await Ui.Confirm(XamlRoot, T.OpdsNewPasswordTitle, T.OpdsNewPasswordWarning, T.OpdsNewPassword))
                    sending.SetFeed(sending.Feed?.On == true, SendState.NewPassword());
            };
            passwordRow.Children.Add(renew);
        }
        Cards.Children.Add(CardFrame(T.OpdsPassword, T.OpdsPasswordNote, passwordRow));

        // The port: empty picks a free one.
        var portBox = new TextBox
        {
            Name = "FeedPort",
            Width = 96,
            PlaceholderText = "8484",
            Text = portTyped ?? ctx.Known.Setting(K.OpdsPort)?.Value ?? "",
            InputScope = new InputScope { Names = { new InputScopeName(InputScopeNameValue.Number) } },
            MaxLength = 5,
        };
        AutomationProperties.SetName(portBox, T.OpdsPort);
        portBox.TextChanged += (_, _) => portTyped = portBox.Text;
        void SavePort()
        {
            var digits = new string(portBox.Text.Where(char.IsAsciiDigit).ToArray());
            var kept = ushort.TryParse(digits, out var port) && port >= 1024 ? port.ToString() : "";
            portTyped = null;
            if (portBox.Text != kept)
                portBox.Text = kept;
            if (kept != (ctx.Known.Setting(K.OpdsPort)?.Value ?? ""))
                ctx.Send(new Command.SetSetting(K.OpdsPort, new SettingValue.Str(kept)));
        }
        portBox.LostFocus += (_, _) => SavePort();
        portBox.KeyDown += (_, e) =>
        {
            if (e.Key == global::Windows.System.VirtualKey.Enter)
            {
                SavePort();
                e.Handled = true;
            }
        };
        Cards.Children.Add(CardFrame(T.OpdsPort, PortNote(feed, portBox.Text), portBox));
        Cards.Children.Add(Note(T.OpdsFooter(SyncWords.Kind)));

        // What the feed offers.
        Cards.Children.Add(Heading(T.OpdsShares));
        foreach (var share in feed?.Shares ?? [])
        {
            var name = share.Shelf switch
            {
                FeedShelf.Downloads => T.OpdsShareDownloads,
                FeedShelf.MangaFolders => T.OpdsShareManga,
                _ => T.OpdsShareNovels,
            };
            var count = share.Shelf == FeedShelf.Downloads ? T.OpdsShareDownloadsCount(share.Count) : T.OpdsShareSeriesCount(share.Count);
            var box = new CheckBox { Name = $"FeedShare{share.Shelf}", IsChecked = share.On, MinWidth = 0 };
            AutomationProperties.SetName(box, name);
            AutomationProperties.SetHelpText(box, count);
            var shelf = share.Shelf;
            box.Checked += (_, _) => { if (!quiet) sending.SetShare(shelf, true); };
            box.Unchecked += (_, _) => { if (!quiet) sending.SetShare(shelf, false); };
            Cards.Children.Add(CardFrame(name, count, box));
        }

        // Who fetched, while it is on.
        if (feed?.On == true)
        {
            Cards.Children.Add(Heading(T.OpdsFetchedBy));
            if (feed.Clients.Length == 0)
                Cards.Children.Add(CardFrame(T.OpdsFetchedNone, null, null));
            foreach (var client in feed.Clients)
            {
                var name = client.Name.Length == 0 ? T.OpdsReaderUnnamed(client.Address) : client.Name;
                var when = Ui.Text(SeriesWords.Ago(client.Last), "HazeBodyStyle", wrap: false);
                Cards.Children.Add(CardFrame(name, T.OpdsFetchedFiles(client.Files), when, ""));
            }
        }
    }

    Button CopyButton(string text, string name, string what)
    {
        var done = copied == text;
        var button = new Button { Name = name, Content = done ? T.ActionDone : T.ActionCopy };
        AutomationProperties.SetName(button, $"{T.ActionCopy}, {what}");
        button.Click += (_, _) =>
        {
            if (!Ui.TryCopy(text))
                return;
            copied = text;
            button.Content = T.ActionDone;
            var clock = DispatcherQueue.CreateTimer();
            clock.Interval = TimeSpan.FromSeconds(2);
            clock.IsRepeating = false;
            clock.Tick += (_, _) =>
            {
                if (copied == text)
                    copied = null;
                button.Content = T.ActionCopy;
            };
            clock.Start();
        };
        return button;
    }

    /// <summary>The password in the groups of four a person reads off to type it: "4821 9930".</summary>
    static string Spaced(string password) =>
        string.Join(" ", Enumerable.Range(0, (password.Length + 3) / 4).Select(i => password.Substring(i * 4, Math.Min(4, password.Length - i * 4))));

    /// <summary>What the address row says: the address, why there is none, or that it is starting.</summary>
    static string AddressWords(FeedView? feed)
    {
        if (feed is not { On: true })
            return T.OpdsAddressOff;
        if (feed.Addresses.FirstOrDefault() is { } address)
            return address;
        return feed.Pause switch
        {
            FeedPause.NoNetwork => T.OpdsPausedNoneWindows,
            FeedPause.VpnOnly => T.OpdsPausedVpnWindows,
            _ => T.OpdsStarting,
        };
    }

    /// <summary>Under the port: that a free one is picked when it is empty, or that the one asked for was busy.</summary>
    static string? PortNote(FeedView? feed, string typed)
    {
        if (feed is { On: true } && ushort.TryParse(typed, out var wanted) && wanted != 0 && feed.Port != wanted)
            return T.OpdsPortMoved(wanted, feed.Port);
        return typed.Length == 0 ? T.OpdsPortNote : null;
    }

#if KAERI_TESTING
    bool feedStaged;

    /// <summary>A shot's feed: on with two readers that fetched, or paused for want of a network.</summary>
    static FeedView StagedFeed(string state)
    {
        var now = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        var shares = new[]
        {
            new FeedShare(FeedShelf.Downloads, true, 42),
            new FeedShare(FeedShelf.MangaFolders, true, 6),
            new FeedShare(FeedShelf.NovelFolders, false, 2),
        };
        return state == "paused"
            ? new FeedView(true, [], 8484, true, 0, "48219930", shares, [], FeedPause.NoNetwork)
            : new FeedView(true, ["http://192.168.1.20:8484/opds"], 8484, true, 1, "48219930", shares,
                [new FeedClient("Reader one", "192.168.1.31", now - 600, 3), new FeedClient("", "192.168.1.44", now - 86_400 * 2, 1)], null);
    }
#endif

    // What Kaeri is for

    /// <summary>General's first card: Watch, Read or Both on this PC, with the turn-off confirmation.</summary>
    SettingsCard KaeriIsForCard()
    {
        var answers = new[] { KaeriIsFor.Watch, KaeriIsFor.Read, KaeriIsFor.Both };
        var box = new ComboBox { Name = "KaeriIsFor", MinWidth = 200, ItemsSource = answers.Select(MediumDialogs.Name).ToList() };
        AutomationProperties.SetName(box, T.SettingsKaeriIsFor);
        box.SelectedIndex = Array.IndexOf(answers, ctx.Known.Medium.Answer);
        refresh.Add(() => box.SelectedIndex = Array.IndexOf(answers, ctx.Known.Medium.Answer));
        box.SelectionChanged += async (_, _) =>
        {
            if (quiet || box.SelectedIndex < 0)
                return;
            var answer = answers[box.SelectedIndex];
            if (answer == ctx.Known.Medium.Answer)
                return;
            if (!await MediumDialogs.Choose(ctx, XamlRoot, answer))
            {
                quiet = true;
                box.SelectedIndex = Array.IndexOf(answers, ctx.Known.Medium.Answer);
                quiet = false;
            }
        };
        return CardFrame(T.SettingsKaeriIsFor, T.SettingsMediumNote, box, "");
    }

    /// <summary>The line under a reading row, where the Mac has one.</summary>
    string? ReadingDetail(SettingRow row) => row.Key switch
    {
        K.WebtoonMode => T.SettingsReaderWebtoonsNote,
        K.CountsAsRead => row.Value switch
        {
            "moving-on" => T.SettingsCountsReadMovingOnNote,
            "marked" => T.SettingsCountsReadMarkedNote,
            _ => T.SettingsCountsReadNote,
        },
        K.LookFollows => T.SettingsLookFollowsNote,
        K.NewChapters => row.Value == "reading" ? T.SettingsNewChaptersNote : null,
        K.DeleteAfterReading => T.SettingsDeleteAfterReadingNote,
        K.ChapterNotifications => T.SettingsChapterNotificationsNote,
        _ => null,
    };
}

/// <summary>
/// The first time the OPDS feed is switched on, a word before Windows asks about the network:
/// allow Kaeri on private networks only. Asked once on this PC, kept as a flag file beside the
/// app's other small choices.
/// </summary>
static class FeedFirewall
{
    static string? path;
    static bool asked;

    public static void Load(string configDir)
    {
        path = Path.Combine(configDir, "windows-feed-firewall-asked");
        asked = SmallFiles.Read(path) is not null;
    }

    /// <summary>True to go on and start the feed: asked before and continued, or Continue now.</summary>
    public static async Task<bool> Ask(XamlRoot root)
    {
        if (asked)
            return true;
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.FeedFirewallTitleWindows,
            Content = new TextBlock { Text = T.FeedFirewallBodyWindows, TextWrapping = TextWrapping.Wrap, MaxWidth = 420 },
            PrimaryButtonText = T.OnboardingContinue,
            PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle"),
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
        };
        if (await Dialogs.Show(dialog) != ContentDialogResult.Primary)
            return false;
        asked = true;
        if (path is not null)
            SmallFiles.Write(path, "");
        return true;
    }
}
