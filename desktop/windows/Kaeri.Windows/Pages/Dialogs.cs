using Kaeri.Engine.Ffi;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Every ContentDialog opens through here. WinUI allows one open dialog at a time and throws
/// from <c>ShowAsync</c> for a second, which inside an <c>async void</c> handler ends the app.
/// A dialog asked for while another is up waits for it to close; one that must come first (the
/// question closing the window asks) closes the other, even one that holds itself open.
/// </summary>
static class Dialogs
{
    static ContentDialog? open;
    static TaskCompletionSource? closed;
    /// <summary>Dialogs waiting for the one that is up, and those of them no longer wanted.</summary>
    static readonly HashSet<ContentDialog> waiting = [], dropped = [];
    /// <summary>Dialogs the app is closing: a Closing handler that holds its dialog open lets these go.</summary>
    static readonly HashSet<ContentDialog> forced = [];

    /// <summary>Whether one of the app's dialogs is up.</summary>
    public static bool Open => open is not null;

    /// <summary>A dialog is about to open: the window makes room for it first.</summary>
    public static event Action? Opening;

    /// <summary>
    /// Shows <paramref name="dialog"/> once no other is up, and its answer. <paramref name="shown"/>
    /// runs as it opens. With <paramref name="first"/> an open dialog is closed (as Cancel) to make
    /// way. A dialog WinUI refuses, or one closed with <see cref="Close"/> while it waited, counts
    /// as cancelled.
    /// </summary>
    public static async Task<ContentDialogResult> Show(ContentDialog dialog, Action? shown = null, bool first = false)
    {
        if (open == dialog || waiting.Contains(dialog))
            return ContentDialogResult.None;
        waiting.Add(dialog);
        try
        {
            while (open is { } other && closed is { } wait)
            {
                if (first)
                    Force(other);
                await wait.Task;
                if (dropped.Contains(dialog))
                    return ContentDialogResult.None;
            }
            if (dropped.Contains(dialog))
                return ContentDialogResult.None;
        }
        finally
        {
            waiting.Remove(dialog);
            dropped.Remove(dialog);
        }
        open = dialog;
        Palette(dialog);
        var done = closed = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        dialog.Opened += OnOpened;
        try
        {
            Opening?.Invoke();
            var showing = dialog.ShowAsync();
            shown?.Invoke();
            return await showing;
        }
        catch (Exception e)
        {
            // A dialog opened outside this helper is up. Every dialog comes through here, so this is a slip.
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"a dialog was refused: {e.Message}");
            return ContentDialogResult.None;
        }
        finally
        {
            dialog.Opened -= OnOpened;
            forced.Remove(dialog);
            if (open == dialog)
            {
                open = null;
                closed = null;
            }
            done.TrySetResult();
        }
    }

    /// <summary>Whether the app is closing <paramref name="dialog"/>, so a Closing handler must let it go.</summary>
    public static bool Forced(ContentDialog dialog) => forced.Contains(dialog);

    /// <summary>
    /// Closes <paramref name="dialog"/> as Cancel: one waiting its turn never opens, and one that is
    /// up closes even when it would hold itself open.
    /// </summary>
    public static void Close(ContentDialog dialog)
    {
        if (waiting.Contains(dialog))
            dropped.Add(dialog);
        else if (open == dialog)
            Force(dialog);
    }

    /// <summary>
    /// Open: the keyboard is in it. A Hide that came while it was still opening is not kept by
    /// WinUI, so it is done again now.
    /// </summary>
    static void OnOpened(ContentDialog dialog, ContentDialogOpenedEventArgs args)
    {
        if (forced.Contains(dialog))
            dialog.Hide();
        else
            Refocus();
    }

    /// <summary>
    /// Puts the keyboard back in the dialog that is up when it is somewhere else. A dialog that
    /// opens while the window is in the background, or a window that comes back to the front,
    /// leaves the focus on the page under the dialog's shade, where Escape and Tab can't reach the
    /// dialog and the mouse can't reach the page: the window would seem locked.
    /// </summary>
    public static void Refocus()
    {
        if (open is not { XamlRoot: { } root } dialog)
            return;
        for (var at = FocusManager.GetFocusedElement(root) as DependencyObject; at is not null; at = VisualTreeHelper.GetParent(at))
            if (at == dialog)
                return;
        if (FocusManager.FindFirstFocusableElement(dialog) is UIElement first)
            first.Focus(FocusState.Programmatic);
        else
            dialog.Focus(FocusState.Programmatic);
    }

    static void Force(ContentDialog dialog)
    {
        forced.Add(dialog);
        dialog.Hide();
    }

    /// <summary>Closes the dialog that is up, if any, as Cancel, whatever it would rather do.</summary>
    public static void HideOpen()
    {
        if (open is { } dialog)
            Force(dialog);
    }

    static readonly global::Windows.UI.ViewManagement.AccessibilitySettings accessibility = new();

    /// <summary>
    /// Gives the dialog the app's palette. ContentDialog's template is Fluent's, and reads Fluent's
    /// own dark brushes, not the palette merged after them, so dialogs came up in Fluent's greys.
    /// The palette's Dark dictionary (merged in App.xaml) is copied onto the dialog, whose own resources
    /// its template reads first: content on Raised, buttons on the ground, controls in the
    /// palette. Under a contrast theme the system's colours rule, so nothing is copied.
    /// </summary>
    static void Palette(ContentDialog dialog)
    {
        if (accessibility.HighContrast || dialog.Resources.ContainsKey("GroundBrush"))
            return;
        if (Microsoft.UI.Xaml.Application.Current.Resources.MergedDictionaries
                .Select(d => d.ThemeDictionaries.TryGetValue("Dark", out var theme) ? theme as Microsoft.UI.Xaml.ResourceDictionary : null)
                .LastOrDefault(d => d is not null) is not { } dark)
            return;
        foreach (var (key, value) in dark)
            if (!dialog.Resources.ContainsKey(key))
                dialog.Resources[key] = value;
        // The style set these from Fluent's dictionary before the copy; the dialog's own values win.
        if (dialog.ReadLocalValue(Control.BackgroundProperty) == Microsoft.UI.Xaml.DependencyProperty.UnsetValue && dark.TryGetValue("ContentDialogBackground", out var ground))
            dialog.Background = (Microsoft.UI.Xaml.Media.Brush)ground;
        if (dialog.ReadLocalValue(Control.BorderBrushProperty) == Microsoft.UI.Xaml.DependencyProperty.UnsetValue && dark.TryGetValue("ContentDialogBorderBrush", out var edge))
            dialog.BorderBrush = (Microsoft.UI.Xaml.Media.Brush)edge;
        if (dialog.ReadLocalValue(Control.ForegroundProperty) == Microsoft.UI.Xaml.DependencyProperty.UnsetValue && dark.TryGetValue("TextFillColorPrimaryBrush", out var silk))
            dialog.Foreground = (Microsoft.UI.Xaml.Media.Brush)silk;
    }
}
