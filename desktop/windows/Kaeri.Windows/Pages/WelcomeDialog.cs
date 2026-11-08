using System.Net;
using System.Net.Sockets;
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
/// First launch, as the spec has it everywhere: Welcome; "Already use Kaeri on another device?",
/// which pairs here and brings that device's setup; What is Kaeri for, three rows with no Skip and
/// nothing chosen, where reading is in this build; the lists; and where shows, books or both come
/// from. Pages the other device's setup answered are skipped. It never mentions torrents. Every
/// step is in Settings afterwards. On Windows the pages are one ContentDialog whose content and
/// buttons change.
/// </summary>
sealed class WelcomeDialog
{
    enum Screen { Welcome, Other, Medium, Lists, Sources }

    readonly Ctx ctx;
    readonly ContentDialog dialog;
    readonly StackPanel body = new() { Spacing = 12 };
    /// <summary>
    /// One size for every page, as the Mac's sheet is, so the dialog does not grow and move between
    /// them; a page taller than it scrolls. The Welcome page has no title, so its box takes the
    /// title's room too.
    /// </summary>
    readonly ScrollViewer frame = new() { Width = 500, VerticalScrollBarVisibility = ScrollBarVisibility.Auto };
    const double PageHeight = 440, TitleRoom = 40;
    readonly TextBox invite = new() { Name = "WelcomeInvite", PlaceholderText = T.StoresInviteLinkHint, AcceptsReturn = false };
    readonly TextBlock problem = Ui.Text("", "HazeCaptionStyle");
    readonly Button join = new() { Name = "WelcomeJoin", Content = T.OnboardingJoin, MinWidth = 72, IsEnabled = false };
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer clock;
    Screen page;
    bool linked;
    /// <summary>The pages another device's setup answered, and the medium page once it was answered here.</summary>
    readonly HashSet<Screen> answered = [];
    /// <summary>What Kaeri is for, as chosen on the medium page or brought from the other device.</summary>
    KaeriIsFor? medium;

    // Pairing on the other-device page.
    bool pairing;
    DateTimeOffset pairStarted;
    string? qrFor;
    /// <summary>What the paired device brought, once the engine said.</summary>
    JoinedSetup? setup;
    readonly HashSet<string> storesOn = [];
    readonly HashSet<string> serversOn = [];

    /// <summary>An invite link to join once the dialog has closed: the join dialog takes it from there.</summary>
    public string? Invite { get; private set; }

    /// <summary>
    /// What opens once this closes: "folder" or "server" when the viewer chose to add their own
    /// shows, "book-folder" or "book-server" for their own books, "pair" when they set this PC up
    /// from another device by the full dialog, "restore" for restoring from a cloud folder.
    /// </summary>
    public string? Then { get; private set; }

    public WelcomeDialog(Ctx ctx, XamlRoot root)
    {
        this.ctx = ctx;
        AutomationProperties.SetName(invite, T.OnboardingInviteHint);
        invite.TextChanged += (_, _) => Check();
        join.Click += (_, _) => JoinNow();
        // Enter in the field joins when it holds an invite; with nothing in it, Enter plays the demo.
        invite.KeyDown += (_, e) =>
        {
            if (e.Key == global::Windows.System.VirtualKey.Enter && Stores.InviteIn(invite.Text) is not null)
            {
                e.Handled = true;
                JoinNow();
            }
        };
        frame.Content = body;
        dialog = new ContentDialog { XamlRoot = root, Content = frame };
        dialog.PrimaryButtonClick += OnPrimary;
        dialog.SecondaryButtonClick += OnSecondary;
        clock = ctx.Window.DispatcherQueue.CreateTimer();
        clock.Interval = TimeSpan.FromMilliseconds(300);
        clock.Tick += (_, _) => RefreshPair();
        // The other device's setup comes as an engine event, heard while the dialog is up.
        ctx.Hear(frame, Heard);
        dialog.Closed += (_, _) =>
        {
            StopPairing();
            ctx.Left(frame);
        };
        Show(Screen.Welcome);
    }

    public Task ShowAsync() => Dialogs.Show(dialog);

    /// <summary>Opens on a later page, for a screenshot: other, medium, anilist (lists) or sources.</summary>
    public void Step(string name) => Show(name switch
    {
        "other" => Screen.Other,
        "medium" when MediumAsked => Screen.Medium,
        "anilist" or "lists" => Screen.Lists,
        "sources" => Screen.Sources,
        _ => Screen.Welcome,
    });

    /// <summary>An invite link opened while this is up: on to the sources page, filled in.</summary>
    public void Fill(string link)
    {
        invite.Text = link;
        linked = true;
        Show(Screen.Sources);
    }

    /// <summary>The medium page shows where reading is in this build and the edition leaves the choice.</summary>
    bool MediumAsked => ctx.Known.Medium.Choosable;

    /// <summary>The pages in order, for "2 of 5".</summary>
    List<Screen> Order()
    {
        var order = new List<Screen> { Screen.Welcome, Screen.Other };
        if (MediumAsked)
            order.Add(Screen.Medium);
        order.Add(Screen.Lists);
        order.Add(Screen.Sources);
        return order;
    }

    /// <summary>The answer the lists and sources pages follow: the one chosen here, else the one in force.</summary>
    KaeriIsFor Answer => medium ?? ctx.Known.Medium.Answer;

    /// <summary>On to the next page that is still to answer, or the app when none is.</summary>
    void Next()
    {
        var order = Order();
        var at = order.IndexOf(page);
        foreach (var next in order.Skip(at + 1))
        {
            if (answered.Contains(next))
                continue;
            Show(next);
            return;
        }
        dialog.Hide();
    }

    void Show(Screen to)
    {
        if (to != Screen.Other)
            StopPairing();
        page = to;
        body.Children.Clear();
        frame.Height = to == Screen.Welcome ? PageHeight + TitleRoom : PageHeight;
        frame.ChangeView(null, 0, null, true);
        dialog.PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle");
        dialog.PrimaryButtonText = "";
        dialog.SecondaryButtonText = "";
        dialog.CloseButtonText = "";
        dialog.IsPrimaryButtonEnabled = true;
        dialog.DefaultButton = ContentDialogButton.Primary;
        if (to != Screen.Welcome)
        {
            var order = Order();
            var position = Ui.Text(T.OnboardingStep(order.IndexOf(to) + 1, order.Count), "HazeCaptionStyle", wrap: false);
            body.Children.Add(position);
        }
        switch (page)
        {
            case Screen.Welcome:
                dialog.Title = null;
                var nagare = new Nagare(5) { HorizontalAlignment = HorizontalAlignment.Left, Margin = new Thickness(0, 4, 0, 4) };
                AutomationProperties.SetAccessibilityView(nagare, AccessibilityView.Raw);
                body.Children.Add(nagare);
                var name = Ui.Text(T.AppName, "TitleTextBlockStyle");
                AutomationProperties.SetHeadingLevel(name, AutomationHeadingLevel.Level1);
                body.Children.Add(name);
                body.Children.Add(Ui.Text(MediumAsked ? T.OnboardingWelcomeLine : T.OnboardingTagline, "BodyTextBlockStyle"));
                if (MediumAsked)
                    body.Children.Add(Ui.Haze(T.OnboardingWelcomeNote));
                dialog.PrimaryButtonText = T.OnboardingContinue;
                // No Skip once the medium is asked: the medium page has to be answered.
                if (!MediumAsked)
                    dialog.CloseButtonText = T.OnboardingSkip;
                break;
            case Screen.Other:
                ShowOther();
                break;
            case Screen.Medium:
                ShowMedium();
                break;
            case Screen.Lists:
                ShowLists();
                break;
            default:
                ShowSources();
                break;
        }
    }

    // ---------------------------------------------------------------- another device

    void ShowOther()
    {
        dialog.Title = T.FirstOtherTitle;
        if (setup is { } brought)
        {
            ShowBrought(brought);
            return;
        }
        body.Children.Add(Ui.Text(T.FirstOtherLine, "BodyTextBlockStyle"));
        if (!pairing)
        {
            body.Children.Add(Row("WelcomePair", "", T.FirstSetupOther, T.FirstOtherPairLine, StartPairing));
            body.Children.Add(Row("WelcomeRestore", "", T.FirstOtherRestore, T.FirstOtherRestoreLine, () => Leave("restore")));
        }
        else
        {
            // The code to enter on the other device, as Add a device shows it there.
            var codeRow = new Grid { ColumnSpacing = 20, Margin = new Thickness(0, 4, 0, 0) };
            codeRow.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            codeRow.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            qrSlot = new Border { Width = 128, Height = 128, Child = QrCode.Make("", 128) };
            qrFor = null;
            codeRow.Children.Add(qrSlot);
            var words = new StackPanel { Spacing = 6, VerticalAlignment = VerticalAlignment.Center };
            code = new TextBlock
            {
                Name = "WelcomePairCode",
                Text = "––– –––",
                FontFamily = new FontFamily("Cascadia Mono, Consolas"),
                FontSize = 34,
                FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
                IsTextSelectionEnabled = true,
            };
            left = Ui.Text("", "HazeCaptionStyle");
            address = Ui.Text("", "HazeCaptionStyle");
            words.Children.Add(code);
            words.Children.Add(left);
            words.Children.Add(address);
            Grid.SetColumn(words, 1);
            codeRow.Children.Add(words);
            body.Children.Add(codeRow);
            waiting = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Visibility = Visibility.Collapsed };
            waiting.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
            waitingText = Ui.Text(T.PairWaiting, "HazeBodyStyle");
            AutomationProperties.SetLiveSetting(waitingText, AutomationLiveSetting.Polite);
            waiting.Children.Add(waitingText);
            body.Children.Add(waiting);
            RefreshPair();
        }
        dialog.PrimaryButtonText = T.FirstOtherNo;
    }

    Border? qrSlot;
    TextBlock? code, left, address, waitingText;
    StackPanel? waiting;

    /// <summary>
    /// Shows the code here and asks the next device to pair for its setup, in the pairing itself:
    /// its stores, servers and answers come as <c>Event.SetupFrom</c>.
    /// </summary>
    void StartPairing()
    {
        pairing = true;
        pairStarted = DateTimeOffset.UtcNow;
        ctx.Send(new Command.SyncGroup(new SyncCommand.AskSetupFrom(null, null)));
        ctx.Known.PairInvite = null;
        ctx.Send(new Command.PairInvite(Sync.SyncDialogs.TypedName));
        clock.Start();
        Show(Screen.Other);
    }

    void StopPairing()
    {
        clock.Stop();
        if (!pairing)
            return;
        pairing = false;
        // Always: a device that stays advertised lists after its code has run out.
        ctx.Send(new Command.PairStop());
        ctx.Known.PairInvite = null;
    }

    void RefreshPair()
    {
        if (!pairing || page != Screen.Other || code is null)
            return;
        var waited = DateTimeOffset.UtcNow - pairStarted;
        if (waiting is not null && waitingText is not null)
        {
            waiting.Visibility = waited.TotalMilliseconds >= 600 ? Visibility.Visible : Visibility.Collapsed;
            var text = waited.TotalSeconds >= 30 ? T.PairNothingYet : T.PairWaiting;
            if (waitingText.Text != text)
                waitingText.Text = text;
        }
        if (ctx.Known.PairInvite is not { } pairInvite)
            return;
        code.Text = SyncWords.SpacedCode(pairInvite.Code);
        var at = $"{LocalAddress()}:{pairInvite.Port}";
        if (address is not null)
            address.Text = at;
        if (qrSlot is not null && qrFor != pairInvite.Code)
        {
            qrFor = pairInvite.Code;
            qrSlot.Child = QrCode.Make(SyncWords.PairLink(pairInvite.Code, at), 128);
        }
        var seconds = pairInvite.Until - DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        if (left is not null)
        {
            left.Text = seconds <= 0 ? T.PairErrExpired : T.PairGoodFor(SyncWords.MinutesSeconds(seconds));
            left.Foreground = Ui.Res<Brush>(seconds <= 0 ? "SystemFillColorCriticalBrush" : "TextFillColorSecondaryBrush");
        }
    }

    /// <summary>This PC's address on the local network, for the code the other device types or scans.</summary>
    static string LocalAddress()
    {
        try
        {
            using var socket = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp);
            // Nothing is sent: connecting a datagram socket only picks the route.
            socket.Connect("192.0.2.1", 9);
            if (socket.LocalEndPoint is IPEndPoint { } local)
                return local.Address.ToString();
        }
        catch (SocketException)
        {
        }
        return "127.0.0.1";
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            if (e is not Event.SetupFrom from)
                continue;
            setup = from.V1;
            StopPairing();
            storesOn.Clear();
            serversOn.Clear();
            foreach (var store in setup.Stores.Where(s => s.InviteFrom is not null))
                storesOn.Add(store.Address);
            foreach (var server in setup.Servers.Where(s => !s.Added))
                serversOn.Add(server.Id);
            if (setup.Medium is { } theirs && medium is null)
                medium = theirs;
            if (page == Screen.Other)
                Show(Screen.Other);
        }
    }

    /// <summary>"From Desk": what the other device brings, each to keep or leave, before anything is joined.</summary>
    void ShowBrought(JoinedSetup brought)
    {
        var device = brought.DeviceName;
        var heading = Heading(T.FirstJoinGroup(device));
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
        body.Children.Add(heading);
        if (MediumAsked)
        {
            var answers = new[] { KaeriIsFor.Watch, KaeriIsFor.Read, KaeriIsFor.Both };
            var box = new ComboBox { Name = "WelcomeJoinMedium", MinWidth = 160, ItemsSource = answers.Select(MediumDialogs.Name).ToList() };
            AutomationProperties.SetName(box, T.FirstJoinMedium);
            box.SelectedIndex = Array.IndexOf(answers, medium ?? KaeriIsFor.Watch);
            box.SelectionChanged += (_, _) =>
            {
                if (box.SelectedIndex >= 0)
                    medium = answers[box.SelectedIndex];
            };
            body.Children.Add(Line(T.FirstJoinMedium, brought.Medium == KaeriIsFor.Read ? T.FirstJoinMediumLine(device) : null, box));
        }
        if (brought.SignedIn.Length > 0)
        {
            var names = brought.SignedIn.Select(KaeriFfiMethods.TrackerName).ToList();
            var joined = names.Count == 1 ? names[0] : T.NamesPair(names.Take(names.Count - 1).Skip(1).Aggregate(names[0], T.NamesList), names[^1]);
            body.Children.Add(Ui.Haze(T.FirstJoinSigninsLine(joined)));
        }
        foreach (var store in brought.Stores.Where(s => s.InviteFrom is not null))
        {
            var id = store.Address;
            body.Children.Add(Line(T.FirstJoinStore(store.Name), T.FirstJoinStoreLine(device, SyncWords.Kind), Switch(storesOn, id, T.FirstJoinStore(store.Name))));
        }
        foreach (var server in brought.Servers.Where(s => !s.Added))
        {
            var id = server.Id;
            var title = T.FirstJoinServer(server.Host);
            body.Children.Add(Line(title, T.FirstJoinServerLine(BookWords.Kind(server.Kind), device, T.SecureStoreWindows), Switch(serversOn, id, title)));
        }
        if (brought.Folders > 0)
            body.Children.Add(Ui.Haze(T.FirstJoinFolders(device)));
        if (Order().Skip(2).All(p => p == Screen.Medium || brought.Answered.Contains(Answers(p))))
            body.Children.Add(Ui.Haze(T.FirstJoinStepsDone));
        dialog.PrimaryButtonText = T.OnboardingContinue;
    }

    /// <summary>The engine's name for a first-run page.</summary>
    static FirstRunPage Answers(Screen step) => step switch
    {
        Screen.Medium => FirstRunPage.Medium,
        Screen.Lists => FirstRunPage.Lists,
        _ => FirstRunPage.Sources,
    };

    ToggleSwitch Switch(HashSet<string> on, string id, string name)
    {
        var toggle = new ToggleSwitch { IsOn = on.Contains(id), OnContent = "", OffContent = "", MinWidth = 0 };
        AutomationProperties.SetName(toggle, name);
        toggle.Toggled += (_, _) =>
        {
            if (toggle.IsOn)
                on.Add(id);
            else
                on.Remove(id);
        };
        return toggle;
    }

    /// <summary>Joins and adds what was kept, answers the medium, and skips the pages the setup answered.</summary>
    void TakeSetup(JoinedSetup brought)
    {
        if (storesOn.Count > 0)
            ctx.Send(new Command.SyncGroup(new SyncCommand.JoinStoresFrom(brought.Device, [.. storesOn], null)));
        if (serversOn.Count > 0)
            ctx.Send(new Command.SyncGroup(new SyncCommand.AddBookServersFrom(brought.Device, [.. serversOn])));
        foreach (var done in brought.Answered)
            answered.Add(done switch
            {
                FirstRunPage.Medium => Screen.Medium,
                FirstRunPage.Lists => Screen.Lists,
                _ => Screen.Sources,
            });
        if (MediumAsked && medium is { } answer)
        {
            ctx.Known.Medium.Answered(new MediumAsk.FirstRun(), answer);
            answered.Add(Screen.Medium);
        }
        syncingWith = brought.DeviceName;
    }

    /// <summary>The device joined, for Home's notice once the dialog is gone.</summary>
    string? syncingWith;

    // ---------------------------------------------------------------- what Kaeri is for

    void ShowMedium()
    {
        dialog.Title = T.OnboardingMediumTitle;
        body.Children.Add(Ui.Text(T.OnboardingMediumLine, "BodyTextBlockStyle"));
        var hint = Ui.Text(T.OnboardingMediumHint, "HazeCaptionStyle");
        body.Children.Add(MediumDialogs.Rows(medium, answer =>
        {
            medium = answer;
            dialog.IsPrimaryButtonEnabled = true;
            hint.Visibility = Visibility.Collapsed;
        }, "WelcomeMedium"));
        hint.Visibility = medium is null ? Visibility.Visible : Visibility.Collapsed;
        body.Children.Add(hint);
        // No Skip and nothing chosen: Continue waits for one of the three.
        dialog.PrimaryButtonText = T.OnboardingContinue;
        dialog.IsPrimaryButtonEnabled = medium is not null;
    }

    // ---------------------------------------------------------------- lists

    void ShowLists()
    {
        var anilist = KaeriFfiMethods.TrackerName("anilist");
        if (!MediumAsked)
        {
            // Before reading: the page as it was, AniList by name.
            dialog.Title = anilist;
            body.Children.Add(Ui.Text(T.AccountsWelcomeDetail(anilist), "BodyTextBlockStyle"));
            body.Children.Add(Ui.Text(T.AccountsOtherTrackersNote, "HazeCaptionStyle"));
            dialog.PrimaryButtonText = T.AccountsSignInTo(anilist);
            dialog.SecondaryButtonText = T.OnboardingNotNow;
            return;
        }
        dialog.Title = T.OnboardingListsTitle;
        body.Children.Add(Ui.Text(Answer switch
        {
            KaeriIsFor.Read => T.OnboardingListsLineRead(anilist),
            KaeriIsFor.Both => T.OnboardingListsLineBoth(anilist),
            _ => T.OnboardingListsLineWatch(anilist),
        }, "BodyTextBlockStyle"));
        body.Children.Add(Line(anilist, T.OnboardingListsAnilistLine, null));
        body.Children.Add(Ui.Haze(Answer switch
        {
            KaeriIsFor.Read => T.OnboardingListsLaterRead,
            KaeriIsFor.Both => T.OnboardingListsLaterBoth,
            _ => T.OnboardingListsLaterWatch,
        }));
        dialog.PrimaryButtonText = T.OnboardingConnect(anilist);
        dialog.SecondaryButtonText = T.OnboardingNotNow;
    }

    // ---------------------------------------------------------------- sources

    void ShowSources()
    {
        switch (Answer)
        {
            case KaeriIsFor.Read:
                dialog.Title = T.OnboardingBooksTitle;
                body.Children.Add(Ui.Text(T.OnboardingBooksLine, "BodyTextBlockStyle"));
                DemoCard(both: false);
                body.Children.Add(Row("WelcomeBookFolder", "", T.OnboardingBooksFolder, T.OnboardingBooksFolderLine, () => Leave("book-folder")));
                body.Children.Add(Row("WelcomeBookServer", "", T.OnboardingBooksServer, T.OnboardingBooksServerLine, () => Leave("book-server")));
                InviteRows(T.OnboardingBooksInviteHeading, T.OnboardingBooksInvitePlaceholder, T.OnboardingBooksPluginLine);
                // No demo to play here: Join is the amber one when an invite opened the page.
                join.Style = Ui.Res<Style>("AccentButtonStyle");
                dialog.CloseButtonText = T.OnboardingSkipForNow;
                dialog.DefaultButton = ContentDialogButton.None;
                Check();
                return;
            case KaeriIsFor.Both:
                dialog.Title = T.OnboardingBothTitle;
                body.Children.Add(Ui.Text(T.OnboardingBothLine, "BodyTextBlockStyle"));
                body.Children.Add(Part(T.OnboardingBothShows));
                body.Children.Add(Row("WelcomeShowFolder", "", T.SourcesAddFolderButton, T.OnboardingBothShowsFolderLine(SyncWords.Kind), () => Leave("folder")));
                body.Children.Add(Row("WelcomeShowServer", "", T.SourcesAddServerButton, T.OnboardingBothShowsServerLine, () => Leave("server")));
                body.Children.Add(Part(T.OnboardingBothBooks));
                body.Children.Add(Row("WelcomeBookFolder", "", T.OnboardingBooksFolder, T.OnboardingBooksFolderLine, () => Leave("book-folder")));
                body.Children.Add(Row("WelcomeBookServer", "", T.OnboardingBooksServer, T.OnboardingBooksServerLine, () => Leave("book-server")));
                if (!DemoCard(both: true))
                {
                    var tryIt = Heading(T.OnboardingDemoHeading);
                    AutomationProperties.SetHeadingLevel(tryIt, AutomationHeadingLevel.Level2);
                    body.Children.Add(tryIt);
                    body.Children.Add(Ui.Text(T.OnboardingDemoDetail, "BodyTextBlockStyle"));
                }
                InviteRows(T.OnboardingInviteHeading, T.StoresInviteLinkHint, T.PluginRiskWarning);
                break;
            default:
                // Where episodes come from: your own media, the demo to try now, and the invite field for a store.
                dialog.Title = T.OnboardingWhereTitle;
                body.Children.Add(Ui.Text(T.OnboardingWhereHosts, "BodyTextBlockStyle"));
                body.Children.Add(Ui.Haze(T.OnboardingWhereStores));
                var own = Heading(T.OnboardingOwnHeading);
                AutomationProperties.SetHeadingLevel(own, AutomationHeadingLevel.Level2);
                body.Children.Add(own);
                var own2 = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
                own2.Children.Add(Ui.Label(T.SourcesAddFolderButton, (_, _) => Leave("folder")));
                own2.Children.Add(Ui.Label(T.SourcesAddServerButton, (_, _) => Leave("server")));
                body.Children.Add(own2);
                var demo = Heading(T.OnboardingDemoHeading);
                AutomationProperties.SetHeadingLevel(demo, AutomationHeadingLevel.Level2);
                body.Children.Add(demo);
                body.Children.Add(Ui.Text(T.OnboardingDemoDetail, "BodyTextBlockStyle"));
                InviteRows(T.OnboardingInviteHeading, T.StoresInviteLinkHint, T.PluginRiskWarning);
                break;
        }
        // One amber button: Play the demo, or Join when an invite link opened this page.
        dialog.PrimaryButtonText = T.OnboardingPlayDemo;
        dialog.PrimaryButtonStyle = linked ? null : Ui.Res<Style>("AccentButtonStyle");
        join.Style = linked ? Ui.Res<Style>("AccentButtonStyle") : null;
        dialog.CloseButtonText = T.OnboardingSkipForNow;
        dialog.DefaultButton = linked ? ContentDialogButton.None : ContentDialogButton.Primary;
        Check();
    }

    void InviteRows(string heading, string placeholder, string warning)
    {
        var link = Heading(heading);
        AutomationProperties.SetHeadingLevel(link, AutomationHeadingLevel.Level2);
        body.Children.Add(link);
        invite.PlaceholderText = placeholder;
        var row = new Grid { ColumnSpacing = 8 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        (invite.Parent as Panel)?.Children.Remove(invite);
        (join.Parent as Panel)?.Children.Remove(join);
        (problem.Parent as Panel)?.Children.Remove(problem);
        row.Children.Add(invite);
        Grid.SetColumn(join, 1);
        row.Children.Add(join);
        body.Children.Add(row);
        body.Children.Add(problem);
        body.Children.Add(Ui.Haze(warning));
    }

    // ---------------------------------------------------------------- pieces

    /// <summary>A row to press: an icon, a title and the line under it, as a full-width button.</summary>
    static Button Row(string name, string glyph, string title, string line, Action act)
    {
        var grid = new Grid { ColumnSpacing = 12 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.Children.Add(new FontIcon { Glyph = glyph, FontSize = 18, VerticalAlignment = VerticalAlignment.Center });
        var words = new StackPanel { Spacing = 2 };
        words.Children.Add(Ui.Text(title, "BodyStrongTextBlockStyle", wrap: false));
        words.Children.Add(Ui.Text(line, "HazeCaptionStyle"));
        Grid.SetColumn(words, 1);
        grid.Children.Add(words);
        var chevron = new FontIcon { Glyph = "", FontSize = 12, VerticalAlignment = VerticalAlignment.Center };
        Grid.SetColumn(chevron, 2);
        grid.Children.Add(chevron);
        var button = new Button
        {
            Name = name,
            Content = grid,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Padding = new Thickness(14, 10, 14, 10),
        };
        AutomationProperties.SetName(button, title);
        AutomationProperties.SetHelpText(button, line);
        button.Click += (_, _) => act();
        return button;
    }

    /// <summary>A title and its line, with a control at the end.</summary>
    static Grid Line(string title, string? line, FrameworkElement? control)
    {
        var grid = new Grid
        {
            ColumnSpacing = 12,
            Padding = new Thickness(14, 10, 10, 10),
            CornerRadius = new CornerRadius(8),
            Background = Ui.Res<Brush>("RaisedBrush"),
        };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        words.Children.Add(Ui.Text(title, "BodyTextBlockStyle"));
        if (line is { Length: > 0 })
            words.Children.Add(Ui.Text(line, "HazeCaptionStyle"));
        grid.Children.Add(words);
        if (control is not null)
        {
            control.VerticalAlignment = VerticalAlignment.Center;
            Grid.SetColumn(control, 1);
            grid.Children.Add(control);
        }
        return grid;
    }

    /// <summary>
    /// Try it now, with reading on: one raised card with the demo's line and Read the demo, which
    /// ends first run on Read's Home, where the demo is the spotlight. With both, the line names the
    /// episode and the comic, and Play the demo stays the dialog's own button. The comic's first
    /// pages are fetched ahead while the page shows. False when the demo has no reading part.
    /// </summary>
    bool DemoCard(bool both)
    {
        if (!ctx.Known.Demo.Reading)
            return false;
        ctx.Send(new Command.BookSources(new BookSourceCommand.PrefetchDemo()));
        var card = new StackPanel
        {
            Spacing = 6,
            Padding = new Thickness(14, 12, 14, 14),
            CornerRadius = new CornerRadius(8),
            Background = Ui.Res<Brush>("RaisedBrush"),
            Margin = new Thickness(0, 6, 0, 0),
        };
        var heading = Ui.Text(T.OnboardingDemoHeading, "BodyStrongTextBlockStyle");
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
        card.Children.Add(heading);
        card.Children.Add(Ui.Haze(both ? T.OnboardingDemoBothDetail : T.OnboardingDemoBooksDetail));
        var read = Ui.Label(T.OnboardingReadDemo, (_, _) => Leave("read-demo"));
        read.Name = "WelcomeReadDemo";
        read.Margin = new Thickness(0, 6, 0, 0);
        card.Children.Add(read);
        body.Children.Add(card);
        return true;
    }

    static TextBlock Part(string text)
    {
        var part = Ui.Text(text, "BodyStrongTextBlockStyle", wrap: false);
        part.Margin = new Thickness(0, 6, 0, 0);
        AutomationProperties.SetHeadingLevel(part, AutomationHeadingLevel.Level3);
        return part;
    }

    void Leave(string then)
    {
        Then = then;
        dialog.Hide();
    }

    void Check()
    {
        if (page != Screen.Sources)
            return;
        var link = Stores.InviteIn(invite.Text);
        join.IsEnabled = link is not null;
        problem.Text = invite.Text.Trim().Length > 0 && link is null ? T.StoresNotAnInvite : "";
        problem.Visibility = problem.Text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    void OnPrimary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        switch (page)
        {
            case Screen.Welcome:
                args.Cancel = true;
                Show(Screen.Other);
                break;
            case Screen.Other:
                args.Cancel = true;
                if (setup is { } brought)
                    TakeSetup(brought);
                Next();
                // The last page answered by the setup: the app opens on Home with the notice.
                if (syncingWith is { } device && !Order().Skip(Order().IndexOf(Screen.Other) + 1).Any(p => !answered.Contains(p)))
                    ctx.Window.Say(T.FirstJoinSyncing(device));
                break;
            case Screen.Medium:
                args.Cancel = true;
                if (medium is not { } answer)
                    return;
                ctx.Known.Medium.Answered(new MediumAsk.FirstRun(), answer);
                answered.Add(Screen.Medium);
                Next();
                break;
            case Screen.Lists:
                args.Cancel = true;
                ctx.SignIn("anilist");
                Next();
                break;
            default:
                // Play the demo: first launch closes into the player on part 1.
                ctx.Known.DemoPlay = true;
                ctx.Send(new Command.PlayDemo());
                break;
        }
    }

    /// <summary>The join dialog shows the store before anything is joined; it opens once this closes.</summary>
    void JoinNow()
    {
        if (Stores.InviteIn(invite.Text) is not { } link)
            return;
        Invite = link;
        dialog.Hide();
    }

    static TextBlock Heading(string text) => new()
    {
        Text = text,
        Style = Ui.Res<Style>("BodyStrongTextBlockStyle"),
        Margin = new Thickness(0, 8, 0, 0),
    };

    void OnSecondary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        if (page == Screen.Lists)
        {
            args.Cancel = true;
            Next();
        }
    }
}
