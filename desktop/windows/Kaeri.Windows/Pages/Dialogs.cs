using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Every ContentDialog opens through here. WinUI allows one open dialog at a time and throws
/// from <c>ShowAsync</c> for a second, which inside an <c>async void</c> handler ends the app.
/// A dialog asked for while another is up waits for it to close; one that must come first (the
/// question closing the window asks) closes the other.
/// </summary>
static class Dialogs
{
    static ContentDialog? open;
    static TaskCompletionSource? closed;

    /// <summary>Whether one of the app's dialogs is up.</summary>
    public static bool Open => open is not null;

    /// <summary>
    /// Shows <paramref name="dialog"/> once no other is up, and its answer. <paramref name="shown"/>
    /// runs as it opens. With <paramref name="first"/> an open dialog is closed (as Cancel) to make
    /// way. A dialog WinUI refuses all the same counts as cancelled.
    /// </summary>
    public static async Task<ContentDialogResult> Show(ContentDialog dialog, Action? shown = null, bool first = false)
    {
        while (open is { } other && closed is { } wait)
        {
            if (other == dialog)
                return ContentDialogResult.None;
            if (first)
                other.Hide();
            await wait.Task;
        }
        open = dialog;
        Palette(dialog);
        var done = closed = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        try
        {
            var showing = dialog.ShowAsync();
            shown?.Invoke();
            return await showing;
        }
        catch (Exception)
        {
            // A dialog opened outside this helper is up.
            return ContentDialogResult.None;
        }
        finally
        {
            if (open == dialog)
            {
                open = null;
                closed = null;
            }
            done.TrySetResult();
        }
    }

    static readonly global::Windows.UI.ViewManagement.AccessibilitySettings accessibility = new();

    /// <summary>
    /// Gives the dialog the app's palette. A ContentDialog opens in the popup layer, where WinUI
    /// reads Fluent's own dark theme and not the app's overrides, so dialogs came up in Fluent's
    /// greys. The app's Dark dictionary is copied onto the dialog, whose own resources its template
    /// reads first: content on Raised, buttons on the ground, controls in the palette. Under a
    /// contrast theme the system's colours rule, so nothing is copied.
    /// </summary>
    static void Palette(ContentDialog dialog)
    {
        if (accessibility.HighContrast || dialog.Resources.ContainsKey("GroundBrush"))
            return;
        if (!Microsoft.UI.Xaml.Application.Current.Resources.ThemeDictionaries.TryGetValue("Dark", out var found) || found is not Microsoft.UI.Xaml.ResourceDictionary dark)
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

    /// <summary>Closes the dialog that is up, if any, as Cancel.</summary>
    public static void HideOpen() => open?.Hide();
}
