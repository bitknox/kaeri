using Kaeri.Engine.Ffi;

namespace Kaeri.App;

/// <summary>What plays here as the TV takes it over: the episode and the second it is at.</summary>
public sealed record CastFrom(uint Id, string Episode, double Position);

/// <summary>The title bar's cast button, as the session view draws it.</summary>
public enum CastButtonState
{
    /// <summary>No TV found, or TVs turned off in Settings.</summary>
    Hidden,
    Available,
    Connecting,
    Connected,
}

/// <summary>
/// The cast session calls the window draws from: <c>cast_connect</c> with the play to carry
/// over, <c>cast_play_here</c>, <c>cast_set_device</c>, and the view's <c>button</c>,
/// <c>rev</c> and <c>waiting</c>. Everything that reads them goes through here.
/// </summary>
public static class CastApi
{
    /// <summary>
    /// Connects to the Cast row <paramref name="key"/>. Whether the engine carries
    /// <paramref name="from"/> over to the TV itself; when nothing is carried, the caller plays
    /// its request once the TV is connected.
    /// </summary>
    public static bool Connect(FfiTvSender sender, string key, CastFrom? from)
    {
        sender.CastConnect(key, from is null ? null : new FfiCastFrom(from.Id, from.Episode, from.Position));
        return from is not null;
    }

    /// <summary>
    /// Play here: the TV stops and is let go, and the episode and its second come back for the
    /// player here. Null when nothing of Kaeri's played on the TV.
    /// </summary>
    public static CastFrom? PlayHere(FfiTvSender sender) =>
        sender.CastPlayHere() is { } back ? new CastFrom(back.Id, back.Episode, back.Position) : null;

    /// <summary>The device word Kaeri's receiver names this computer by.</summary>
    public static void SetDevice(FfiTvSender sender, string word) => sender.CastSetDevice(word);

    /// <summary>The engine's own button state.</summary>
    public static CastButtonState? Button(FfiCastSessionView view) => view.Button switch
    {
        FfiCastButton.Available => CastButtonState.Available,
        FfiCastButton.Connecting => CastButtonState.Connecting,
        FfiCastButton.Connected => CastButtonState.Connected,
        _ => CastButtonState.Hidden,
    };

    /// <summary>The engine's change count.</summary>
    public static ulong? Rev(FfiCastSessionView view) => view.Rev;

    /// <summary>Whether the TV waits for the stream mid-episode, in whole seconds since it began.</summary>
    public static uint? Waiting(FfiCastItem item) => item.Waiting;
}
