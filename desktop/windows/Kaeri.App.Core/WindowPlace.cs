namespace Kaeri.App;

/// <summary>A window's outer rectangle in physical pixels, as Windows gives and takes it.</summary>
public readonly record struct WindowFrame(int X, int Y, int Width, int Height)
{
    public int Right => X + Width;
    public int Bottom => Y + Height;
}

/// <summary>
/// Where the window opens and what is kept of it for the next launch, by Windows' own convention:
/// a window left maximised opens maximised again, and Restore down takes it back to the frame it
/// had before it was maximised.
/// </summary>
public static class WindowPlace
{
    /// <summary>
    /// The frame to keep. Restored, it is the window's own frame, <paramref name="current"/>.
    /// Maximised, it is <paramref name="normal"/>: the frame Restore down goes back to, which
    /// Windows keeps for the window, on the display the window is maximised on. The maximised
    /// rectangle itself is never kept. When Windows does not say, the frame kept before,
    /// <paramref name="kept"/>, stays; null keeps nothing.
    /// </summary>
    public static WindowFrame? Keep(bool maximised, WindowFrame current, WindowFrame? normal, WindowFrame? kept) =>
        maximised ? normal ?? kept : current;

    /// <summary>
    /// GetWindowPlacement's normal frame, which is in workspace coordinates (from the corner of
    /// the work area of the window's display), in screen coordinates. A taskbar on the left or at
    /// the top of that display moves the two apart.
    /// </summary>
    public static WindowFrame FromWorkspace(WindowFrame normal, WindowFrame display, WindowFrame workArea) =>
        normal with { X = normal.X + workArea.X - display.X, Y = normal.Y + workArea.Y - display.Y };

    /// <summary>
    /// The frame a kept place opens at, given the work area of the display its title strip lies
    /// on most (null when it lies on none), or null to open at the first launch's frame on the
    /// main display. A frame whose title strip can be grabbed opens as it was kept. A maximised
    /// place whose frame is the maximised rectangle itself says nothing of where Restore down
    /// should go, so it opens maximised over the first launch's frame on that display.
    /// </summary>
    public static WindowFrame? Opening(WindowFrame kept, bool maximised, WindowFrame? workArea, double scale)
    {
        if (workArea is not { } area)
            return null;
        if (maximised && Covers(kept, area))
            return First(area, scale);
        return Grabbable(kept, area) ? kept : null;
    }

    /// <summary>
    /// Whether enough of the frame's title strip lies on the work area to be grabbed: 120 pixels
    /// of its width, its top no more than 8 pixels above the work area and more than 40 above its
    /// bottom.
    /// </summary>
    public static bool Grabbable(WindowFrame frame, WindowFrame workArea)
    {
        var left = Math.Max(workArea.X, frame.X);
        var right = Math.Min(workArea.Right, frame.Right);
        return right - left >= 120 && frame.Y >= workArea.Y - 8 && frame.Y < workArea.Bottom - 40;
    }

    /// <summary>
    /// The maximised rectangle: past the work area on every side by the window's resize border,
    /// a few pixels at any scale.
    /// </summary>
    static bool Covers(WindowFrame frame, WindowFrame workArea) =>
        frame.X < workArea.X && frame.Y < workArea.Y && frame.Right > workArea.Right && frame.Bottom > workArea.Bottom
        && frame.Width - workArea.Width <= 128 && frame.Height - workArea.Height <= 128;

    /// <summary>
    /// The window's minimum, 500 by 420 effective pixels, in the physical pixels the presenter
    /// takes at <paramref name="scale"/>, the scale of the display the window is on now.
    /// </summary>
    public static (int Width, int Height) Minimum(double scale) =>
        ((int)Math.Round(500 * scale), (int)Math.Round(420 * scale));

    /// <summary>
    /// The first launch's frame: 80% of the work area, at least 1280 by 800 effective pixels at
    /// <paramref name="scale"/> and at most the work area, centred on it.
    /// </summary>
    public static WindowFrame First(WindowFrame workArea, double scale)
    {
        var width = Math.Min(Math.Max((int)(workArea.Width * 0.8), (int)(1280 * scale)), workArea.Width);
        var height = Math.Min(Math.Max((int)(workArea.Height * 0.8), (int)(800 * scale)), workArea.Height);
        return new WindowFrame(
            workArea.X + Math.Max(0, (workArea.Width - width) / 2), workArea.Y + Math.Max(0, (workArea.Height - height) / 2), width, height);
    }
}
