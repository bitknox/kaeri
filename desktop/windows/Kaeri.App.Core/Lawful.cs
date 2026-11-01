using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// The words for media servers, folders and licensed services. The engine sends states and
/// failures as data, so every line here is this app's own string.
/// </summary>
public static class Lawful
{
    /// <summary>A server error as a sentence. <paramref name="address"/> is what was typed, for an answer that names none.</summary>
    public static string Failure(ServerFailure failure, string address) => failure switch
    {
        ServerFailure.Unreachable u => T.ServersErrorUnreachable(u.Address is { Length: > 0 } ? u.Address : address),
        ServerFailure.NotJellyfin => T.ServersErrorNotJellyfin,
        ServerFailure.TooOld old => T.ServersErrorTooOld(old.Version),
        ServerFailure.Credentials => T.ServersErrorCredentials,
        ServerFailure.QuickConnectOff => T.ServersErrorQcOff,
        ServerFailure.QuickConnectExpired => T.ServersQcExpired,
        ServerFailure.PlainHttp => T.ServersErrorPlainHttp,
        ServerFailure.Certificate c => T.ServersCertBody(c.Host),
        ServerFailure.SignedOut => T.ServersRowSignedOut,
        ServerFailure.BadAddress => T.ServersErrorBadAddress,
        _ => T.ServersErrorOther,
    };

    /// <summary>The line under a server's name: signed out, matching while series are still being matched, or the summary.</summary>
    public static string ServerLine(ServerRow row, Known known, DateTimeOffset now)
    {
        if (row.SignedOutHere)
            return T.ServersRowSignedOutHere;
        if (row.SignedOut)
            return T.ServersRowSignedOut;
        if (row.NotAnswering && row.LastSeen is { } seen)
            return T.ServersRowNotAnswering(SeenAt(seen, now));
        return ServerMatching(row, known, now) ?? T.ServersRowSummary(row.Libraries.Count(l => l.Ticked), row.Series);
    }

    /// <summary>When a server last answered: the time today, the date and time before that.</summary>
    public static string SeenAt(long unixSeconds, DateTimeOffset now)
    {
        var at = DateTimeOffset.FromUnixTimeSeconds(unixSeconds).ToLocalTime();
        var today = now.ToLocalTime().Date == at.Date;
        return at.ToString(today ? "t" : "d MMM, t", System.Globalization.CultureInfo.CurrentCulture);
    }

    /// <summary>"Matching · 12 of 84 series", with the wait for AniList's limit after it, or null when nothing is matching.</summary>
    public static string? ServerMatching(ServerRow row, Known known, DateTimeOffset now) =>
        row.SignedOut || row.SignedOutHere ? null : row.Matching is { } progress
            ? Matching(progress, known.WaitingSeconds("s:" + row.Id, now))
            : null;

    /// <summary>The matching line: how far it is, and "Waiting for AniList · 12 s" after it while the limit holds it back.</summary>
    public static string Matching(MatchProgress progress, int? waiting)
    {
        var line = T.ServersRowMatching(progress.Done, progress.Total);
        return waiting is { } seconds ? $"{line} · {T.LibraryRowWaiting(seconds)}" : line;
    }

    /// <summary>A library's kind in its line.</summary>
    public static string LibraryKind(string? kind) => kind switch
    {
        "tvshows" => T.ServersKindShows,
        "movies" => T.ServersKindMovies,
        _ => T.ServersKindOther,
    };

    /// <summary>A library's line in the Choose libraries step: its kind and series.</summary>
    public static string LibraryLine(LibraryChoice library, int series) => T.ServersLibraryLine(LibraryKind(library.Kind), series);

        /// <summary>A folder's line: the drive being away, the scan counting or matching, or what it holds.</summary>
    public static string FolderLine(FolderRow row, Known known, DateTimeOffset now)
    {
        if (!row.Available)
            return T.LibraryRowDriveOffline;
        switch (row.Scan)
        {
            case FolderScan.Scanning scanning:
                return T.LibraryRowScanning(scanning.Files);
            case FolderScan.Matching matching:
                return Matching(matching.Progress, known.WaitingSeconds("f:" + row.Path, now));
        }
        var summary = T.LibraryRowSummary(T.LibraryEpisodes(row.Episodes), T.LibrarySeries(row.Series));
        return row.NeedsLook > 0 ? $"{summary} · {T.LibraryRowNeedsLook(row.NeedsLook)}" : summary;
    }

    /// <summary>Why a file group is not matched.</summary>
    public static string Reason(NotMatchedReason? reason) => reason switch
    {
        NotMatchedReason.Offline => T.LibraryRowDriveOffline,
        _ => T.LibraryReasonNoTitle,
    };

    /// <summary>The Licensed services row on Your sources.</summary>
    public static string LicensedLine(LicensedView view) =>
        view.Settings.Services.Length == 0
            ? T.LicensedRowNone
            : view.Region is { Length: > 0 } region
                ? T.LicensedRowLine(string.Join(", ", view.Settings.Services), RegionName(region))
                : string.Join(", ", view.Settings.Services);

    /// <summary>A country code as the system names it, or the code when it knows none.</summary>
    public static string RegionName(string code)
    {
        try
        {
            return new System.Globalization.RegionInfo(code).DisplayName;
        }
        catch (ArgumentException)
        {
            return code;
        }
    }
}
