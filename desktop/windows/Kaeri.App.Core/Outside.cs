namespace Kaeri.App;

/// <summary>How a link the engine asks the app to open goes to Windows.</summary>
public enum Launch
{
    /// <summary>Not opened at all.</summary>
    Refuse,

    /// <summary>Opened in the default browser.</summary>
    Open,
}

/// <summary>
/// The links the engine hands to Windows through the host's <c>OpenUrl</c>: a tracker's sign-in,
/// a store's page, or a stream a provider plugin says plays elsewhere. A plugin chooses that last
/// link itself, so the host decides what Windows may do with it.
/// </summary>
public static class Outside
{
    /// <summary>
    /// What happens to <paramref name="url"/>: an http or https link with a host opens in the
    /// browser; every other scheme is refused, as a link from a plugin could otherwise name a
    /// file, a share, Windows' own handlers or any app with any arguments.
    /// </summary>
    public static Launch For(string? url, out Uri? uri)
    {
        uri = null;
        if (string.IsNullOrWhiteSpace(url) || !Uri.TryCreate(url.Trim(), UriKind.Absolute, out var parsed))
            return Launch.Refuse;
        if (parsed.IsUnc || parsed.IsFile || parsed.Scheme is not ("http" or "https") || parsed.Host.Length == 0)
            return Launch.Refuse;
        uri = parsed;
        return Launch.Open;
    }
}
