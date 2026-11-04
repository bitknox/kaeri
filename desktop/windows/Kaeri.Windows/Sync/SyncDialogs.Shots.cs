#if KAERI_TESTING
using Kaeri.App;
using Kaeri.Engine.Model;

namespace Kaeri.Windows.Sync;

/// <summary>
/// The states of the sync screens for the shot run (<c>--show=sync-*</c>, with
/// <c>--page=settings --pane=devices</c>): each puts made-up status into the window the way the
/// engine would say it, and opens the dialog of that state. Made-up names only.
/// </summary>
static partial class SyncDialogs
{
    const string Phone = "Mika's phone";

    static long Clock => DateTimeOffset.UtcNow.ToUnixTimeSeconds();

    static SyncCloudView[] Clouds() =>
    [
        new(SyncFolderKind.GoogleDrive, new SyncCloudWay.Browser(), false),
        new(SyncFolderKind.Dropbox, new SyncCloudWay.Browser(), false),
        new(SyncFolderKind.OneDrive, new SyncCloudWay.Unavailable(CloudUnavailable.NoClientId), false),
    ];

    static SyncDeviceView Device(string id, string name, bool self, long ago, SyncDeviceKind kind = SyncDeviceKind.Computer) =>
        new(id, name, self, Clock - ago, [], [], SyncFolderKind.GoogleDrive, [], kind);

    static SyncDeviceView WithSignIns(SyncDeviceView d) => d with { Trackers = ["anilist", "mal"], SignIns = [SyncFolderKind.GoogleDrive] };

    static SyncStatus Off() => new(false, false, "Study PC", null, [], null, null, [], [], Clouds());

    static SyncStatus On() => new(
        true, false, "Study PC",
        new SyncFolderView(SyncFolderKind.GoogleDrive, "Google Drive", "mika@example.invalid"), // i18n-skip
        [
            Device("a", "Study PC", true, 0),
            WithSignIns(Device("b", Phone, false, 30, SyncDeviceKind.Phone)),
            Device("d", "Living room TV", false, 120, SyncDeviceKind.Tv) with { RemoteKey = "0a1b2c3d4e5f60718293a4b5c6d7e8f9" },
        ],
        Clock - 240, null, [], [], Clouds(), null, SyncFolderKind.GoogleDrive, true, Clock - 2 * 86_400);

    /// <summary>Every kind of card Devices can have: a held-back tablet, a month-old laptop, a kept watch.</summary>
    static SyncStatus Full()
    {
        var on = On();
        return on with
        {
            Devices =
            [
                .. on.Devices,
                Device("c", "Living room tablet", false, 3 * 3600, SyncDeviceKind.Tablet) with { Paused = true },
                Device("e", "Travel laptop", false, 41 * 86_400),
            ],
            Notes = [new SyncNoteView(1, 900_001, "5", "Living room TV", Phone, Clock - 3600)],
        };
    }

    static void Say(SyncStatus status) => Ctx.Inject(new Event.SyncStatus(status));

    /// <summary>Puts the window in the state <paramref name="show"/> names.</summary>
    public static void Stage(string show)
    {
        Ctx.Window.DispatcherQueue.TryEnqueue(async () =>
        {
            await Task.Delay(1500);
            Say(show switch
            {
                "sync-off" or "sync-road" or "sync-picker" or "sync-picker-error" or "sync-wait" or "sync-nextcloud" or "sync-webdav"
                    or "sync-restore" or "sync-restore-wrong" or "sync-motion" => Off(),
                "sync-devices-full" or "sync-device-page" or "sync-device-page-remote-off" or "sync-device-page-phone" or "sync-notes" => Full(),
                "sync-syncing" => On() with { Syncing = true },
                "sync-setting" => On(),
                "sync-problem" => On() with { Problem = new SyncProblem.SignInNeeded(SyncFolderKind.GoogleDrive) },
                "sync-signedout" => On() with { Problem = new SyncProblem.SignInNeeded(SyncFolderKind.GoogleDrive) },
                "sync-offline" => On() with { Problem = new SyncProblem.Unreachable(), Error = "offline" }, // i18n-skip
                "sync-none" => On() with { Folder = null, RecoveryKey = false, RecoveryKeyMade = null },
                _ => On(),
            });
            if (show == "sync-setting")
            {
                SettingUp = true;
                Changed?.Invoke();
            }
            if (show == "sync-signedout")
            {
                ByHand = true;
                Changed?.Invoke();
            }
            await Task.Delay(2500);
            Open(show);
        });
    }

    static async void Open(string show)
    {
        const string url = "https://auth.example/authorize";
        switch (show)
        {
            case "sync-road":
                Road();
                break;
            case "sync-picker":
                Picker();
                break;
            case "sync-picker-error":
                last = SyncFolderKind.Dropbox;
                Picker();
                Problem(new SyncProblem.Unreachable());
                break;
            case "sync-wait":
                SetWait(PresentPicker("list"), SyncFolderKind.GoogleDrive, url);
                break;
            case "sync-nextcloud":
                PresentPicker("nextcloud");
                break;
            case "sync-webdav":
                PresentPicker("webdav");
                break;
            case "sync-restore":
                Restore(SyncFolderKind.GoogleDrive);
                break;
            case "sync-restore-wrong":
                Restore(SyncFolderKind.GoogleDrive);
                if (restore is { } r)
                    SayError(r.Error, T.RkeyWrong);
                break;
            case "sync-key":
                Ctx.Known.RecoveryKey = "K7QF2M9XHD4RTB6WPN3ZC8JA";
                KeyDialog();
                break;
            case "sync-pair-show":
                Ctx.Known.PairInvite = ("482913", 47_812, Clock + 108);
                PresentPairShow(Origin.Devices);
                break;
            case "sync-pair-paired":
                Ctx.Known.PairInvite = ("482913", 47_812, Clock + 108);
                PresentPairShow(Origin.Devices);
                await Task.Delay(400);
                Paired(Phone);
                break;
            case "sync-pair-motion":
                Ctx.Known.PairInvite = ("482913", 47_812, Clock + 108);
                PresentPairShow(Origin.Devices);
                await Task.Delay(4000);
                Paired(Phone);
                break;
            case "sync-pair-enter":
                PairEnter();
                break;
            case "sync-pair-enter-address":
                PairEnter();
                await Task.Delay(400);
                enter?.ByAddress?.Invoke();
                break;
            case "sync-pair-enter-typed":
                PairEnter();
                await Task.Delay(400);
                Type("04291");
                break;
            case "sync-pair-enter-error" or "sync-pair-enter-wrong":
                PairEnter();
                await Task.Delay(400);
                Type("042917");
                if (enter is { } e)
                    PairFailed(e, show == "sync-pair-enter-wrong" ? T.PairErrCode(Phone) : T.PairErrNotFound);
                break;
            case "sync-pair-enter-motion":
                PairEnter();
                await Task.Delay(1500);
                Type("042917");
                await Task.Delay(1500);
                if (enter is { } working)
                {
                    working.Working = true;
                    working.Dialog.PrimaryButtonText = T.PairWorkingLine;
                    working.Dialog.IsPrimaryButtonEnabled = false;
                }
                await Task.Delay(1500);
                Paired(Phone);
                break;
            case "sync-device-page":
                DevicePage(Full().Devices.First(d => d.Id == "d"));
                break;
            case "sync-device-page-remote-off":
                DevicePage(Full().Devices.First(d => d.Id == "d") with { RemoteOff = true });
                break;
            case "sync-device-page-phone":
                DevicePage(Full().Devices.First(d => d.Id == "b"));
                break;
            case "sync-turn-off":
                TurnOff();
                break;
            case "sync-motion":
                // The list, Nextcloud's form (the title crossfades with the page), the list again,
                // Google Drive's wait, then the sign-in comes back and the folder is set up.
                Picker();
                await Task.Delay(2000);
                PresentPicker("nextcloud");
                await Task.Delay(2000);
                PresentPicker("list");
                await Task.Delay(1500);
                if (picker is { } p)
                    SetWait(p, SyncFolderKind.GoogleDrive, url);
                await Task.Delay(2500);
                picker?.Dialog.Hide();
                Say(On());
                break;
        }
    }

    /// <summary>Types a code into the open Enter a code dialog's boxes, as a person would.</summary>
    static void Type(string digits)
    {
        if (enter?.Dialog.Content is Microsoft.UI.Xaml.Controls.Border { Child: Microsoft.UI.Xaml.Controls.Grid grid })
            foreach (var box in Descendants(grid).OfType<Microsoft.UI.Xaml.Controls.TextBox>().Where(b => b.MaxLength == 1).Zip(digits))
                box.First.Text = box.Second.ToString();
    }

    static IEnumerable<Microsoft.UI.Xaml.DependencyObject> Descendants(Microsoft.UI.Xaml.DependencyObject root)
    {
        if (root is Microsoft.UI.Xaml.Controls.Panel panel)
            foreach (var child in panel.Children)
            {
                yield return child;
                foreach (var below in Descendants(child))
                    yield return below;
            }
        else if (root is Microsoft.UI.Xaml.Controls.Border { Child: { } inner })
        {
            yield return inner;
            foreach (var below in Descendants(inner))
                yield return below;
        }
    }
}
#endif
