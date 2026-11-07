// The wheel's glide: a notch moves on the next frame, eases into its target and stops there, and
// notches that follow carry on from the speed already reached.

using Kaeri.App;
using Xunit;

namespace Kaeri.App.Tests;

public class WheelGlideTests
{
    const double Frame = 1 / 240.0;

    /// <summary>The positions frame by frame from <paramref name="from"/> until the glide ends.</summary>
    static List<double> Run(WheelGlide glide, double from)
    {
        var shown = new List<double>();
        for (var t = from; glide.Moving && t < from + 2; t += Frame)
            shown.Add(glide.Step(t));
        return shown;
    }

    [Fact]
    public void A_notch_moves_on_the_first_frame_and_settles_on_the_target()
    {
        var glide = new WheelGlide();
        glide.Push(100, 0, 0, 0, 1000);
        var shown = Run(glide, Frame);
        Assert.True(shown[0] > 3, $"first frame at {shown[0]}");
        Assert.Equal(100, shown[^1]);
        Assert.InRange(shown.Count * Frame, WheelGlide.Seconds - 2 * Frame, WheelGlide.Seconds + 2 * Frame);
        // Never back, never past.
        for (var i = 1; i < shown.Count; i++)
            Assert.True(shown[i] >= shown[i - 1] && shown[i] <= 100);
        // Fastest first: each frame moves no more than the one before.
        for (var i = 2; i < shown.Count; i++)
            Assert.True(shown[i] - shown[i - 1] <= shown[i - 1] - shown[i - 2] + 1e-9);
    }

    [Fact]
    public void Notches_in_a_row_add_up_and_keep_the_speed()
    {
        var glide = new WheelGlide();
        glide.Push(100, 0, 0, 0, 10000);
        var t = 0.0;
        var last = 0.0;
        var steps = new List<double>();
        for (var notch = 1; notch < 8; notch++)
        {
            for (var i = 0; i < 6; i++)
            {
                t += Frame;
                var at = glide.Step(t);
                steps.Add(at - last);
                last = at;
            }
            var before = glide.Sample(t).Velocity;
            glide.Push(100, t, last, 0, 10000);
            // The next notch never slows the glide down.
            Assert.True(glide.Sample(t).Velocity >= before - 1e-6);
        }
        Assert.Equal(800, glide.Target);
        foreach (var step in Run(glide, t + Frame))
        {
            steps.Add(step - last);
            last = step;
        }
        Assert.Equal(800, last);
        // One glide: it never stands still on the way.
        Assert.All(steps.Take(steps.Count - 1), s => Assert.True(s > 0));
    }

    [Fact]
    public void A_notch_the_other_way_turns_from_where_it_is()
    {
        var glide = new WheelGlide();
        glide.Push(300, 0, 0, 0, 1000);
        var at = glide.Step(0.04);
        glide.Push(-100, 0.04, at, 0, 1000);
        Assert.Equal(at - 100, glide.Target, 6);
    }

    [Fact]
    public void The_target_stays_within_the_ends()
    {
        var glide = new WheelGlide();
        glide.Push(-100, 0, 30, 0, 1000);
        Assert.Equal(0, glide.Target);
        var shown = Run(glide, Frame);
        Assert.All(shown, s => Assert.InRange(s, 0, 30));
        glide.Push(-100, 1, 0, 0, 1000);
        Assert.Equal(0, glide.Step(1 + Frame));
        Assert.False(glide.Moving);
    }
}
