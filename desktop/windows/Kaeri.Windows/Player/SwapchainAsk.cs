using System.Runtime.InteropServices;
using Kaeri.Engine.Ffi;
using Microsoft.UI.Dispatching;

namespace Kaeri.Windows.Player;

/// <summary>
/// Asks mpv for its composition swapchain on a worker thread and hands the answer to the UI
/// thread. mpv answers from its video thread, which can be busy for seconds compiling shaders or
/// rebuilding after a resize, and the window must keep answering meanwhile. One question is out
/// at a time. The swapchain is held (AddRef) from the moment it is read until
/// the UI thread has bound it, so one mpv replaces in between is not freed under the panel.
/// </summary>
sealed class SwapchainAsk(DispatcherQueue ui)
{
    int asking;

    /// <summary>Asks <paramref name="player"/>; <paramref name="bind"/> runs on the UI thread with a swapchain that is there.</summary>
    public void Ask(FfiPlayer player, Action<nint> bind)
    {
        if (Interlocked.Exchange(ref asking, 1) == 1)
            return;
        _ = Task.Run(() =>
        {
            nint swapchain = 0;
            try
            {
                swapchain = (nint)player.Swapchain();
                if (swapchain != 0)
                    Marshal.AddRef(swapchain);
            }
            catch (Exception)
            {
                swapchain = 0;
            }
            if (!ui.TryEnqueue(() => Answer(swapchain, bind)))
            {
                if (swapchain != 0)
                    Marshal.Release(swapchain);
                Volatile.Write(ref asking, 0);
            }
        });
    }

    void Answer(nint swapchain, Action<nint> bind)
    {
        Volatile.Write(ref asking, 0);
        if (swapchain == 0)
            return;
        try
        {
            bind(swapchain);
        }
        finally
        {
            Marshal.Release(swapchain);
        }
    }
}
