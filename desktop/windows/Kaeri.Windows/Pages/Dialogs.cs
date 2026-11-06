using Kaeri.Engine.Ffi;
using Microsoft.UI.Xaml.Controls;

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
        var done = closed = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
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
}
