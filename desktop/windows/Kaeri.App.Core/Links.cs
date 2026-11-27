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
    /// A room's invite, <c>kaeri://room#…</c> or <c>https://kaeri.app/room#…</c>, as it came (an old
    /// <c>kaeri://party</c> link too): the engine reads it (<c>parse_room_target</c>), so every app agrees on it.
    /// </summary>
    public sealed record Room(string Text) : Link;

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
            return RoomIn(text) ? new Room(text) : InviteIn(text) is { } https ? new Invite(https) : null;
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
            case "room" or "party":
                return new Room(text);
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
    /// The link a launch carries: the first argument the app opens, as the installer's
    /// <c>kaeri.exe "%1"</c> hands over a <c>kaeri:</c> link, a magnet link or a torrent file.
    /// </summary>
    public static string? InArguments(IEnumerable<string> args) =>
        args.Select(a => a.Trim().Trim('"')).FirstOrDefault(Launched);

    /// <summary>
    /// The same, from a whole command line, as a redirected launch brings it: split the way
    /// Windows splits a program's command line, so a quoted path with spaces stays one argument.
    /// The program's own path, first, is never one the app opens.
    /// </summary>
    public static string? InCommandLine(string? line) =>
        string.IsNullOrWhiteSpace(line) ? null : InArguments(Arguments(line));

    /// <summary>
    /// Whether a launch's argument is one the app opens: a link with the app's scheme, a magnet
    /// link or a torrent file's path, the three the installer registers. A switch is never one.
    /// </summary>
    public static bool Launched(string arg)
    {
        arg = arg.Trim().Trim('"');
        return !arg.StartsWith('-')
            && (arg.StartsWith(Scheme + ":", StringComparison.OrdinalIgnoreCase)
                || arg.StartsWith("magnet:", StringComparison.OrdinalIgnoreCase)
                || arg.EndsWith(".torrent", StringComparison.OrdinalIgnoreCase));
    }

    /// <summary>
    /// A command line's arguments, read as Windows programs read theirs: spaces and tabs part
    /// them outside quotes, a quote opens or closes a quoted part, and backslashes count only
    /// before a quote, where each pair stands for one backslash and an odd one out keeps the
    /// quote as text. The program's path comes first, read with its quotes alone.
    /// </summary>
    static List<string> Arguments(string line)
    {
        static bool Space(char c) => c is ' ' or '\t';
        var args = new List<string>();
        var arg = new System.Text.StringBuilder();
        var i = 0;
        var quoted = false;
        for (; i < line.Length && (quoted || !Space(line[i])); i++)
        {
            if (line[i] == '"')
                quoted = !quoted;
            else
                arg.Append(line[i]);
        }
        args.Add(arg.ToString());
        while (true)
        {
            while (i < line.Length && Space(line[i]))
                i++;
            if (i == line.Length)
                return args;
            arg.Clear();
            quoted = false;
            while (i < line.Length && (quoted || !Space(line[i])))
            {
                var slashes = 0;
                for (; i < line.Length && line[i] == '\\'; i++)
                    slashes++;
                if (i < line.Length && line[i] == '"')
                {
                    arg.Append('\\', slashes / 2);
                    if (slashes % 2 == 1)
                        arg.Append('"');
                    else
                        quoted = !quoted;
                    i++;
                }
                else
                {
                    arg.Append('\\', slashes);
                    if (i < line.Length && (quoted || !Space(line[i])))
                        arg.Append(line[i++]);
                }
            }
            args.Add(arg.ToString());
        }
    }

    /// <summary>The https invite <paramref name="text"/> is, as the store makes it: a path with <c>/join/</c> in it.</summary>
    /// <summary>A room's https invite: <c>https://kaeri.app/room#v=3&amp;…</c>.</summary>
    static bool RoomIn(string text) =>
        Uri.TryCreate(text, UriKind.Absolute, out var uri) && uri.Scheme == Uri.UriSchemeHttps
        && uri.Host.Equals("kaeri.app", StringComparison.OrdinalIgnoreCase) && uri.AbsolutePath.TrimEnd('/') == "/room";

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
