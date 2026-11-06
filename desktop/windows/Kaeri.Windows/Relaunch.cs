using System.Diagnostics;
using Kaeri.App;
using Kaeri.Engine.Ffi;

namespace Kaeri.Windows;

/// <summary>
/// Opening the app again: a hidden PowerShell waits for this copy to end, since a second copy
/// only hands its launch to the first, then starts it with the arguments it ran with, less a
/// link and the testing switches. A fixtures scene stays as it is.
/// </summary>
static class Relaunch
{
    /// <summary>Closes the window and opens the app again; <paramref name="why"/> goes to the log.</summary>
    public static void Now(string why)
    {
        if (Start(why))
            Ctx.Current.Window.CloseNow();
    }

    /// <summary>
    /// Sets the app to open again once this copy has ended, and whether this copy should now end:
    /// false in a fixtures scene, which keeps its window.
    /// </summary>
    public static bool Start(string why)
    {
        if (Ctx.Current.Options.Scene is not null)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Info, "windows", $"{why}: a scene keeps the window");
            return false;
        }
        try
        {
            var app = Environment.ProcessPath ?? throw new InvalidOperationException("the app's own path is unknown");
            Process.Start(new ProcessStartInfo("powershell.exe",
                ProfileWords.Relaunch(Environment.ProcessId, app, Environment.GetCommandLineArgs().Skip(1)))
            {
                UseShellExecute = false,
                CreateNoWindow = true,
            });
        }
        catch (Exception e) when (e is System.ComponentModel.Win32Exception or InvalidOperationException)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Error, "windows", $"{why}: the app could not open again: {e.Message}");
        }
        return true;
    }
}
