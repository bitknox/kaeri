using System.Diagnostics;
#if KAERI_TESTING
using Kaeri.Engine.Ffi;
#endif

namespace Kaeri.Windows.Player;

/// <summary>
/// The clock the starting screen's seconds run on: <see cref="Stopwatch"/> ticks. In a Fixtures
/// build while the engine's still clock is on (a still shot: <c>--still=&lt;seconds&gt;</c>,
/// <c>--show=starting-&lt;state&gt;-&lt;seconds&gt;</c> or <c>KAERI_STILL_AT</c>), that clock's
/// moment instead, in the same ticks, so a shot held at 24 s reads 24 s however long the machine
/// took to get there. The app itself, and a Fixtures run with the clock off, read the stopwatch.
/// </summary>
static class StartClock
{
    /// <summary>Now, in <see cref="Stopwatch"/> ticks.</summary>
    public static long Now()
    {
#if KAERI_TESTING
        if (KaeriFfiMethods.TestingStillClockNowMs() is { } ms)
            return (long)(ms * (Stopwatch.Frequency / 1000.0));
#endif
        return Stopwatch.GetTimestamp();
    }

    /// <summary>How long since <paramref name="from"/>, a moment <see cref="Now"/> gave.</summary>
    public static TimeSpan Since(long from) => Stopwatch.GetElapsedTime(from, Now());
}
