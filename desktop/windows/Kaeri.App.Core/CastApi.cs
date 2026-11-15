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
/// The cast session calls whose shape changes on the engine branch: <c>cast_connect</c> with the
/// play to carry over, <c>cast_play_here</c>, <c>cast_set_device</c>, and the view's
/// <c>button</c>, <c>rev</c> and <c>waiting</c>. Everything that reads them goes through here,
/// so moving to the next engine changes this file alone. Until a field lands, what it would say
/// is worked out from what the view has; a call that has not landed does nothing.
/// </summary>
public static class CastApi
{
    /// <summary>
    /// Connects to the Cast row <paramref name="key"/>. Whether the engine carried
    /// <paramref name="from"/> over to the TV itself; when it did not, the caller plays it once
    /// the TV is connected.
    /// </summary>
    public static bool Connect(FfiTvSender sender, string key, CastFrom? from)
    {
        sender.CastConnect(key);
        return false;
    }

    /// <summary>
    /// Play here: the TV stops and is let go, and the episode and its second come back for the
    /// player here. Null when the engine cannot say, and the caller falls back to what it saw.
    /// </summary>
    public static CastFrom? PlayHere(FfiTvSender sender) => null;

    /// <summary>The device word the receiver names this computer by. Coming in the engine.</summary>
    public static void SetDevice(FfiTvSender sender, string word)
    {
    }

    /// <summary>The engine's own button state, once the view carries it.</summary>
    public static CastButtonState? Button(FfiCastSessionView view) => null;

    /// <summary>The engine's change count, once the view carries it.</summary>
    public static ulong? Rev(FfiCastSessionView view) => null;

    /// <summary>
    /// Whether the TV waits for the stream mid-episode, in whole seconds since it began. The
    /// engine's <c>buffering</c> stands in until <c>waiting</c> lands, without a count.
    /// </summary>
    public static uint? Waiting(FfiCastItem item) => item.Buffering && !item.Loading ? 0u : null;
}
