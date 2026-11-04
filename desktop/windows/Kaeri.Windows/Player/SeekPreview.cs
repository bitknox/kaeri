using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Player;

/// <summary>
/// The frame under the pointer on the timeline, from a second mpv that is never heard: video
/// only, paused, no audio or subtitles, opened on the first hover of a file and kept for the
/// next ones, closed with the file. Seeks are to keyframes, and the newest waits its turn while
/// mpv works on the last. The other apps do the same (Apple's <c>SeekPreview</c>).
/// </summary>
sealed class SeekPreview : IDisposable
{
    /// <summary>The preview's own playback number: the engine knows no such playback and drops its reports.</summary>
    const ulong Playback = 0x7FFF_0000_0001;

    sealed class Quiet : FfiPlayerSink
    {
        public void Event(FfiPlayerEvent e)
        {
        }
    }

    readonly FfiEngine engine;
    readonly SwapChainPanel panel;
    readonly DispatcherQueueTimer pump;
    readonly SwapchainAsk ask;
    FfiPlayer? player;
    PreviewSource? source;
    nint bound;
    double? target;
    bool opened;
    DateTime lastSeek;

    readonly string? cacheDir;

    public SeekPreview(FfiEngine engine, SwapChainPanel panel, DispatcherQueue queue, string? cacheDir)
    {
        this.engine = engine;
        this.cacheDir = cacheDir;
        this.panel = panel;
        ask = new SwapchainAsk(queue);
        pump = queue.CreateTimer();
        pump.Interval = TimeSpan.FromMilliseconds(80);
        pump.Tick += (_, _) => Pump();
    }

    /// <summary>Whether a player is up for previews at all.</summary>
    public bool Alive => player is not null;

    /// <summary>Show the frame at <paramref name="seconds"/> of <paramref name="from"/>.</summary>
    public void Show(double seconds, PreviewSource from, double scale)
    {
        if (player is null || source != from)
            Open(from, seconds, scale);
        target = seconds;
        if (!pump.IsRunning)
            pump.Start();
    }

    void Open(PreviewSource from, double seconds, double scale)
    {
        Close();
        try
        {
            // As Android's preview: software decoding the fast way, and almost no buffer, since it
            // only ever wants the keyframe under the pointer. With mpv's defaults a stream's
            // preview held about 900 MB beside the player's own. The shaders come from the
            // player's cache, so they are not compiled again for each file.
            player = new FfiPlayer(new FfiPlayerOptions(
                Video: new FfiVideo.Composition((uint)(160 * scale), (uint)(90 * scale)),
                Hwdec: "no",
                CacheDir: cacheDir,
                AudioOutput: "null",
                LogLevel: "error",
                Extra:
                [
                    new MpvOption("aid", "no"), new MpvOption("sid", "no"), new MpvOption("hr-seek", "no"),
                    new MpvOption("osd-level", "0"), new MpvOption("keep-open", "yes"), new MpvOption("pause", "yes"),
                    new MpvOption("vd-lavc-threads", "2"), new MpvOption("vd-lavc-fast", "yes"),
                    new MpvOption("vd-lavc-skiploopfilter", "all"), new MpvOption("demuxer-readahead-secs", "0"),
                    new MpvOption("demuxer-max-bytes", "4MiB"), new MpvOption("demuxer-max-back-bytes", "0"),
                ]), engine, new Quiet());
            player.CompositionSize((uint)(160 * scale), (uint)(90 * scale));
            player.Display((uint)(160 * scale), (uint)(90 * scale), scale, null);
            source = from;
            MpvOption[] options = [.. from.Options, new MpvOption("start", seconds.ToString(System.Globalization.CultureInfo.InvariantCulture))];
            player.Apply(Playback, [new PlayerOp.Open(false, null, null), new PlayerOp.Load(from.Url, options)]);
            opened = true;
            lastSeek = DateTime.UtcNow;
        }
        catch (Exception e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"the seek preview did not open: {e.Message}");
            Close();
        }
    }

    void Pump()
    {
        if (player is null)
        {
            pump.Stop();
            return;
        }
        Bind();
        if (target is { } seconds && opened && DateTime.UtcNow - lastSeek > TimeSpan.FromMilliseconds(120))
        {
            target = null;
            lastSeek = DateTime.UtcNow;
            try
            {
                player.Apply(Playback, [new PlayerOp.Seek(seconds, true)]);
            }
            catch (Exception)
            {
                // Closed under it: the next hover opens again.
            }
        }
    }

    /// <summary>Hand mpv's swapchain to the panel once it has made one, asked off the UI thread.</summary>
    void Bind()
    {
        if (player is not { } asked)
            return;
        ask.Ask(asked, swapchain =>
        {
            if (player != asked || swapchain == bound)
                return;
            try
            {
                Dxgi.SetSwapChain(panel, swapchain);
                bound = swapchain;
            }
            catch (Exception e)
            {
                KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"binding the preview's swapchain failed: {e.Message}");
            }
        });
    }

    /// <summary>Done with this file, or with previews: that mpv shuts down.</summary>
    public void Close()
    {
        pump.Stop();
        target = null;
        opened = false;
        bound = 0;
        source = null;
        if (player is { } old)
        {
            player = null;
            try
            {
                old.Shutdown();
                old.Dispose();
            }
            catch (Exception)
            {
                // Already gone.
            }
        }
    }

    public void Dispose() => Close();
}
