using Microsoft.UI.Xaml;

namespace Kaeri.PlayerSpike;

public partial class App : Application
{
    private MainWindow? _window;

    public App()
    {
        InitializeComponent();
        // The spike runs unattended over SSH, so whatever kills it goes to a file.
        UnhandledException += (_, e) =>
        {
            var dir = Path.Combine(Path.GetTempPath(), "kaeri-player-spike");
            Directory.CreateDirectory(dir);
            File.AppendAllText(Path.Combine(dir, "crash.txt"), $"{DateTime.Now:O} {e.Exception}\n");
        };
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        _window = new MainWindow(SpikeOptions.Parse(Environment.GetCommandLineArgs().Skip(1)));
        _window.Activate();
    }
}
