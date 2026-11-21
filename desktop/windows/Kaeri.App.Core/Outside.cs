namespace Kaeri.App;

/// <summary>How a link the engine asks the app to open goes to Windows.</summary>
public enum Launch
{
    /// <summary>Not opened at all.</summary>
    Refuse,

    /// <summary>Opened in the default browser.</summary>
    Open,

    /// <summary>Handed to the app that claims its scheme, with Windows asking first.</summary>
    Ask,
}

/// <summary>
/// The links the engine hands to Windows through the host's <c>OpenUrl</c>: a tracker's sign-in,
/// a store's page, or a stream a provider plugin says plays in another app. A plugin chooses that
/// last link itself, so the host decides what Windows may do with it.
/// </summary>
public static class Outside
{
    /// <summary>
    /// Schemes that reach Windows itself rather than another app: files and shares, Search, the
    /// shell, script, and every <c>ms-</c> handler (Settings, the Store, Office documents).
    /// </summary>
    static readonly string[] Refused = ["file", "search", "search-ms", "shell", "javascript", "vbscript", "data"];

    /// <summary>
    /// What happens to <paramref name="url"/>: http and https open in the browser; a file, a
    /// share or a system handler is refused; any other app's scheme is handed over with Windows
    /// asking first, as a link from a plugin can name any handler with any arguments.
    /// </summary>
    public static Launch For(string? url, out Uri? uri)
    {
        uri = null;
        if (string.IsNullOrWhiteSpace(url) || !Uri.TryCreate(url.Trim(), UriKind.Absolute, out var parsed))
            return Launch.Refuse;
        var scheme = parsed.Scheme.ToLowerInvariant();
        if (parsed.IsUnc || parsed.IsFile || Refused.Contains(scheme) || scheme.StartsWith("ms-", StringComparison.Ordinal))
            return Launch.Refuse;
        var launch = scheme is "https" or "http" ? parsed.Host.Length > 0 ? Launch.Open : Launch.Refuse : Launch.Ask;
        if (launch != Launch.Refuse)
            uri = parsed;
        return launch;
    }
}
