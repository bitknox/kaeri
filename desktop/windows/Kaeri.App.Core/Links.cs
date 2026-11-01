namespace Kaeri.App;

/// <summary>
/// The <c>kaeri://</c> links the app is opened with, as the Mac answers them: a watch
/// party's invite, a plugin store's invite, as the https link it wraps, and the widgets' destinations, a title, an episode
/// to play, the Calendar, Settings › Accounts and a finished download.
/// </summary>
public abstract record Link
{
    public const string Scheme = "kaeri";

    /// <summary>A store's invite: the https link itself, <c>https://…/join/{code}#{key}</c>, with <c>?open</c> for an open link.</summary>
    public sealed record Invite(string Https) : Link;

    /// <summary>A magnet link or a torrent file the viewer opened, as the engine reads it (<c>OpenLink</c>): the link, or the file's path.</summary>
    public sealed record Opened(string Input) : Link;

    /// <summary>A title's page: <c>kaeri://title/900001</c>.</summary>
    public sealed record Title(uint Id) : Link;

    /// <summary>An episode playing, from its title's page: <c>kaeri://play/900001/7</c>.</summary>
    public sealed record Play(uint Id, uint Episode) : Link;

    /// <summary>The Calendar: <c>kaeri://calendar</c>.</summary>
    public sealed record Calendar : Link;

    /// <summary>Settings › Accounts: <c>kaeri://settings/accounts</c>.</summary>
    public sealed record Accounts : Link;

    /// <summary>A finished download playing, or Downloads while it is not there: <c>kaeri://download/42</c>.</summary>
    public sealed record Download(long Id) : Link;

    /// <summary>
    /// A tracker's sign-in coming back from the browser, <c>kaeri://auth/anilist#access_token=…</c>:
    /// the tracker, and the whole link for the engine to finish with. A secret, as it can carry the token.
    /// </summary>
    public sealed record Auth(string Tracker, string Callback) : Link
    {
        public override string ToString() => $"Auth {{ Tracker = {Tracker} }}";
    }

    /// <summary>
    /// A watch party's invite, <c>kaeri://party?server=…&amp;room=…&amp;title=…&amp;episode=…</c>, as
    /// it came: the engine reads it (<c>parse_party_invite</c>), so every app agrees on what it says.
    /// </summary>
    public sealed record Party(string Text) : Link;

    /// <summary>
    /// What <paramref name="text"/> opens, or null when it is not a link the app answers. A store
    /// invite comes as <c>kaeri://join?link=&lt;the https link&gt;</c>, or as the https link
    /// with the app's scheme in front of the host, <c>kaeri://plugins.example/join/{code}#{key}</c>.
    /// A sign-in coming back is <c>kaeri://auth/&lt;tracker&gt;</c> with what the tracker sent.
    /// </summary>
    public static Link? Parse(string? text)
    {
        text = text?.Trim().Trim('"');
        if (string.IsNullOrEmpty(text))
            return null;
        if (text.StartsWith("magnet:", StringComparison.OrdinalIgnoreCase) || text.EndsWith(".torrent", StringComparison.OrdinalIgnoreCase))
            return new Opened(text);
        if (text.StartsWith("https://", StringComparison.OrdinalIgnoreCase))
            return InviteIn(text) is { } https ? new Invite(https) : null;
        var prefix = Scheme + "://";
        if (!text.StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
            return null;
        var rest = text[prefix.Length..];
        var hostEnd = rest.IndexOfAny(['/', '?', '#']);
        var host = (hostEnd < 0 ? rest : rest[..hostEnd]).ToLowerInvariant();
        var tail = hostEnd < 0 ? "" : rest[hostEnd..];
        var path = tail.Split(['?', '#'], 2)[0];
        var parts = path.Split('/', StringSplitOptions.RemoveEmptyEntries);
        switch (host)
        {
            case "join":
                return Query(tail, "link") is { } wrapped && InviteIn(wrapped) is { } inner ? new Invite(inner) : null;
            case "auth":
                return parts is [var tracker, ..] && tracker.All(c => char.IsAsciiLetterOrDigit(c) || c == '-')
                    ? new Auth(tracker.ToLowerInvariant(), text)
                    : null;
            case "party":
                return new Party(text);
            case "title" when parts is [var id] && uint.TryParse(id, out var t) && t > 0:
                return new Title(t);
            case "play" when parts is [var id, var ep] && uint.TryParse(id, out var p) && p > 0
                                                       && uint.TryParse(ep, out var e) && e > 0:
                return new Play(p, e);
            case "calendar" when parts.Length == 0:
                return new Calendar();
            case "settings" when parts is ["accounts"]:
                return new Accounts();
            case "download" when parts is [var id] && long.TryParse(id, out var d) && d > 0:
                return new Download(d);
            case "title" or "play" or "calendar" or "settings" or "download" or "":
                return null;
        }
        // The store's https link with kaeri:// in front.
        return InviteIn("https://" + rest) is { } invite ? new Invite(invite) : null;
    }

    /// <summary>
    /// The link a launch carries: the first argument with the app's scheme, as the installer's
    /// <c>kaeri.exe "%1"</c> hands it over, whole or as a command line.
    /// </summary>
    public static string? InArguments(IEnumerable<string> args) =>
        args.Select(a => a.Trim().Trim('"'))
            .FirstOrDefault(a => a.StartsWith(Scheme + ":", StringComparison.OrdinalIgnoreCase));

    /// <summary>The same, from a whole command line, as a redirected launch brings it.</summary>
    public static string? InCommandLine(string? line)
    {
        if (string.IsNullOrEmpty(line))
            return null;
        // A torrent file, from its association: the path, quoted when it has spaces.
        var trimmed = line.Trim().Trim('"');
        if (trimmed.EndsWith(".torrent", StringComparison.OrdinalIgnoreCase))
            return trimmed;
        var at = line.IndexOf(Scheme + "://", StringComparison.OrdinalIgnoreCase);
        if (at < 0)
            at = line.IndexOf("magnet:", StringComparison.OrdinalIgnoreCase);
        if (at < 0)
            return null;
        var quoted = at > 0 && line[at - 1] == '"';
        var end = quoted ? line.IndexOf('"', at) : line.IndexOf(' ', at);
        return end < 0 ? line[at..] : line[at..end];
    }

    /// <summary>The https invite <paramref name="text"/> is, as the store makes it: a path with <c>/join/</c> in it.</summary>
    static string? InviteIn(string text) =>
        Uri.TryCreate(text, UriKind.Absolute, out var uri) && uri.Scheme == Uri.UriSchemeHttps
            && uri.AbsolutePath.Contains("/join/", StringComparison.Ordinal) && uri.Host.Length > 0
            ? text
            : null;

    static string? Query(string tail, string name)
    {
        var query = tail.Split('?', 2) is [_, var q] ? q.Split('#', 2)[0] : "";
        foreach (var pair in query.Split('&', StringSplitOptions.RemoveEmptyEntries))
        {
            var (key, value) = pair.Split('=', 2) is [var k, var v] ? (k, v) : (pair, "");
            if (key == name)
                return Uri.UnescapeDataString(value.Replace('+', ' '));
        }
        return null;
    }
}
