using CommunityToolkit.WinUI.Controls;
using Kaeri.App;
using Kaeri.Engine.Model;
using Kaeri.Windows.Sync;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Settings, Devices: this PC's name, the devices in the group with their Sync and Remote
/// switches, the cloud folder, the recovery key and Turn off sync last; or, while sync is off,
/// the one button that turns it on. The TVs paired for Play on TV sit here too, and keep working
/// with sync off. The engine holds every rule; the pane shows what it last said.
/// </summary>
public sealed partial class SettingsPage
{
    /// <summary>Titles of kept watches the pane asked for and has not shown yet.</summary>
    readonly HashSet<uint> notesWaiting = [];

    static long Now => DateTimeOffset.UtcNow.ToUnixTimeSeconds();

    void SyncChanged()
    {
        if (pane.Name == "devices")
            Show();
    }

    static readonly SyncStatus NoSync = new(false, false, Devices: [], Notes: [], StoresToJoin: [], Clouds: []);

    /// <summary>
    /// What the Devices pane says, from the engine's last word on the group: each device's name,
    /// kind and line, the kept watches, the cloud folder's state and the recovery key. This PC's
    /// name and the devices' switches are left out, as they change in place; the pane is drawn
    /// again only when this changes, so a click in it is not lost and a name being typed stays.
    /// </summary>
    string DevicesKey()
    {
        var status = ctx.Known.SyncGroup ?? NoSync;
        var now = Now;
        // A pass starting or ending, and a device's or the folder's line, change in place: they
        // are not part of the key, so a sync pass never redraws the pane.
        var words = new List<object?> { SyncWords.Bare(status), status.RecoveryKey, status.RecoveryKeyMade, ctx.Tv.Available };
        foreach (var device in status.Devices.Where(d => !d.ThisDevice))
            words.Add((device.Id, device.Name, device.Kind, device.RemoteKey is null));
        foreach (var note in status.Notes)
            words.Add((note, ctx.Known.Entries.TryGetValue(note.AnilistId, out var entry) ? entry.Title : ""));
        if (status.Folder is { } folder)
            words.Add((folder.Kind, folder.Account, CloudButtons(SyncWords.State(status, now, SyncDialogs.ByHand))));
        return Drawn.Fingerprint(words);
    }

    /// <summary>Which buttons and title the cloud card has for a state: what redraws it, where its line alone does not.</summary>
    static string CloudButtons(CloudState state) => state switch
    {
        CloudState.Problem problem => $"problem {problem.Fix}", // i18n-skip
        CloudState.SignedOut => "signed out", // i18n-skip
        _ => "",
    };

    void ShowDevices()
    {
        var status = ctx.Known.SyncGroup ?? NoSync;
        ThisDevice(status);
        if (!SyncWords.Bare(status))
        {
            YourDevices(status);
            SyncNotes(status);
            CloudFolder(status);
            RecoveryAndOff(status);
            PairedTvs(false);
        }
        else
        {
            TwoWaysIn();
            PairedTvs(true);
        }
    }

    /// <summary>
    /// While sync is off: the two ways in, each a card with its buttons, under what sync shares,
    /// and a footnote. Pairing a device or adding a cloud folder turns sync on by itself.
    /// </summary>
    void TwoWaysIn()
    {
        Cards.Children.Add(Heading(T.SyncGroup));
        var lead = Ui.Text(T.DevicesSyncFoot, "HazeCaptionStyle");
        lead.Margin = new Thickness(2, 0, 0, 4);
        Cards.Children.Add(lead);
        var nearby = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var show = new Button { Content = T.PairShowCode };
        show.Click += (_, _) => SyncDialogs.PairShow(Origin.Devices);
        var enter = new Button { Content = T.DevicesEnterCode };
        enter.Click += (_, _) => SyncDialogs.PairEnter();
        nearby.Children.Add(show);
        nearby.Children.Add(enter);
        Cards.Children.Add(CardFrame(T.SyncRoadNearby, T.SyncRoadNearbyLine, nearby, "\uE702"));
        var add = new Button { Content = T.CloudAdd };
        add.Click += (_, _) => SyncDialogs.Picker();
        Cards.Children.Add(CardFrame(T.SyncRoadCloud, T.SyncRoadCloudLine, add, "\uE753"));
        var foot = Ui.Text(T.SyncStartFoot, "HazeCaptionStyle");
        foot.Margin = new Thickness(2, 4, 0, 0);
        Cards.Children.Add(foot);
    }

    void ThisDevice(SyncStatus status)
    {
        Cards.Children.Add(Heading(T.DevicesThisDevice));
        var name = new TextBox { Width = 220, MaxLength = 64, Text = status.DeviceName ?? SyncDialogs.TypedName };
        AutomationProperties.SetName(name, T.DevicesName);
        var saved = name.Text;
        // Said to the group on Enter, or when the box is left with a new name.
        void Save()
        {
            if (name.Text.Trim() is { Length: > 0 } typed && typed != saved)
            {
                saved = typed;
                SyncDialogs.Rename(typed);
            }
        }
        name.KeyDown += (_, e) =>
        {
            if (e.Key == global::Windows.System.VirtualKey.Enter)
            {
                Save();
                e.Handled = true;
            }
        };
        name.LostFocus += (_, _) => Save();
        // The group's word on the name, in place, and never over what is being typed.
        refresh.Add(() =>
        {
            if (name.FocusState != FocusState.Unfocused)
                return;
            var now = ctx.Known.SyncGroup?.DeviceName ?? SyncDialogs.TypedName;
            if (name.Text != now)
                name.Text = saved = now;
        });
        Cards.Children.Add(CardFrame(T.DevicesName, null, name, ""));
    }

    static string DeviceGlyph(SyncDeviceView device) => device.Kind switch
    {
        SyncDeviceKind.Tv => "",
        SyncDeviceKind.Phone or SyncDeviceKind.Tablet => "",
        SyncDeviceKind.Computer => "",
        _ => device.Name.Contains("tv", StringComparison.OrdinalIgnoreCase) ? ""
            : device.Name.Contains("phone", StringComparison.OrdinalIgnoreCase) ? "" : "",
    };

    /// <summary>
    /// A switch with its word before it, as the board's device cards have them. The group's later
    /// word on it, from <paramref name="now"/>, is set in place.
    /// </summary>
    StackPanel Labelled(string word, string name, bool on, Action<bool> said, Func<bool?> now)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        row.Children.Add(new TextBlock { Text = word, VerticalAlignment = VerticalAlignment.Center });
        var toggle = new ToggleSwitch { IsOn = on, OnContent = "", OffContent = "", MinWidth = 0 };
        AutomationProperties.SetName(toggle, name);
        toggle.Toggled += (_, _) =>
        {
            if (!quiet)
                said(toggle.IsOn);
        };
        refresh.Add(() =>
        {
            if (now() is { } value && toggle.IsOn != value)
                toggle.IsOn = value;
        });
        row.Children.Add(toggle);
        return row;
    }

    /// <summary>The device as the group last said, by its id.</summary>
    SyncDeviceView? Device(string id) => ctx.Known.SyncGroup?.Devices.FirstOrDefault(d => d.Id == id);

    SettingsCard DeviceCard(SyncDeviceView device)
    {
        var pairedTv = ctx.Tv.Paired.Any(tv => tv.Name == device.Name);
        var (line, mark) = SyncWords.DeviceLine(device, Now, pairedTv);
        var switches = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 16 };
        // The group's switch: off holds the group back from the device, and it from the group.
        switches.Children.Add(Labelled(T.DevicesSyncSwitch, T.DevicesSyncSwitchName(device.Name), !device.Paused,
            on => ctx.Send(new Command.SyncGroup(new SyncCommand.SetDeviceSync(device.Id, on))),
            () => Device(device.Id) is { } now ? !now.Paused : null));
        // Only for a pair that is a TV and a device that drives TVs.
        if (device.RemoteKey is not null)
            switches.Children.Add(Labelled(T.DevicesRemoteSwitch, $"{T.DevicesRemoteSwitch} {device.Name}", !device.RemoteOff,
                on => ctx.Send(new Command.SyncGroup(new SyncCommand.SetDeviceRemote(device.Id, on))),
                () => Device(device.Id) is { } now ? !now.RemoteOff : null));
        var card = new SettingsCard
        {
            Header = device.Name,
            Content = switches,
            HeaderIcon = new FontIcon { Glyph = DeviceGlyph(device) },
            IsClickEnabled = true,
            ActionIcon = new FontIcon { Glyph = "" },
        };
        if (Marked(line, mark) is { } described)
            card.Description = described;
        var drawn = (line, mark);
        refresh.Add(() =>
        {
            if (Device(device.Id) is not { } now)
                return;
            var said = SyncWords.DeviceLine(now, Now, ctx.Tv.Paired.Any(tv => tv.Name == now.Name));
            if (said == drawn)
                return;
            drawn = said;
            if (Marked(said.Item1, said.Item2) is { } again)
                card.Description = again;
            else
                card.ClearValue(SettingsCard.DescriptionProperty);
        });
        AutomationProperties.SetName(card, T.DevicesOpenName(device.Name));
        card.Click += (_, _) => SyncDialogs.DevicePage(Device(device.Id) ?? device);
        return card;
    }

    /// <summary>A card's line with its dot before it, in the colour of what it says.</summary>
    static FrameworkElement? Marked(string line, SyncMark mark)
    {
        if (line.Length == 0)
            return null;
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        var colour = mark switch
        {
            SyncMark.Water => Colours.Water,
            SyncMark.Amber => Colours.Amber,
            SyncMark.Vermilion => Ui.Vermilion,
            _ => Colours.Haze,
        };
        if (mark == SyncMark.Vermilion)
            row.Children.Add(new FontIcon { Glyph = "", FontSize = 14, Foreground = new SolidColorBrush(Ui.Vermilion), VerticalAlignment = VerticalAlignment.Top });
        else if (mark != SyncMark.None)
            row.Children.Add(Ui.Dot(colour, mark == SyncMark.Hollow));
        row.Children.Add(new TextBlock { Text = line, TextWrapping = TextWrapping.Wrap, Foreground = Res<Brush>("HazeBrush"), MaxWidth = 380 });
        return row;
    }

    void YourDevices(SyncStatus status)
    {
        Cards.Children.Add(Heading(T.DevicesYours));
        var others = status.Devices.Where(d => !d.ThisDevice).ToList();
        if (others.Count == 0)
            Cards.Children.Add(CardFrame(T.DevicesOnlyThis, T.DevicesOnlyThisLine, null, ""));
        foreach (var device in others)
            Cards.Children.Add(DeviceCard(device));
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var show = new Button { Content = T.PairShowCode };
        show.Click += (_, _) => SyncDialogs.PairShow(Origin.Devices);
        var enter = new Button { Content = T.DevicesEnterCode };
        enter.Click += (_, _) => SyncDialogs.PairEnter();
        buttons.Children.Add(show);
        buttons.Children.Add(enter);
        Cards.Children.Add(CardFrame(T.DevicesAdd, T.PairEnterLine, buttons, ""));
    }

    /// <summary>
    /// What the rules decided against someone's action: a watch kept although another device
    /// marked the episode unwatched, with its undo, for 7 days.
    /// </summary>
    void SyncNotes(SyncStatus status)
    {
        if (status.Notes.Length == 0)
            return;
        var missing = status.Notes.Select(n => n.AnilistId).Distinct().Where(id => !ctx.Known.Entries.ContainsKey(id)).ToArray();
        if (missing.Length > 0)
        {
            foreach (var id in missing)
                notesWaiting.Add(id);
            ctx.Send(new Command.LoadCards(missing));
        }
        Cards.Children.Add(Heading(T.SyncNotes));
        foreach (var note in status.Notes.OrderByDescending(n => n.At))
        {
            var title = ctx.Known.Entries.TryGetValue(note.AnilistId, out var entry) ? entry.Title : "";
            var undo = new Button { Content = T.SyncNoteUndo };
            undo.Click += (_, _) => SyncDialogs.UndoNote(note);
            var episode = long.TryParse(note.Episode, out var n) ? n : 0;
            Cards.Children.Add(CardFrame(T.SyncNoteKeptWatched(episode, title, note.UnwatchDevice, note.WatchDevice), null, undo, ""));
        }
        var foot = Ui.Text(T.SyncNotesFoot, "HazeCaptionStyle");
        foot.Margin = new Thickness(2, 4, 0, 0);
        Cards.Children.Add(foot);
    }

    void CloudFolder(SyncStatus status)
    {
        Cards.Children.Add(Heading(T.CloudGroup));
        var note = Ui.Text(T.CloudFoot, "HazeCaptionStyle");
        note.Margin = new Thickness(2, 0, 0, 4);
        Cards.Children.Add(note);
        if (status.Folder is not { } folder)
        {
            var add = new Button { Content = T.CloudAdd };
            add.Click += (_, _) => SyncDialogs.Picker();
            Cards.Children.Add(CardFrame(T.CloudAdd, null, add, ""));
            return;
        }
        var service = SyncWords.ServiceName(folder.Kind);
        var state = SyncWords.State(status, Now, SyncDialogs.ByHand);
        static (string, SyncMark) Line(CloudState state, SyncFolderView folder) => state switch
        {
            CloudState.Synced synced => (folder.Account is { } account ? $"{synced.Line} · {account}" : synced.Line, SyncMark.Water),
            CloudState.Syncing => (T.SyncSyncing, SyncMark.Water),
            CloudState.SignedOut => (T.CloudSignedOutLine(SyncWords.Kind), SyncMark.Hollow),
            CloudState.Problem problem => (problem.Line, SyncMark.Vermilion),
            _ => (T.CloudOffline(SyncWords.Kind), SyncMark.Amber),
        };
        var (line, mark) = Line(state, folder);
        Button? syncNow = null;
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        switch (state)
        {
            case CloudState.SignedOut or CloudState.Problem { Fix: ProblemFix.SignInAgain }:
                var again = state is CloudState.Problem;
                var signIn = new Button { Content = again ? T.CloudSignInAgain : T.CloudSignIn };
                if (again)
                    signIn.Style = Res<Style>("AccentButtonStyle");
                signIn.Click += (_, _) => SyncDialogs.SignInAgain(folder.Kind);
                buttons.Children.Add(signIn);
                break;
            case CloudState.Problem { Fix: ProblemFix.ChooseFolder }:
                var choose = new Button { Content = T.CloudChooseFolder, Style = Res<Style>("AccentButtonStyle") };
                choose.Click += (_, _) => SyncDialogs.Picker();
                buttons.Children.Add(choose);
                break;
            default:
                var now = syncNow = new Button { Content = T.CloudSyncNow, IsEnabled = !status.Syncing };
                now.Click += (_, _) => ctx.Send(new Command.SyncFolderNow());
                buttons.Children.Add(now);
                break;
        }
        buttons.Children.Add(MoreButton(folder.Kind, service));
        var card = new SettingsCard
        {
            Header = state is CloudState.Problem ? T.CloudErrTitle(service) : service,
            Content = buttons,
            HeaderIcon = new FontIcon { Glyph = "" },
        };
        if (Marked(line, mark) is { } described)
            card.Description = described;
        AutomationProperties.SetName(card, $"{card.Header}, {line}");
        Cards.Children.Add(card);
        // A pass starting or ending changes the line and Sync now in place.
        var drawn = (line, mark);
        refresh.Add(() =>
        {
            if (ctx.Known.SyncGroup is not { Folder: { } nowFolder } nowStatus)
                return;
            if (syncNow is not null && syncNow.IsEnabled == nowStatus.Syncing)
                syncNow.IsEnabled = !nowStatus.Syncing;
            var said = Line(SyncWords.State(nowStatus, Now, SyncDialogs.ByHand), nowFolder);
            if (said == drawn)
                return;
            drawn = said;
            if (Marked(said.Item1, said.Item2) is { } again)
                card.Description = again;
            AutomationProperties.SetName(card, $"{card.Header}, {said.Item1}");
        });
    }

    /// <summary>
    /// The cloud card's More button, a borderless glyph button with a flyout: Use another service,
    /// Sign out for a cloud drive and, apart below a separator, Remove cloud folder in the
    /// critical colour, which asks first.
    /// </summary>
    Button MoreButton(SyncFolderKind kind, string service)
    {
        var flyout = new MenuFlyout();
        var another = new MenuFlyoutItem { Text = T.CloudUseAnother };
        another.Click += (_, _) => SyncDialogs.PickerChange(kind);
        flyout.Items.Add(another);
        if (SyncWords.IsCloud(kind))
        {
            var signOut = new MenuFlyoutItem { Text = T.CloudSignOut };
            signOut.Click += (_, _) =>
            {
                SyncDialogs.SignedOutByHand();
                ctx.Send(new Command.SyncSignOut(kind));
            };
            flyout.Items.Add(signOut);
        }
        flyout.Items.Add(new MenuFlyoutSeparator());
        var remove = new MenuFlyoutItem { Text = T.CloudRemove, Foreground = Res<Brush>("SystemFillColorCriticalBrush") };
        remove.Click += (_, _) => SyncDialogs.RemoveFolder(kind);
        flyout.Items.Add(remove);
        var more = new Button
        {
            Content = new FontIcon { Glyph = "\uE712", FontSize = 16 },
            Width = 32,
            Height = 32,
            Padding = new Thickness(0),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            VerticalAlignment = VerticalAlignment.Center,
            Flyout = flyout,
        };
        SyncDialogs.MoreOpener = () => flyout.ShowAt(more);
        AutomationProperties.SetName(more, T.CloudMore(service));
        ToolTipService.SetToolTip(more, T.CloudMore(service));
        return more;
    }

    /// <summary>The recovery key (once a folder holds one), Make a new key, and Turn off sync last.</summary>
    void RecoveryAndOff(SyncStatus status)
    {
        if (status.RecoveryKey)
        {
            Cards.Children.Add(Heading(T.RkeyRow));
            var made = status.RecoveryKeyMade is { } at
                ? T.RkeyMade(DateTimeOffset.FromUnixTimeSeconds(at).ToLocalTime().ToString("d MMMM", System.Globalization.CultureInfo.CurrentCulture))
                : null;
            var show = new Button { Content = T.DevicesShowKey };
            show.Click += (_, _) => SyncDialogs.ShowKeyAgain();
            Cards.Children.Add(CardFrame(T.RkeyRow, made, show, ""));
            var fresh = new SettingsCard
            {
                Header = T.RkeyNew,
                Description = T.RkeyNewFoot,
                HeaderIcon = new FontIcon { Glyph = "" },
                IsClickEnabled = true,
            };
            fresh.Click += (_, _) => SyncDialogs.MakeNewKey();
            Cards.Children.Add(fresh);
        }
        else
        {
            Cards.Children.Add(new Border { Height = 16 });
        }
        var off = new Button { Content = T.DevicesTurnOffAction };
        off.Click += (_, _) => SyncDialogs.TurnOff();
        Cards.Children.Add(CardFrame(T.SyncTurnOff(SyncWords.Kind), null, off, ""));
    }

    /// <summary>The TVs paired for Play on TV, which work whether sync is on or off.</summary>
    void PairedTvs(bool withFoot)
    {
        if (!ctx.Tv.Available)
            return;
        Cards.Children.Add(Heading(T.TvPlayOn));
        tvGroup = new StackPanel { Spacing = 4 };
        tvsDrawn = null;
        ShowTvs(tvGroup);
        Cards.Children.Add(tvGroup);
        if (withFoot)
        {
            var foot = Ui.Text(T.DevicesPairedTvsFoot, "HazeCaptionStyle");
            foot.Margin = new Thickness(2, 4, 0, 0);
            Cards.Children.Add(foot);
        }
    }
}
