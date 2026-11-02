using Kaeri.Engine.Core;
using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// The optional torrent proxy: the words for it, the form's draft, and the commands it sends.
/// Torrents connect directly until a proxy is saved, and a proxy is saved only once its check
/// passed. The Mac's and Linux's torrent setup, in the same words.
/// </summary>
public static class TorrentWords
{
    public static string Exposed => T.TorrentsRisk;
    public static string CheckNote => T.TorrentsCheckNote;
    public static string OffNote => T.TorrentsOffDetail;

    /// <summary>Edit proxy's line over its form.</summary>
    public static string EditNote => T.TorrentsProxyKeepsSaved;
    /// <summary>Under the password, wherever a saved proxy's details are open again.</summary>
    public static string PasswordKept => T.TorrentsPasswordKeep;
    /// <summary>While a proxy is checked before it is saved.</summary>
    public static string CheckingProxy => T.TorrentsCheckingProxy;
    /// <summary>Beside the proxy switch once a proxy passed its check and was saved.</summary>
    public static string ProxySaved => T.TorrentsProxySaved;

    /// <summary>Why a proxy was not saved: its check failed, and the saved proxy stays.</summary>
    public static string NotSaved() => T.TorrentsProxyNotSaved(T.TorrentsCheckNotMadeProxy);

    /// <summary>Where torrent traffic goes now, under the switch while torrents are on.</summary>
    public static string OnNote(TorrentView view) => view.Proxy is { } proxy
        ? T.TorrentsSummaryProxyAt(proxy.Host, proxy.Port.ToString(System.Globalization.CultureInfo.InvariantCulture))
        : T.TorrentsSummaryDirect;

    /// <summary>
    /// The proxy check's last word: the line under Proxy check, the word beside it, and
    /// whether it failed. <paramref name="ago"/> is how many seconds since it was said.
    /// </summary>
    public static (string? Note, string Word, bool Failed) Check(ProxyStatus? status, long? ago) => status switch
    {
        ProxyStatus.Passed => (ago is { } s ? Wording.Ago(s) : null, T.TorrentsCheckPassed, false),
        ProxyStatus.Checking => (null, T.TorrentsCheckRunning, false),
        ProxyStatus.Failed => (T.TorrentsCheckFailedLine, T.TorrentsCheckFailed, true),
        _ => (T.TorrentsCheckWaitingLine, T.TorrentsCheckWaiting, false),
    };
}

/// <summary>
/// The proxy form's fields before they are sent. A password left empty keeps the one kept.
/// </summary>
public sealed record TorrentDraft(string Host, string Port, string User, string Password)
{
    /// <summary>The draft the form opens on: the saved proxy's fields, or empty ones.</summary>
    public static TorrentDraft From(SettingsView? settings)
    {
        var proxy = settings?.Torrent.Proxy;
        return new(
            proxy?.Host ?? "",
            proxy is { } p ? p.Port.ToString(System.Globalization.CultureInfo.InvariantCulture) : "",
            proxy?.User ?? "",
            "");
    }

    /// <summary>The proxy as the engine takes it, when the fields make one.</summary>
    public TorrentProxy? Proxy =>
        Host.Trim() is { Length: > 0 } host
            && ushort.TryParse(Port.Trim(), out var port) && port > 0
            ? new TorrentProxy(host, port, User.Trim() is { Length: > 0 } user ? user : null, Password.Length > 0 ? Password : null)
            : null;

    /// <summary>Whether it can be sent: a proxy needs its host and port.</summary>
    public bool Complete => Proxy is not null;
}

public static class Torrents
{
    /// <summary>Saves <paramref name="proxy"/>, or with none removes the proxy so torrents connect directly.</summary>
    public static Command SetProxy(TorrentProxy? proxy) => new Command.SetTorrentProxy(proxy);

    /// <summary>The proxy check run again now, on the saved proxy.</summary>
    public static Command CheckNow() => new Command.CheckProxy();
}

/// <summary>A proxy to check before it is saved.</summary>
public sealed record TrialAsk(TorrentProxy Proxy);

/// <summary>
/// Checks before anything is saved, one at a time. The engine answers each
/// <see cref="Command.CheckTorrentProxy"/> with an unnumbered
/// <see cref="Event.ProxyChecked"/>, each from its own task, so two in flight could answer
/// in either order: a check is only sent once the one before it has answered, and an answer
/// goes to the ask that was sent, never to one made since. Asking again replaces what waits;
/// <see cref="Drop"/> forgets the ask, its answer then going nowhere.
/// </summary>
public sealed class ProxyTrials(Action<Command> send)
{
    int outstanding;
    TrialAsk? queued, sent;
    Action<TrialAsk, ProxyStatus>? answer;

    /// <summary>Whether an ask waits for its verdict.</summary>
    public bool Checking => queued is not null || sent is not null;

    /// <summary>Checks <paramref name="ask"/>, and calls <paramref name="answered"/> with it and its verdict.</summary>
    public void Ask(TrialAsk ask, Action<TrialAsk, ProxyStatus> answered)
    {
        answer = answered;
        sent = null;
        queued = ask;
        Pump();
    }

    /// <summary>Done waiting: the form closed. A verdict still to come is dropped.</summary>
    public void Drop()
    {
        queued = null;
        sent = null;
        answer = null;
    }

    /// <summary>The engine's verdict on the check it was sent last.</summary>
    public void Heard(ProxyStatus status)
    {
        if (outstanding > 0)
            outstanding--;
        if (sent is { } mine && outstanding == 0)
        {
            var answered = answer;
            sent = null;
            answer = null;
            answered?.Invoke(mine, status);
        }
        Pump();
    }

    void Pump()
    {
        if (queued is not { } next || outstanding > 0)
            return;
        queued = null;
        sent = next;
        outstanding++;
        send(new Command.CheckTorrentProxy(next.Proxy));
    }
}
