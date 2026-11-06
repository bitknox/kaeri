using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Windows.Pages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows;

/// <summary>
/// What shows instead of the app when the engine could not start: why, in the engine's words
/// (which name the file and what to do), the folder it lives in, and Quit. The same window says
/// when the engine stopped while the app was open, with Open again in place of the folder.
/// </summary>
sealed class StartFailedWindow : Window
{
    public StartFailedWindow(string reason, string folder)
        : this(T.StartFailedTitle, reason, T.StartFailedOpenFolder, async () =>
        {
            try
            {
                Directory.CreateDirectory(folder);
                await global::Windows.System.Launcher.LaunchFolderPathAsync(folder);
            }
            catch (Exception e)
            {
                KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"the folder did not open: {e.Message}");
            }
        })
    {
    }

    /// <summary>The engine stopped with the window open: what happened, Open again and Quit.</summary>
    public static StartFailedWindow Stopped() =>
        new(T.EngineStoppedTitle, T.EngineStoppedDetail, T.EngineStoppedOpenAgain, () =>
        {
            if (Relaunch.Start("the engine stopped"))
                Application.Current.Exit();
        });

    StartFailedWindow(string title, string reason, string action, Action act)
    {
        Title = T.AppName;
        var body = new StackPanel { Spacing = 16, Padding = new Thickness(32), MaxWidth = 560, VerticalAlignment = VerticalAlignment.Center };
        body.Children.Add(Ui.Text(title, "SubtitleTextBlockStyle"));
        var why = Ui.Text(reason, "BodyTextBlockStyle");
        why.IsTextSelectionEnabled = true;
        body.Children.Add(why);
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var open = new Button { Content = action };
        open.Click += (_, _) => act();
        var quit = new Button { Content = T.StartFailedQuit, Style = Ui.Res<Style>("AccentButtonStyle") };
        quit.Click += (_, _) => Application.Current.Exit();
        buttons.Children.Add(open);
        buttons.Children.Add(quit);
        body.Children.Add(buttons);
        Content = new Grid
        {
            Background = Ui.Res<Brush>("GroundBrush"),
            RequestedTheme = ElementTheme.Dark,
            Children = { body },
        };
        Closed += (_, _) => Application.Current.Exit();
        AppWindow.Resize(new global::Windows.Graphics.SizeInt32(640, 360));
    }

    /// <summary>The engine's words for why it could not start, or the exception's when it gave none.</summary>
    public static string Reason(FfiException e) => e switch
    {
        FfiException.Dirs x => x.msg.Text(),
        FfiException.Config x => x.msg.Text(),
        FfiException.Store x => x.msg.Text(),
        FfiException.Client x => x.msg.Text(),
        FfiException.Runtime x => x.msg.Text(),
        FfiException.Engine x => x.msg.Text(),
        FfiException.Remote x => x.msg.Text(),
        _ => e.Message,
    };
}
