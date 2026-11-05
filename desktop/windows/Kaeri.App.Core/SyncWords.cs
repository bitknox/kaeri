using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>A line's dot on the Devices pane: the colours the Linux app and the board name.</summary>
public enum SyncMark
{
    Water,
    Amber,
    Vermilion,
    Hollow,
    None,
}

/// <summary>What the cloud card offers when its folder has a problem.</summary>
public enum ProblemFix
{
    SignInAgain,
    ChooseFolder,
    Nothing,
}

/// <summary>What the cloud card says: the state and, for a problem, its line and its button.</summary>
public abstract record CloudState
{
    public sealed record Synced(string Line) : CloudState;
    public sealed record Syncing : CloudState;
    public sealed record SignedOut : CloudState;
    public sealed record Problem(string Line, ProblemFix Fix) : CloudState;
    public sealed record Offline : CloudState;
}

/// <summary>
/// The words of sync on the Devices pane and its dialogs, from what the engine last said
/// (<see cref="SyncStatus"/>). The engine holds every rule; this only says it.
/// </summary>
public static class SyncWords
{
    /// <summary>The word for this device in "Turn off sync on this PC".</summary>
    public static string Kind => T.DeviceKindPc;

    /// <summary>The service name a person reads.</summary>
    public static string ServiceName(SyncFolderKind kind) => kind switch
    {
        SyncFolderKind.Directory => T.CloudFolder(Kind),
        SyncFolderKind.WebDav => T.CloudWebdav,
        SyncFolderKind.Nextcloud => "Nextcloud", // i18n-skip
        SyncFolderKind.GoogleDrive => "Google Drive", // i18n-skip
        SyncFolderKind.Dropbox => "Dropbox", // i18n-skip
        _ => "OneDrive", // i18n-skip
    };

    /// <summary>A drive signed in to with a browser, which also has Sign out.</summary>
    public static bool IsCloud(SyncFolderKind kind) => kind is SyncFolderKind.GoogleDrive or SyncFolderKind.Dropbox or SyncFolderKind.OneDrive;

    /// <summary>What a device's card says: when it last synced, or that it has not been seen for a month.</summary>
    public static (string Line, SyncMark Mark) SeenLine(SyncDeviceView device, long now)
    {
        if (device.LastSeen is not { } seen)
            return ("", SyncMark.None);
        var days = (now - seen) / 86_400;
        return days >= 30
            ? (T.DevicesNotSeen(days), SyncMark.Amber)
            : (T.DevicesRowSynced(Wording.Ago(Math.Max(0, now - seen))), SyncMark.Water);
    }

    /// <summary>
    /// The card's line under a device's name: when it synced, and Remote when it is a TV and a
    /// device that drives TVs; only Remote for a TV paired for Play on TV that never synced.
    /// </summary>
    public static (string Line, SyncMark Mark) DeviceLine(SyncDeviceView device, long now, bool pairedTv)
    {
        var (line, mark) = SeenLine(device, now);
        if (device.Folder is null && pairedTv && device.LastSeen is null)
            return (T.DevicesRemoteOnly, SyncMark.None);
        if (device.RemoteKey is not null || pairedTv)
            line = line.Length == 0 ? T.DevicesRemoteSwitch : T.DevicesLineRemote(line);
        return (line, mark);
    }

    /// <summary>
    /// Sync is on but has nothing to sync with: no other device and no folder. That reads as off,
    /// with the two ways in, as pairing or adding a folder is what turns sync on.
    /// </summary>
    public static bool Bare(SyncStatus status) =>
        !status.Enabled || (status.Folder is null && status.Devices.All(d => d.ThisDevice));

    /// <summary>
    /// The cloud card's state; <paramref name="byHand"/> is that the person pressed Sign out, so a
    /// sign-in that is needed is theirs and no error.
    /// </summary>
    public static CloudState State(SyncStatus status, long now, bool byHand)
    {
        var service = status.Folder is { } f ? ServiceName(f.Kind) : "";
        switch (status.Problem)
        {
            case SyncProblem.SignInNeeded when byHand:
                return new CloudState.SignedOut();
            case SyncProblem.SignInNeeded:
                return new CloudState.Problem(T.CloudErrSignedOut(service, Kind), ProblemFix.SignInAgain);
            case SyncProblem.Unreachable when status.Folder?.Kind == SyncFolderKind.Directory:
                return new CloudState.Problem(T.CloudErrFolderGone, ProblemFix.ChooseFolder);
            case SyncProblem.Unreachable:
                return new CloudState.Offline();
            case not null:
                return new CloudState.Problem(status.Error ?? T.DevicesRowError, ProblemFix.Nothing);
        }
        if (status.Syncing)
            return new CloudState.Syncing();
        return new CloudState.Synced(T.DevicesRowSynced(status.LastSynced is { } at ? Wording.Ago(Math.Max(0, now - at)) : T.TimeJustNowInline));
    }

    /// <summary>
    /// The cloud drives this build offers, alphabetical, with those it cannot sign in to hidden
    /// (no client id, or no way here). A drive that needs a phone is a TV's affair and never
    /// listed on a PC. Nextcloud is always offered: it signs in at its own address.
    /// </summary>
    public static IReadOnlyList<(SyncFolderKind Kind, string Name)> Offered(SyncStatus status)
    {
        var offered = status.Clouds
            .Where(c => c.Way is not (SyncCloudWay.Unavailable or SyncCloudWay.DeviceCode))
            .Select(c => (c.Service, ServiceName(c.Service)))
            .Append((SyncFolderKind.Nextcloud, ServiceName(SyncFolderKind.Nextcloud)))
            .DistinctBy(c => c.Item1)
            .OrderBy(c => c.Item2, StringComparer.Ordinal)
            .ToList();
        return offered;
    }

    /// <summary>The groups of four of a recovery key, upper case, for two rows of three.</summary>
    public static string[] KeyGroups(string key)
    {
        var clean = key.Where(char.IsAsciiLetterOrDigit).Select(char.ToUpperInvariant).ToArray();
        return clean.Chunk(4).Select(c => new string(c)).ToArray();
    }

    /// <summary>A recovery key typed in is long enough to try: 24 letters and digits.</summary>
    public static bool KeyReady(string typed) => typed.Count(char.IsAsciiLetterOrDigit) >= 24;

    /// <summary>A pairing code as "042 917".</summary>
    public static string SpacedCode(string code) => code.Length == 6 ? $"{code[..3]} {code[3..]}" : code;

    /// <summary>The digits of a typed pairing code.</summary>
    public static string CodeDigits(string typed) => new(typed.Where(char.IsAsciiDigit).ToArray());

    /// <summary>A typed pairing code is whole: six digits.</summary>
    public static bool CodeReady(string typed) => CodeDigits(typed).Length == 6;

    /// <summary>"1:48" from seconds.</summary>
    public static string MinutesSeconds(long left)
    {
        left = Math.Max(0, left);
        return $"{left / 60}:{left % 60:00}";
    }

    /// <summary>The host of a typed server address, for the line that says it could not be reached.</summary>
    public static string HostOf(string typed)
    {
        var t = typed.Trim();
        var at = t.IndexOf("://", StringComparison.Ordinal);
        if (at >= 0)
            t = t[(at + 3)..];
        var slash = t.IndexOf('/');
        return slash >= 0 ? t[..slash] : t;
    }

    /// <summary>What a problem says in a dialog, for the service last chosen.</summary>
    public static string ProblemText(SyncProblem problem, SyncFolderKind? service, string host)
    {
        var name = service is { } s ? ServiceName(s) : "";
        return problem switch
        {
            SyncProblem.Unreachable => service switch
            {
                SyncFolderKind.Nextcloud => T.CloudErrNoServer(host),
                SyncFolderKind.WebDav => T.CloudErrWebdav,
                _ => T.CloudErrOffline(name),
            },
            SyncProblem.SignInExpired => T.CloudErrTimeout,
            SyncProblem.SignInDenied denied => T.CloudErrDenied(ServiceName(denied.Service)),
            SyncProblem.GroupExists => T.CloudErrOtherGroup,
            SyncProblem.WrongRecoveryKey => T.RkeyWrong,
            SyncProblem.Other other => other.V1,
            _ => T.DevicesRowError,
        };
    }

    /// <summary>
    /// The title and the line of a failed cloud sign-in. A refusal, a page that timed out, or any
    /// other failure before the folder (the details go to Messages) say the sign-in failed; one
    /// after the folder was reached says it could not be set up.
    /// </summary>
    public static (string Title, string Line) SignInError(SyncProblem problem, SyncFolderKind service, string text, bool finishing)
    {
        var name = ServiceName(service);
        var signIn = T.CloudErrSigninTitle(name);
        return problem switch
        {
            SyncProblem.SignInDenied => (signIn, T.CloudErrDeniedShort(name)),
            SyncProblem.SignInExpired => (signIn, T.CloudErrTimeout),
            SyncProblem.Unreachable => (signIn, T.CloudErrOffline(name)),
            _ when finishing => (T.CloudErrSetupTitle, T.CloudErrSetupOther(name)),
            _ => (signIn, T.CloudErrSigninOther(name)),
        };
    }

    /// <summary>The name a tracker sign-in goes by.</summary>
    static string SignInName(string id) => id switch
    {
        "anilist" => "AniList", // i18n-skip
        "mal" => "MyAnimeList", // i18n-skip
        "simkl" => "Simkl", // i18n-skip
        "trakt" => "Trakt", // i18n-skip
        _ => id,
    };

    /// <summary>What a device holds sign-ins for, as a list a person reads.</summary>
    public static string SignInNames(SyncDeviceView device) =>
        string.Join(", ", device.Trackers.Select(SignInName).Concat(device.SignIns.Select(ServiceName)));

    /// <summary>What Copy sign-ins asks the device for: its trackers and each cloud sign-in.</summary>
    public static SyncCopy[] CopyAll(SyncDeviceView device) =>
        [new SyncCopy.Trackers(), .. device.SignIns.Select(s => (SyncCopy)new SyncCopy.Cloud(s))];

    /// <summary>The device a pairing just added: the newest one that is not this device.</summary>
    public static SyncDeviceView? Newest(SyncStatus status) => status.Devices.LastOrDefault(d => !d.ThisDevice);

    /// <summary>The link a phone's camera reads from the code dialog's QR code.</summary>
    public static string PairLink(string code, string address) => $"kaeri://pair/{code}@{address}";
}
