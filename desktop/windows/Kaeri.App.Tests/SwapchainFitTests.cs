// mpv's swapchain in a XAML panel: bound once, and its matrix set again whenever the panel's
// scale changes, since mpv resizes the same swapchain.

using Kaeri.App;
using Xunit;

namespace Kaeri.App.Tests;

public class SwapchainFitTests
{
    [Fact]
    public void ANewSwapchainIsBoundWithTheInverseOfThePanelsScale()
    {
        var fit = new SwapchainFit();
        Assert.Equal(SwapchainStep.None, fit.Next(0, 1.5f, 1.5f));
        Assert.Equal(SwapchainStep.Bind, fit.Next(0x10, 1.5f, 1.5f));
        Assert.Equal(1f / 1.5f, SwapchainFit.Matrix(1.5f));
        fit.Done(0x10, 1.5f, 1.5f);
        Assert.Equal(SwapchainStep.None, fit.Next(0x10, 1.5f, 1.5f));
    }

    [Fact]
    public void AScaleChangeSetsTheMatrixAgainOnTheSameSwapchain()
    {
        var fit = new SwapchainFit();
        fit.Done(0x10, 1f, 1f);
        // Moved to a 150% display: mpv resizes the same swapchain, whose matrix was identity.
        Assert.Equal(SwapchainStep.Rescale, fit.Next(0x10, 1.5f, 1.5f));
        fit.Done(0x10, 1.5f, 1.5f);
        // And back to 100%: the 1/1.5 matrix goes.
        Assert.Equal(SwapchainStep.Rescale, fit.Next(0x10, 1f, 1f));
        Assert.Equal(1f, SwapchainFit.Matrix(1f));
    }

    [Fact]
    public void AClosedPlayersNextSwapchainIsBoundAfresh()
    {
        var fit = new SwapchainFit();
        fit.Done(0x10, 1.25f, 1.25f);
        fit.Reset();
        Assert.Equal(SwapchainStep.Bind, fit.Next(0x10, 1.25f, 1.25f));
    }
}
