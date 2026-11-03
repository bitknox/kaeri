using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Plugin stores: the ones this device has joined, and for the selected one what it offers (with
/// the hosts each plugin contacts) and what to do with it: install, update, remove, leave, open
/// its admin page. A store's owner also invites people and sees who has joined.
/// </summary>
public sealed partial class StoresPage : Page
{
    readonly Ctx ctx = Ctx.Current;
    string? selected;
    bool quiet;
    string? inviteName;

    /// <summary>The owner's member rows, kept across redraws so their 600 ms is not started again.</summary>
    readonly StateHost membersState = new() { Placeholders = () => States.Rows(2, 0, 0, rowHeight: 36, spacing: 0) };
    readonly StackPanel memberRows = new();

    public StoresPage()
    {
        InitializeComponent();
        membersState.Children.Add(memberRows);
        // The store rows: a name and a host, at the rows' own height.
        State.Placeholders = () => States.Rows(3, 0, 0, rowHeight: 56, spacing: 0);
        // The joined stores are kept on this device; offline, the list still comes.
        State.NeedsConnection = false;
        ctx.Hear(this, Heard);
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        selected = e.Parameter as string ?? ctx.Options.Select;
        ctx.Send(new Command.LoadStores());
        Show();
    }

    internal void Select(string store)
    {
        selected = store;
        Show();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.Stores or Event.StoreMembers or Event.InviteLink or Event.Providers))
            Show();
        foreach (var e in batch)
        {
            if (e is Event.StoreJoined joined)
                Select(joined.Store);
            // A made invite opens as a sheet with its QR code, once.
            if (e is Event.InviteLink made && ctx.Known.Invite is { } invite && made.V1.Store == selected)
                InviteDialog.Show(ctx, XamlRoot, invite);
        }
    }

    void Show()
    {
        quiet = true;
        var stores = ctx.Known.Stores;
        var current = stores.FirstOrDefault(s => s.Id == selected) ?? stores.FirstOrDefault();
        selected = current?.Id;
        Empty.Visibility = stores.Length == 0 && ctx.Known.StoresLoaded ? Visibility.Visible : Visibility.Collapsed;
        if (ctx.Known.StoresLoaded)
            State.Done();
        else
            State.Wait();
        StoreList.Items.Clear();
        foreach (var store in stores)
        {
            var item = new ListViewItem { Content = Line(store), Tag = store.Id };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(item, store.Name);
            StoreList.Items.Add(item);
            if (store.Id == current?.Id)
                StoreList.SelectedItem = item;
        }
        ShowDetail(current);
        quiet = false;
    }

    static Grid Line(StoreRow store)
    {
        var grid = new Grid { Height = 56, ColumnSpacing = 10 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(Ui.Text(store.Name, "BodyStrongTextBlockStyle", wrap: false));
        text.Children.Add(Ui.Text(store.Host, "HazeCaptionStyle", wrap: false));
        grid.Children.Add(text);
        var state = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, VerticalAlignment = VerticalAlignment.Center };
        Grid.SetColumn(state, 1);
        switch (store.State)
        {
            case StoreState.Offline:
                state.Children.Add(Ui.Dot(Colours.Haze, ring: true));
                state.Children.Add(Ui.Text(T.StoresStatusOffline, "HazeCaptionStyle", wrap: false));
                break;
            case StoreState.Revoked:
                state.Children.Add(Ui.Dot(Ui.Vermilion));
                state.Children.Add(Ui.Text(T.StoresStatusAccessEnded, "HazeCaptionStyle", wrap: false));
                break;
            default:
                if (Stores.Updates(store) is > 0 and var updates)
                {
                    state.Children.Add(Ui.Dot(Colours.Haze));
                    state.Children.Add(Ui.Text(T.StoresUpdatesCount(updates), "HazeCaptionStyle", wrap: false));
                }
                break;
        }
        grid.Children.Add(state);
        return grid;
    }

    void ShowDetail(StoreRow? store)
    {
        Detail.Children.Clear();
        if (store is null)
            return;
        Detail.Children.Add(Ui.Text(store.Name, "SubtitleTextBlockStyle"));
        Detail.Children.Add(Ui.Haze(Stores.RunBy(store.Owners, store.Host)));
        Detail.Children.Add(Ui.Text(T.StoresKeyLine(store.Fingerprint, Stores.JoinedAs(store)), "HazeCaptionStyle"));
        switch (store.State)
        {
            // The engine's reason is English from the network: the banner says what it means.
            case StoreState.Offline:
                Detail.Children.Add(Banner(InfoBarSeverity.Warning, T.StoresStatusNotAnswering, T.StoresOfflineKeepWorking));
                break;
            case StoreState.Revoked:
                Detail.Children.Add(Banner(InfoBarSeverity.Error, T.StoresStatusAccessEnded, T.StoresAccessEndedDetail));
                break;
        }

        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 4, 0, 0) };
        if (store.Owner)
            actions.Children.Add(Ui.Label(T.StoresOpenAdmin, (_, _) => ctx.Send(new Command.OpenAdmin(store.Id))));
        actions.Children.Add(Ui.Label(T.StoresLeave, (_, _) => ConfirmLeave(store)));
        Detail.Children.Add(actions);

        Detail.Children.Add(Ui.Heading(T.StoresPlugins));
        var plugins = store.Plugins ?? [];
        if (plugins.Length == 0)
            Detail.Children.Add(Ui.Haze(T.StoresNoPlugins));
        foreach (var plugin in plugins)
            Detail.Children.Add(Plugin(store, plugin));

        if (store.Owner)
            Owner(store);
    }

    static InfoBar Banner(InfoBarSeverity severity, string title, string text) =>
        new() { IsOpen = true, IsClosable = false, Severity = severity, Title = title, Message = text };

    UIElement Plugin(StoreRow store, StorePluginRow plugin)
    {
        var grid = new Grid { ColumnSpacing = 12 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var text = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(Ui.Text(plugin.Name, "BodyStrongTextBlockStyle", wrap: false));
        // The update line names both versions already; any other standing goes before the version.
        var version = T.StoresPluginVersion(Stores.InstalledVersion(plugin));
        var meta = Stores.Standing(plugin) is not { } standing ? version
            : plugin.Status is StorePluginStatus.Update ? standing
            : T.StoresStandingVersion(standing, version);
        text.Children.Add(Ui.Text(meta, "HazeCaptionStyle"));
        text.Children.Add(Ui.Text(Stores.ContactsLine(plugin), "HazeCaptionStyle"));
        if (plugin.Status is StorePluginStatus.Update { NewHosts: { Length: > 0 } added })
            text.Children.Add(Ui.Text(T.StoresUpdateContacts(string.Join(", ", added)), "HazeCaptionStyle"));
        grid.Children.Add(text);

        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
        switch (plugin.Status)
        {
            case StorePluginStatus.Available:
                buttons.Children.Add(Ui.Label(T.ActionInstall, (_, _) => ctx.Send(new Command.InstallPlugin(store.Id, plugin.Id))));
                break;
            case StorePluginStatus.Update:
                buttons.Children.Add(Ui.Label(T.ActionUpdate, (_, _) => Update(store, plugin)));
                buttons.Children.Add(Ui.Label(T.ActionRemove, (_, _) => RemovePlugin(plugin)));
                break;
            case StorePluginStatus.Installed or StorePluginStatus.AddedByHand:
                buttons.Children.Add(Ui.Label(T.ActionRemove, (_, _) => RemovePlugin(plugin)));
                break;
            case StorePluginStatus.OtherStore:
                break;
        }
        Grid.SetColumn(buttons, 1);
        grid.Children.Add(buttons);
        return Ui.Card(grid);
    }

    void Owner(StoreRow store)
    {
        Detail.Children.Add(Ui.Heading(T.StoresPeople));
        var members = ctx.Known.Members.GetValueOrDefault(store.Id);
        if (!ctx.Known.Members.ContainsKey(store.Id))
            ctx.Send(new Command.LoadMembers(store.Id));

        MakeInvite(store, members.InviteChoices || !ctx.Known.Members.ContainsKey(store.Id));

        // The members come from the store: plates in their place while it answers.
        memberRows.Children.Clear();
        Detail.Children.Add(membersState);
        if (ctx.Known.Members.ContainsKey(store.Id))
            membersState.Done();
        else
            membersState.Wait();
        foreach (var member in members.Members ?? [])
        {
            var line = new Grid { ColumnSpacing = 8, Height = 36 };
            line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var label = member.You ? T.PartyMemberYou(member.Name) : member.Name;
            var devices = T.StoresMemberDevices(member.Devices);
            var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
            text.Children.Add(Ui.Text(label, wrap: false));
            text.Children.Add(Ui.Text(member.Owner ? T.StoresOwnerDevices(devices) : devices, "HazeCaptionStyle", wrap: false));
            line.Children.Add(text);
            if (!member.You && !member.Owner)
            {
                var remove = Ui.Label(T.ActionRemove, async (_, _) =>
                {
                    if (await Ui.Confirm(XamlRoot, T.StoresRemoveMemberTitle(member.Name), T.StoresRemoveMemberDetail, T.ActionRemove))
                        ctx.Send(new Command.RemoveMember(store.Id, member.Id));
                });
                Grid.SetColumn(remove, 1);
                line.Children.Add(remove);
            }
            memberRows.Children.Add(line);
        }
        InviteLists(store, members.Invites ?? [], members.Ended ?? []);

        var open = new ToggleSwitch { IsOn = store.Open, OnContent = "", OffContent = "", Header = T.StoresWhoCanJoin };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(open, T.StoresWhoCanJoin);
        var says = Ui.Text(store.Open ? T.StoresAnyoneWithLink : T.StoresOnlyInvited, "HazeCaptionStyle");
        open.Toggled += (_, _) =>
        {
            says.Text = open.IsOn ? T.StoresAnyoneWithLink : T.StoresOnlyInvited;
            if (!quiet)
                ctx.Send(new Command.SetOpenJoin(store.Id, open.IsOn));
        };
        Detail.Children.Add(open);
        Detail.Children.Add(says);
    }

    InviteUses uses = InviteUses.Once;
    InviteLasts lasts = InviteLasts.Week;
    readonly HashSet<string> usesOpen = [];

    static string UsesName(InviteUses uses) => uses switch
    {
        InviteUses.Five => T.StoresInviteUsesFive,
        InviteUses.Any => T.StoresInviteUsesAny,
        _ => T.StoresInviteUsesOnce,
    };

    static string LastsName(InviteLasts lasts) => lasts switch
    {
        InviteLasts.Day => T.StoresInviteLastsDay,
        InviteLasts.Month => T.StoresInviteLastsMonth,
        InviteLasts.Forever => T.StoresInviteLastsForever,
        _ => T.StoresInviteLastsWeek,
    };

    /// <summary>The line under the choices, and under a made link: how often it works and until when.</summary>
    static string Works(uint max, long? until) => (max, until) switch
    {
        (0, { } u) => T.StoresInviteWorksAny(Wording.Date(u)),
        (0, null) => T.StoresInviteWorksAnyForever,
        (1, { } u) => T.StoresInviteOnceUntil(Wording.Date(u)),
        (1, null) => T.StoresInviteOnceForever,
        (_, { } u) => T.StoresInviteWorksTimes(max, Wording.Date(u)),
        (_, null) => T.StoresInviteWorksTimesForever(max),
    };

    static long? Until(InviteLasts lasts) => lasts switch
    {
        InviteLasts.Day => DateTimeOffset.UtcNow.AddDays(1).ToUnixTimeSeconds(),
        InviteLasts.Month => DateTimeOffset.UtcNow.AddDays(30).ToUnixTimeSeconds(),
        InviteLasts.Forever => null,
        _ => DateTimeOffset.UtcNow.AddDays(7).ToUnixTimeSeconds(),
    };

    /// <summary>
    /// The invite maker: a name, how many times the link works and for how long, then the link.
    /// A store whose server predates the choices keeps once and a week and says so.
    /// </summary>
    void MakeInvite(StoreRow store, bool choices)
    {
        var group = choices && uses != InviteUses.Once;
        var invite = new Grid { ColumnSpacing = 8 };
        invite.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        invite.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var name = new TextBox { PlaceholderText = group ? T.StoresInviteGroupName : T.StoresInviteFor, Text = inviteName ?? "" };
        name.TextChanged += (_, _) => inviteName = name.Text;
        invite.Children.Add(name);
        var make = Ui.Label(T.StoresMakeLink, (_, _) => ctx.Send(new Command.CreateInvite(
            store.Id, string.IsNullOrWhiteSpace(inviteName) ? null : inviteName.Trim(), choices ? uses : null, choices ? lasts : null)));
        Grid.SetColumn(make, 1);
        invite.Children.Add(make);
        Detail.Children.Add(invite);

        if (choices)
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
            ComboBox Choice<TValue>(string header, TValue[] values, TValue current, Func<TValue, string> label, Action<TValue> pick) where TValue : struct, Enum
            {
                var box = new ComboBox { Header = header, MinWidth = 180, ItemsSource = values.Select(label).ToList(), SelectedIndex = Array.IndexOf(values, current) };
                Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, header);
                box.SelectionChanged += (_, _) =>
                {
                    if (!quiet && box.SelectedIndex >= 0)
                    {
                        pick(values[box.SelectedIndex]);
                        Show();
                    }
                };
                return box;
            }
            row.Children.Add(Choice(T.StoresInviteUses, new[] { InviteUses.Once, InviteUses.Five, InviteUses.Any }, uses, UsesName, v => uses = v));
            row.Children.Add(Choice(T.StoresInviteLasts, new[] { InviteLasts.Day, InviteLasts.Week, InviteLasts.Month, InviteLasts.Forever }, lasts, LastsName, v => lasts = v));
            Detail.Children.Add(row);
            Detail.Children.Add(Ui.Text(Works(uses switch { InviteUses.Five => 5u, InviteUses.Any => 0u, _ => 1u }, Until(lasts)), "HazeCaptionStyle"));
            if (uses != InviteUses.Once)
                Detail.Children.Add(Ui.Text(T.StoresInviteGroupDetail, "HazeCaptionStyle"));
        }
        else
            Detail.Children.Add(Ui.Text(T.StoresServerOlderInvites, "HazeCaptionStyle"));

        if (ctx.Known.Invite is { } link && link.Store == store.Id)
        {
            var box = new TextBox { Text = link.Link, IsReadOnly = true };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, T.StoresInviteLink);
            var made = new Grid { ColumnSpacing = 8 };
            made.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            made.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            made.Children.Add(box);
            var copy = Ui.Label(T.ActionCopy, (_, _) => Ui.TryCopy(link.Link));
            Grid.SetColumn(copy, 1);
            made.Children.Add(copy);
            Detail.Children.Add(made);
            Detail.Children.Add(Ui.Text(Works(link.Max, link.Until), "HazeCaptionStyle"));
        }
    }

    /// <summary>"Used 2 of 5 · Until 7 November": how much of a link is spent, and when it ends.</summary>
    static string Facts(StoreInviteRow invite)
    {
        var used = invite.Max == 1 || invite.Used == 0
            ? invite.Used == 0 ? T.StoresInviteNotUsed : T.StoresInviteUsed(invite.Used)
            : invite.Max == 0 ? T.StoresInviteUsed(invite.Used) : T.StoresInviteUsedOf(invite.Used, invite.Max);
        return invite.Until is { } until ? $"{used} · {T.StoresInvitedUntil(Wording.Date(until))}" : used;
    }

    static string EndedFacts(StoreInviteRow invite) => invite.Ended switch
    {
        { Reason: InviteEndReason.UsedUp } e => T.StoresInviteUsedUp(Wording.Date(e.At)),
        { Reason: InviteEndReason.Cancelled } e => T.StoresInviteCancelledOn(Wording.Date(e.At)),
        { } e => T.StoresInviteRanOut(Wording.Date(e.At)),
        _ => "",
    };

    /// <summary>"Invite links" that still work, then the ones that ended in the last 30 days, dimmed.</summary>
    void InviteLists(StoreRow store, StoreInviteRow[] working, StoreInviteRow[] ended)
    {
        Detail.Children.Add(Ui.Heading(T.StoresInviteLinks));
        if (working.Length == 0)
            Detail.Children.Add(Ui.Haze(T.StoresNoInvites));
        foreach (var pending in working)
            Detail.Children.Add(InviteRow(store, pending, Facts(pending), ended: false));
        if (ended.Length == 0)
            return;
        Detail.Children.Add(Ui.Heading(T.StoresInvitesEnded));
        foreach (var gone in ended)
            Detail.Children.Add(InviteRow(store, gone, EndedFacts(gone), ended: true));
    }

    UIElement InviteRow(StoreRow store, StoreInviteRow invite, string facts, bool ended)
    {
        var rows = new StackPanel { Spacing = 4, Opacity = ended ? 0.6 : 1 };
        var line = new Grid { ColumnSpacing = 8, MinHeight = 44 };
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(Ui.Text(invite.Name is { Length: > 0 } n ? T.StoresInviteForName(n) : T.StoresInviteUnnamed, wrap: false));
        text.Children.Add(Ui.Text(facts, "HazeCaptionStyle", wrap: false));
        line.Children.Add(text);
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
        var people = invite.Uses ?? [];
        if (people.Length > 0)
            buttons.Children.Add(Ui.Label(T.StoresInviteShowUses, (_, _) =>
            {
                if (!usesOpen.Remove(invite.Id))
                    usesOpen.Add(invite.Id);
                Show();
            }));
        if (!ended)
            buttons.Children.Add(Ui.Label(T.StoresCancelInvite, async (_, _) =>
            {
                if (await Ui.Confirm(XamlRoot, T.StoresInviteForName(invite.Name), T.StoresCancelInviteDetail(invite.Used), T.StoresCancelInvite))
                    ctx.Send(new Command.CancelInvite(store.Id, invite.Id));
            }));
        Grid.SetColumn(buttons, 1);
        line.Children.Add(buttons);
        rows.Children.Add(line);
        if (usesOpen.Contains(invite.Id))
            foreach (var use in people)
            {
                var who = new Grid { ColumnSpacing = 8, MinHeight = 32, Margin = new Thickness(16, 0, 0, 0) };
                who.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
                who.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
                who.Children.Add(Ui.Haze(T.StoresInviteJoined(use.Name, Wording.Date(use.At)), wrap: false));
                if (!use.Removed)
                {
                    var remove = Ui.Label(T.ActionRemove, async (_, _) =>
                    {
                        if (await Ui.Confirm(XamlRoot, T.StoresRemoveMemberTitle(use.Name), T.StoresRemoveMemberDetail, T.ActionRemove))
                            ctx.Send(new Command.RemoveMember(store.Id, use.Member));
                    });
                    Grid.SetColumn(remove, 1);
                    who.Children.Add(remove);
                }
                rows.Children.Add(who);
            }
        return rows;
    }

    async void RemovePlugin(StorePluginRow plugin)
    {
        if (await Ui.Confirm(XamlRoot, T.StoresRemovePluginTitle(plugin.Name), T.StoresRemovePluginDetail, T.ActionRemove))
            ctx.Send(new Command.RemovePlugin(plugin.Id));
    }

    /// <summary>An update that would contact new hosts names them and asks first.</summary>
    async void Update(StoreRow store, StorePluginRow plugin)
    {
        if (plugin.Status is StorePluginStatus.Update { NewHosts: { Length: > 0 } added }
            && !await Ui.Confirm(XamlRoot, T.SourcesUpdateTitle(plugin.Name), T.StoresUpdateContacts(string.Join(", ", added)), T.ActionUpdate))
            return;
        ctx.Send(new Command.InstallPlugin(store.Id, plugin.Id));
    }

    /// <summary>Below about 760 the detail goes under the list, as one column.</summary>
    void OnColumnsSizeChanged(object sender, SizeChangedEventArgs e)
    {
        var narrow = e.NewSize.Width < 760;
        ListColumn.Width = narrow ? new GridLength(1, GridUnitType.Star) : new GridLength(320);
        DetailColumn.Width = narrow ? new GridLength(0) : new GridLength(1, GridUnitType.Star);
        ListRow.Height = narrow ? GridLength.Auto : new GridLength(1, GridUnitType.Star);
        DetailRow.Height = narrow ? new GridLength(1, GridUnitType.Star) : new GridLength(0);
        Grid.SetColumn(DetailScroll, narrow ? 0 : 1);
        Grid.SetRow(DetailScroll, narrow ? 1 : 0);
        ListScroll.MaxHeight = narrow ? Math.Max(160, e.NewSize.Height * 0.35) : double.PositiveInfinity;
    }

    async void ConfirmLeave(StoreRow store)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Title = T.StoresLeaveTitle(store.Name),
            Content = T.StoresLeaveRejoin,
            PrimaryButtonText = T.ActionLeave,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Close,
        };
        if (await Dialogs.Show(dialog) == ContentDialogResult.Primary)
            ctx.Send(new Command.LeaveStore(store.Id));
    }

    void OnSelected(object sender, SelectionChangedEventArgs e)
    {
        if (quiet || StoreList.SelectedItem is not ListViewItem { Tag: string id })
            return;
        selected = id;
        ShowDetail(ctx.Known.Stores.FirstOrDefault(s => s.Id == id));
    }

    void OnJoinEmpty(object sender, RoutedEventArgs e) => ctx.Window.ShowJoinStore();

    async void OnJoin(object sender, RoutedEventArgs e) => await new JoinStoreDialog(ctx, XamlRoot).ShowAsync();
}
