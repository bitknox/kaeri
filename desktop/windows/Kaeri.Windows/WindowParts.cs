using System.Runtime.InteropServices;
using System.Text.Json;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI;
using Microsoft.UI.Windowing;
using Windows.Graphics;

namespace Kaeri.Windows;

/// <summary>Where the window was left: its normal frame in physical pixels, and whether it was maximised.</summary>
sealed record Place(RectInt32 Frame, bool Maximised)
{
    sealed record Stored(int X, int Y, int Width, int Height, bool Maximised);

    public static Place? Read(string path)
    {
        try
        {
            if (!File.Exists(path))
                return null;
            var s = JsonSerializer.Deserialize<Stored>(File.ReadAllText(path));
            return s is { Width: > 0, Height: > 0 } ? new Place(new RectInt32(s.X, s.Y, s.Width, s.Height), s.Maximised) : null;
        }
        catch (Exception)
        {
            return null;
        }
    }

    public static void Write(string path, Place place)
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);
            var f = place.Frame;
            File.WriteAllText(path, JsonSerializer.Serialize(new Stored(f.X, f.Y, f.Width, f.Height, place.Maximised)));
        }
        catch (Exception)
        {
            // Not kept: the next launch centres the window, which is harmless.
        }
    }

    /// <summary>
    /// The frame this place opens at, as <see cref="WindowPlace.Opening"/> has it, or null to open
    /// at the first launch's frame. The display is the one the frame's title strip overlaps most.
    /// <c>DisplayArea.FindAll()</c> is not used: walking its list from C# throws
    /// InvalidCastException on some machines.
    /// </summary>
    public RectInt32? Opening(double scale)
    {
        try
        {
            var strip = new RectInt32(Frame.X, Frame.Y, Math.Max(1, Frame.Width), 40);
            var display = DisplayArea.GetFromRect(strip, DisplayAreaFallback.None);
            return WindowPlace.Opening(Frame.ToFrame(), Maximised, display?.WorkArea.ToFrame(), scale)?.ToRect();
        }
        catch (Exception)
        {
            return null;
        }
    }
}

/// <summary>
/// The frame Restore down takes the window back to, from GetWindowPlacement. Windows keeps it while
/// the window is maximised, and moves it with the window to another display.
/// </summary>
static partial class NormalFrame
{
    /// <summary>The window's normal frame in screen pixels; null when Windows does not say.</summary>
    public static RectInt32? Of(nint hwnd, WindowId window)
    {
        try
        {
            var placement = new WindowPlacement { Length = (uint)Marshal.SizeOf<WindowPlacement>() };
            if (!GetWindowPlacement(hwnd, ref placement))
                return null;
            var display = DisplayArea.GetFromWindowId(window, DisplayAreaFallback.Nearest);
            var normal = new WindowFrame(placement.Left, placement.Top, placement.Right - placement.Left, placement.Bottom - placement.Top);
            return WindowPlace.FromWorkspace(normal, display.OuterBounds.ToFrame(), display.WorkArea.ToFrame()).ToRect();
        }
        catch (Exception)
        {
            return null;
        }
    }

    /// <summary>WINDOWPLACEMENT: the normal frame is in workspace coordinates.</summary>
    [StructLayout(LayoutKind.Sequential)]
    struct WindowPlacement
    {
        public uint Length, Flags, ShowCommand;
        public int MinX, MinY, MaxX, MaxY;
        public int Left, Top, Right, Bottom;
    }

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool GetWindowPlacement(nint hwnd, ref WindowPlacement placement);
}

/// <summary>A frame as WinRT and as the app's core have it.</summary>
static class Frames
{
    public static WindowFrame ToFrame(this RectInt32 rect) => new(rect.X, rect.Y, rect.Width, rect.Height);

    public static RectInt32 ToRect(this WindowFrame frame) => new(frame.X, frame.Y, frame.Width, frame.Height);
}

/// <summary>
/// The taskbar button's progress: overall download progress while anything downloads, paused
/// when everything waits, red when one failed (ITaskbarList3).
/// </summary>
sealed class Taskbar
{
    const int NoProgress = 0, Normal = 2, Error = 4, Paused = 8;

    readonly ITaskbarList3 list;
    readonly nint hwnd;

    Taskbar(ITaskbarList3 list, nint hwnd)
    {
        this.list = list;
        this.hwnd = hwnd;
    }

    public static Taskbar? For(nint hwnd)
    {
        try
        {
            var list = (ITaskbarList3)new TaskbarListClass();
            list.HrInit();
            return new Taskbar(list, hwnd);
        }
        catch (Exception)
        {
            return null;
        }
    }

    public void Downloads(DownloadRow[] rows)
    {
        try
        {
            var going = rows.Where(r => r.State is DownloadState.Active or DownloadState.Queued or DownloadState.Paused).ToArray();
            if (going.Length == 0)
            {
                list.SetProgressState(hwnd, rows.Any(r => r.State == DownloadState.Failed) ? Error : NoProgress);
                if (rows.Any(r => r.State == DownloadState.Failed))
                    list.SetProgressValue(hwnd, 100, 100);
                return;
            }
            ulong done = 0, total = 0;
            foreach (var r in going)
            {
                done += r.Downloaded;
                total += r.Total;
            }
            var state = rows.Any(r => r.State == DownloadState.Failed) ? Error
                : going.All(r => r.State == DownloadState.Paused) ? Paused
                : Normal;
            list.SetProgressState(hwnd, state);
            if (total > 0)
                list.SetProgressValue(hwnd, done, total);
        }
        catch (Exception)
        {
            // The taskbar is decoration: nothing to do.
        }
    }

    public void Clear()
    {
        try
        {
            list.SetProgressState(hwnd, NoProgress);
        }
        catch (Exception)
        {
        }
    }

    [ComImport, Guid("56FDF344-FD6D-11d0-958A-006097C9A090"), ClassInterface(ClassInterfaceType.None)]
    class TaskbarListClass;

    [ComImport, Guid("ea1afb91-9e28-4b86-90e9-9e9f8a5eefaf"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface ITaskbarList3
    {
        // ITaskbarList
        void HrInit();
        void AddTab(nint hwnd);
        void DeleteTab(nint hwnd);
        void ActivateTab(nint hwnd);
        void SetActiveAlt(nint hwnd);
        // ITaskbarList2
        void MarkFullscreenWindow(nint hwnd, [MarshalAs(UnmanagedType.Bool)] bool fullscreen);
        // ITaskbarList3
        void SetProgressValue(nint hwnd, ulong completed, ulong total);
        void SetProgressState(nint hwnd, int flags);
        void RegisterTab(nint tab, nint hwnd);
        void UnregisterTab(nint tab);
        void SetTabOrder(nint tab, nint insertBefore);
        void SetTabActive(nint tab, nint hwnd, uint reserved);
        void ThumbBarAddButtons(nint hwnd, uint count, [MarshalAs(UnmanagedType.LPArray, SizeParamIndex = 1)] ThumbButton[] buttons);
        void ThumbBarUpdateButtons(nint hwnd, uint count, [MarshalAs(UnmanagedType.LPArray, SizeParamIndex = 1)] ThumbButton[] buttons);
    }

    /// <summary>THUMBBUTTON: one button under the window's taskbar thumbnail.</summary>
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct ThumbButton
    {
        public const uint Icon = 2, Tooltip = 4, Flags = 8;
        public const uint Enabled = 0, Hidden = 8;

        public uint Mask;
        public uint Id;
        public uint Bitmap;
        public nint IconHandle;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)]
        public string Tip;
        public uint State;
    }

    /// <summary>
    /// Puts <paramref name="buttons"/> under the window's thumbnail, which takes them once per
    /// window and then only updates them; false when the taskbar would not.
    /// </summary>
    public bool ThumbButtons(ThumbButton[] buttons, bool first)
    {
        try
        {
            if (first)
                list.ThumbBarAddButtons(hwnd, (uint)buttons.Length, buttons);
            else
                list.ThumbBarUpdateButtons(hwnd, (uint)buttons.Length, buttons);
            return true;
        }
        catch (Exception)
        {
            return false;
        }
    }
}
