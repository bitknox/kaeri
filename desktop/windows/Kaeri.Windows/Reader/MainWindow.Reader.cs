using Medium = Kaeri.Engine.Core.Medium;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;

namespace Kaeri.Windows;

// The window's side of the reader: the reader's top row is the title bar while it is up, the
// caption buttons come and go with its group, and F11 makes it full screen. The reader calls
// these; MainWindow itself only places it.
public sealed partial class MainWindow
{
    /// <summary>The reader came up over the window, or left it.</summary>
    internal void ReaderShowing(bool showing)
    {
        // Home is out of sight under the reader: no trailer plays and none is kept.
        if (showing)
        {
            TrailerHub.StopAll();
            TrailerPool.Empty();
        }
        SetTitleBar(showing ? ReaderLayer.DragArea : AppTitleBar);
        AppTitleBar.Visibility = showing ? Visibility.Collapsed : Visibility.Visible;
        Nav.Visibility = showing ? Visibility.Collapsed : Visibility.Visible;
        AppWindow.TitleBar.PreferredHeightOption = TitleBarHeightOption.Tall;
        if (!showing && ReaderFullscreen)
            Fullscreen(false);
        if (!showing)
            DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, FocusPage);
    }

    /// <summary>The caption buttons follow the reader's group: there when it is, gone with it.</summary>
    internal void ReaderChrome(bool shown)
    {
        if (ReaderLayer.Showing)
            AppWindow.TitleBar.PreferredHeightOption = shown ? TitleBarHeightOption.Tall : TitleBarHeightOption.Collapsed;
    }

    internal bool ReaderFullscreen => AppWindow.Presenter.Kind == AppWindowPresenterKind.FullScreen;

    /// <summary>Full screen on, off or the other way (null).</summary>
    internal void ReaderSetFullscreen(bool? on) => Fullscreen(on);
}
