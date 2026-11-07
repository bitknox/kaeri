namespace Kaeri.App;

/// <summary>
/// A mouse wheel's glide along one axis, as a browser smooths the wheel. Each notch moves the
/// target, and the position eases there from where it stands at the speed it already has: it
/// moves on the very next frame, slows into the target over <see cref="Seconds"/>, and a quick
/// run of notches is one glide rather than a step per notch. Times are in seconds on any one
/// clock; the caller samples it once a frame.
/// </summary>
public sealed class WheelGlide
{
    /// <summary>How long a notch's glide takes from rest.</summary>
    public const double Seconds = 0.13;

    double from, speed, to, start, length = Seconds;

    /// <summary>Whether a glide is under way.</summary>
    public bool Moving { get; private set; }

    /// <summary>The glide under way: where it set off from, at what speed, and how long it takes to arrive.</summary>
    public (double From, double Speed, double Length) Segment => (from, speed, length);

    /// <summary>Whether the glide is still on its way at <paramref name="now"/>.</summary>
    public bool Running(double now) => Moving && now - start < length;

    /// <summary>Where the glide is heading.</summary>
    public double Target => to;

    /// <summary>
    /// A notch of <paramref name="by"/> at <paramref name="now"/>: from <paramref name="shown"/>
    /// when nothing is moving, else from where the glide is. Notches the same way add up; one the
    /// other way turns the glide from where it stands. The target stays within
    /// <paramref name="min"/> and <paramref name="max"/>.
    /// </summary>
    public void Push(double by, double now, double shown, double min, double max)
    {
        var (at, velocity) = Moving ? Sample(now) : (shown, 0);
        var target = Moving && Math.Sign(by) == Math.Sign(to - at) ? to + by : at + by;
        Begin(at, velocity, Math.Clamp(target, min, Math.Max(min, max)), now);
    }

    /// <summary>The position at <paramref name="now"/>; the glide ends once it is there.</summary>
    public double Step(double now)
    {
        var (at, _) = Sample(now);
        if (now - start >= length)
            Moving = false;
        return at;
    }

    /// <summary>Stops where it is: something else moved the view.</summary>
    public void Stop() => Moving = false;

    /// <summary>
    /// The position and speed at <paramref name="now"/>: a cubic from (<see cref="from"/>,
    /// <see cref="speed"/>) to the target at rest.
    /// </summary>
    public (double Position, double Velocity) Sample(double now)
    {
        var s = (now - start) / length;
        if (s >= 1)
            return (to, 0);
        if (s <= 0)
            return (from, speed);
        var m = speed * length;
        var s2 = s * s;
        var s3 = s2 * s;
        var position = (2 * s3 - 3 * s2 + 1) * from + (s3 - 2 * s2 + s) * m + (3 * s2 - 2 * s3) * to;
        var velocity = ((6 * s2 - 6 * s) * from + (3 * s2 - 4 * s + 1) * m + (6 * s - 6 * s2) * to) / length;
        return (position, velocity);
    }

    void Begin(double at, double velocity, double target, double now)
    {
        var distance = target - at;
        from = at;
        to = target;
        start = now;
        length = Seconds;
        if (Math.Abs(distance) < 0.5)
        {
            speed = 0;
            length = 1e-6;
            Moving = Math.Abs(distance) > 0;
            return;
        }
        // From rest, a cubic ease-out: its fastest frame is its first, so nothing waits.
        var least = 3 * distance / length;
        if (Math.Sign(velocity) == Math.Sign(distance) && Math.Abs(velocity) > Math.Abs(least))
        {
            // Already faster: keep that speed. Past a cubic ease-out's start the curve would
            // overshoot and come back, so it arrives sooner instead.
            speed = velocity;
            length = 3 * distance / speed;
        }
        else
        {
            speed = least;
        }
        Moving = true;
    }
}
