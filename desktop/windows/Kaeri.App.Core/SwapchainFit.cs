namespace Kaeri.App;

/// <summary>What a panel showing mpv's swapchain does on a poll.</summary>
public enum SwapchainStep
{
    /// <summary>Nothing: the swapchain is bound and its matrix fits the panel's scale.</summary>
    None,

    /// <summary>Bind the swapchain to the panel, then set its matrix.</summary>
    Bind,

    /// <summary>Set the bound swapchain's matrix again: the panel's scale changed.</summary>
    Rescale,
}

/// <summary>
/// mpv's swapchain in a SwapChainPanel. Its buffers are in physical pixels and XAML lays them out
/// one per effective pixel, so the swapchain's matrix scales them by the inverse of the panel's
/// scale. mpv resizes the same swapchain, so a scale change (the window moved to another display,
/// or the display's scale changed) has to set the matrix again on a swapchain already bound.
/// </summary>
public sealed class SwapchainFit
{
    nint bound;
    float scaleX = 1, scaleY = 1;

    /// <summary>What to do with <paramref name="swapchain"/> at the panel's scale now.</summary>
    public SwapchainStep Next(nint swapchain, float panelScaleX, float panelScaleY)
    {
        if (swapchain == 0)
            return SwapchainStep.None;
        if (swapchain != bound)
            return SwapchainStep.Bind;
        return Math.Abs(panelScaleX - scaleX) > 0.001 || Math.Abs(panelScaleY - scaleY) > 0.001
            ? SwapchainStep.Rescale
            : SwapchainStep.None;
    }

    /// <summary>The swapchain is bound and its matrix set for the panel's scale.</summary>
    public void Done(nint swapchain, float panelScaleX, float panelScaleY)
    {
        bound = swapchain;
        scaleX = panelScaleX;
        scaleY = panelScaleY;
    }

    /// <summary>The player closed: the next swapchain is bound afresh.</summary>
    public void Reset() => Done(0, 1, 1);

    /// <summary>The matrix's scale for a panel at <paramref name="panelScale"/>: its inverse.</summary>
    public static float Matrix(float panelScale) => panelScale > 0 ? 1f / panelScale : 1f;
}
