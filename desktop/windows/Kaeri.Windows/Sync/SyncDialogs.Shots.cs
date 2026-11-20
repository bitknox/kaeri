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

    /// <summary>Another device started sync over in Google Drive: no key opens the folder now.</summary>
    static SyncStatus StartedOver() => On() with
    {
        Problem = new SyncProblem.GroupReplaced(),
        Error = T.CloudErrStartedOver("Google Drive"), // i18n-skip
        RecoveryKey = false,
        RecoveryKeyMade = null,
    };

    static void Say(SyncStatus status) => Ctx.Inject(new Event.SyncStatus(status));

    /// <summary>Puts the window in the state <paramref name="show"/> names.</summary>
    public static void Stage(string show)
    {
        if (show.EndsWith("-reduced", StringComparison.Ordinal))
            Motion.Override = false;
        Ctx.Window.DispatcherQueue.TryEnqueue(async () =>
        {
            await Task.Delay(1500);
            Say(show switch
            {
                "sync-off" or "sync-picker" or "sync-picker-error" or "sync-wait" or "sync-connecting" or "sync-setting-up" or "sync-key-setup"
                    or "sync-wait-denied" or "sync-wait-expired" or "sync-wait-full" or "sync-nextcloud" or "sync-webdav"
                    or "sync-restore" or "sync-restore-wrong" or "sync-motion" => Off(),
                // The folder holds another group of two devices, as the engine counts them.
                "sync-startover" or "sync-startover-wrong" or "sync-startover-confirm" or "sync-startover-clearing" or "sync-startover-setup"
                    or "sync-startover-key" or "sync-startover-offline" or "sync-startover-refused" or "sync-startover-motion"
                    or "sync-startover-cancel-motion" => Off() with { RestoreDevices = 2 },
                // Another device started sync over in Google Drive: no key opens the folder now.
                "sync-startedover" or "sync-startedover-restore" => StartedOver(),
                "sync-devices-full" or "sync-device-page" or "sync-device-page-remote-off" or "sync-device-page-phone" or "sync-notes" => Full(),
                "sync-syncing" => On() with { Syncing = true },
                "sync-problem" => On() with { Problem = new SyncProblem.SignInNeeded(SyncFolderKind.GoogleDrive) },
                "sync-signedout" => On() with { Problem = new SyncProblem.SignInNeeded(SyncFolderKind.GoogleDrive) },
                "sync-offline" => On() with { Problem = new SyncProblem.Unreachable(), Error = "offline" }, // i18n-skip
                "sync-none" => On() with { Folder = null, RecoveryKey = false, RecoveryKeyMade = null },
                _ => On(),
            });
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
            case "sync-picker":
                Picker();
                break;
            case "sync-switch":
                PickerChange(SyncFolderKind.GoogleDrive);
                break;
            case "sync-more":
                MoreOpener?.Invoke();
                break;
            case "sync-remove":
                RemoveFolder(SyncFolderKind.GoogleDrive);
                break;
            case "sync-picker-error":
                last = SyncFolderKind.Dropbox;
                Picker();
                Problem(new SyncProblem.Unreachable());
                break;
            case "sync-wait" or "sync-connecting" or "sync-setting-up" or "sync-key-setup" or "sync-wait-denied" or "sync-wait-expired" or "sync-wait-full":
                ShowState(show, SyncFolderKind.GoogleDrive, url);
                break;
            case "sync-nextcloud":
                PresentPicker("nextcloud", null);
                break;
            case "sync-webdav":
                PresentPicker("webdav", null);
                break;
            case "sync-restore":
                Restore(SyncFolderKind.GoogleDrive);
                break;
            case "sync-restore-wrong":
                Restore(SyncFolderKind.GoogleDrive);
                if (restore is { } r)
                    SayError(r.Error, T.RkeyWrong);
                break;
            case "sync-startover":
                Restore(SyncFolderKind.GoogleDrive);
                break;
            case "sync-startover-wrong":
                Restore(SyncFolderKind.GoogleDrive);
                if (restore is { } wrong)
                {
                    wrong.Entry.Text = "K7QM 4XR9 DH2V T8NB 3CWZ 6PLE";
                    SayError(wrong.Error, T.RkeyWrong);
                }
                break;
            case "sync-startover-confirm":
                Restore(SyncFolderKind.GoogleDrive);
                await Task.Delay(400);
                if (restore is { } confirming)
                    AskStartOver(confirming);
                break;
            case "sync-startover-clearing" or "sync-startover-setup" or "sync-startover-key" or "sync-startover-offline" or "sync-startover-refused":
                StartOverAt(show switch
                {
                    "sync-startover-clearing" => [Clearing()],
                    "sync-startover-setup" => [Clearing(), Finishing()],
                    "sync-startover-key" => [Clearing(), Finishing(), new Event.SyncRecoveryKey("K7QM4XR9TB2HWN6CP3ZD8FLA")],
                    "sync-startover-offline" => [Clearing(), new Event.SyncProblem(new SyncProblem.Unreachable())],
                    _ => [Clearing(), new Event.SyncProblem(new SyncProblem.ClearRefused())],
                });
                break;
            case "sync-startover-motion":
                // The key field, the confirmation in its place, then the work at the fixture's
                // pace (1.5 s a step) and the key; each through the dialog's own pacing.
                Restore(SyncFolderKind.GoogleDrive);
                await Task.Delay(2000);
                if (restore is not { } moving)
                    break;
                AskStartOver(moving);
                await Task.Delay(2000);
                var flow = startOver = new StartOver(SyncFolderKind.GoogleDrive);
                moving.Pressed = DateTime.UtcNow;
                RestoreButtons(moving);
                StageStartOver(moving, flow);
                foreach (var (wait, said) in new (int, Event)[] { (100, Clearing()), (1500, Finishing()), (1500, new Event.SyncRecoveryKey("K7QM4XR9TB2HWN6CP3ZD8FLA")) })
                {
                    await Task.Delay(wait);
                    Ctx.Inject(said);
                }
                break;
            case "sync-startedover-restore":
                EnterNewKey(SyncFolderKind.GoogleDrive);
                break;
            case "sync-startedover-motion" or "sync-startedover-motion-reduced":
                // Synced, then the pass finds the group started over elsewhere: the cloud card
                // turns into the error and the Recovery key rows go.
                await Task.Delay(2000);
                Say(StartedOver());
                break;
            case "sync-startover-cancel-motion":
                // The failure page, then its Cancel pressed as a person would.
                StartOverAt([Clearing(), new Event.SyncProblem(new SyncProblem.Unreachable())]);
                await Task.Delay(3000);
                if (restore is { } failed && DialogButton(failed.Dialog, "CloseButton") is { } cancel)
                    new Microsoft.UI.Xaml.Automation.Peers.ButtonAutomationPeer(cancel).Invoke();
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
                PresentPicker("nextcloud", null);
                await Task.Delay(2000);
                PresentPicker("list", null);
                await Task.Delay(1500);
                // Google Drive chosen: the wait, the return, the folder being made, the key.
                foreach (var state in new[] { "sync-wait", "sync-connecting", "sync-setting-up", "sync-key-setup" })
                {
                    ShowState(state, SyncFolderKind.GoogleDrive, url);
                    await Task.Delay(state == "sync-wait" ? 2500 : 1500);
                }
                break;
        }
    }

    /// <summary>
    /// Drives the fixture engine through a start over as a person would, for the real flow's
    /// shots: with --scene=sync-other-group (or its -offline and -refused forms) and
    /// --show=startover-run, Add a cloud folder, Google Drive, the restore dialog the folder's
    /// other group opens, Start over without the key, then Delete and start over. With
    /// --scene=sync-started-over and --show=startover-enter-new, Enter new key. --show=startover-look
    /// does nothing, and only keeps other dialogs out of the shot.
    /// </summary>
    public static void Drive(string show)
    {
        if (show == "startover-look")
            return;
        Ctx.Window.DispatcherQueue.TryEnqueue(async () =>
        {
            await Task.Delay(3000);
            if (show == "startover-enter-new")
            {
                if (Ctx.Known.SyncGroup?.Folder is { } folder)
                    EnterNewKey(folder.Kind);
                return;
            }
            Picker();
            await Task.Delay(1000);
            Choose(SyncFolderKind.GoogleDrive);
            for (var i = 0; i < 100 && restore is null; i++)
                await Task.Delay(100);
            if (restore is not { } r)
                return;
            await Task.Delay(1500);
            AskStartOver(r);
            await Task.Delay(1500);
            BeginStartOver(r);
        });
    }

    static Event Clearing() => new Event.SyncSignIn(SyncFolderKind.GoogleDrive, new SyncSignInStep.Clearing());

    static Event Finishing() => new Event.SyncSignIn(SyncFolderKind.GoogleDrive, new SyncSignInStep.Finishing());

    /// <summary>The restore dialog with a start over that heard <paramref name="said"/>, shown at once.</summary>
    static void StartOverAt(Event[] said)
    {
        var flow = startOver = new StartOver(SyncFolderKind.GoogleDrive);
        foreach (var e in said)
        {
            Ctx.Known.Take(e);
            flow.Take(e);
        }
        var r = PresentRestore(SyncFolderKind.GoogleDrive, flow);
        r.Shown = DateTime.UtcNow;
    }

    /// <summary>The picker's wait in one of its states, at once and without the return's pacing.</summary>
    static void ShowState(string show, SyncFolderKind service, string url)
    {
        var p = PresentPicker("wait", null);
        var name = SyncWords.ServiceName(service);
        (Phase, (string, string)?) pick = show switch
        {
            "sync-connecting" => (Phase.Connecting, null),
            "sync-setting-up" => (Phase.SettingUp, null),
            "sync-key-setup" => (Phase.Key, null),
            "sync-wait-denied" => (Phase.Error, (T.CloudErrSigninTitle(name), T.CloudErrDeniedShort(name))),
            "sync-wait-expired" => (Phase.Error, (T.CloudErrSigninTitle(name), T.CloudErrTimeout)),
            "sync-wait-full" => (Phase.Error, (T.CloudErrSetupTitle, T.CloudErrFull(name))),
            _ => (Phase.Browser, null),
        };
        var (stage, error) = pick;
        if (stage == Phase.Key)
            FillKey(p.KeySlot, "K7QM4XR9TB2HWN6CP3ZD8FLA");
        ShowStage(p, service, stage, stage == Phase.Browser ? url : null, error);
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
