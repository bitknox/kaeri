using Microsoft.UI.Xaml;

namespace Kaeri.Windows;

public partial class App : Application
{
    MainWindow? window;
    StartFailedWindow? failed;

    public App()
    {
        InitializeComponent();
        // What reaches here goes to the engine's log, and to a file for a run with no engine yet.
        // One handler's mistake (a button's, a page's) is not worth the window: the app goes on.
        // Only what leaves the process in no state to go on ends it.
        UnhandledException += (_, e) =>
        {
            e.Handled = e.Exception is not (OutOfMemoryException or InsufficientExecutionStackException or System.Runtime.InteropServices.SEHException);
            try
            {
                // XAML's own failures come with no managed stack: the HRESULT, XAML's message and
                // the managed frames that were running say where.
                Kaeri.Engine.Ffi.KaeriFfiMethods.LogLine(Kaeri.Engine.Ffi.FfiLogLevel.Error, "windows",
                    $"{(e.Handled ? "went on after" : "stopped on")} {e.Exception} (0x{e.Exception.HResult:X8}, {e.Message}){(e.Exception.StackTrace is null ? " while running: " + Environment.StackTrace : "")}");
            }
            catch (Exception)
            {
                // No engine log yet.
            }
            // Kept on the device only while Save crash reports is on; the engine redacts it.
            CrashGuard.Caught(e.Exception);
        };
    }

    static App()
    {
        // A task nobody waited on that failed: logged, never the end of the app.
        TaskScheduler.UnobservedTaskException += (_, e) =>
        {
            e.SetObserved();
            try
            {
                Kaeri.Engine.Ffi.KaeriFfiMethods.LogLine(Kaeri.Engine.Ffi.FfiLogLevel.Warn, "windows", $"a task failed unseen: {e.Exception.InnerException?.Message}");
            }
            catch (Exception)
            {
                // No engine log yet.
            }
        };
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        var options = Options.Parse(Environment.GetCommandLineArgs().Skip(1));
        StringResources.Install(options.Language, options.Fixture);
        try
        {
            window = new MainWindow(options);
        }
        catch (Kaeri.Engine.Ffi.FfiException e)
        {
            // The engine refused to start (a folder, config.toml, the database): say why, in a
            // window of its own, rather than end before any window shows.
            Kaeri.Engine.Ffi.KaeriFfiMethods.LogLine(Kaeri.Engine.Ffi.FfiLogLevel.Error, "windows", $"the engine did not start: {e.Message}");
            failed = new StartFailedWindow(StartFailedWindow.Reason(e), Kaeri.App.AppDirs.User().Config);
            failed.Activate();
            return;
        }
        window.Activate();
#if KAERI_TESTING
        Probe.Start(window.DispatcherQueue);
#endif
    }
}
