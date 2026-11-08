using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.Web.WebView2.Core;

namespace Kaeri.Windows;

/// <summary>
/// One web view with the engine's trailer page in it, and its bridge. A preview's page is kept by
/// <see cref="TrailerPool"/> for the run and moved from place to place: from the pool's unseen
/// dock into a hover card and back, playing trailer after trailer through the engine's load
/// script, so a hover asks YouTube only for the video. The view takes no pointer or keyboard
/// input of its own, but for Play trailer's.
/// </summary>
sealed class TrailerPage : IWarmPage
{
    readonly List<string> queued = [];
    bool loaded, closed, asleep;
    int naps;
    string? folder;

    public WebView2 View { get; }
    public EmbedStyle Style { get; }
    public string Subtitles { get; }
    /// <summary>Kept by the pool when handed back.</summary>
    public bool Pooled { get; }
    public bool Broken { get; private set; }
    /// <summary>Who hears the page now: the trailer playing in it.</summary>
    public Action<string?>? Hear { get; set; }

    TrailerPage(EmbedStyle style, string subtitles, bool pooled)
    {
        Style = style;
        Subtitles = subtitles;
        Pooled = pooled;
        var full = style == EmbedStyle.Full;
        View = new WebView2
        {
            DefaultBackgroundColor = Colors.Transparent,
            IsHitTestVisible = full,
            IsTabStop = full,
        };
        AutomationProperties.SetAccessibilityView(View, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
    }

    /// <summary>
    /// A page with <paramref name="html"/> in it, put into <paramref name="parent"/> at
    /// <paramref name="index"/>: the view is made in place, as WebView2 is made in a window.
    /// </summary>
    public static TrailerPage Open(EmbedStyle style, string html, string subtitles, bool pooled, Panel parent, int index)
    {
        var page = new TrailerPage(style, subtitles, pooled);
        parent.Children.Insert(Math.Clamp(index, 0, parent.Children.Count), page.View);
        _ = page.Load(html);
        return page;
    }

    async Task Load(string html)
    {
        try
        {
            var env = await TrailerHub.Environment();
            await View.EnsureCoreWebView2Async(env, await TrailerHub.Private());
            if (closed)
                return;
            var core = View.CoreWebView2;
            var settings = core.Settings;
            settings.AreDefaultContextMenusEnabled = false;
            settings.AreDevToolsEnabled = false;
            settings.AreBrowserAcceleratorKeysEnabled = false;
            settings.IsStatusBarEnabled = false;
            settings.IsZoomControlEnabled = false;
            settings.IsSwipeNavigationEnabled = false;
            settings.IsPasswordAutosaveEnabled = false;
            settings.IsGeneralAutofillEnabled = false;
            core.NewWindowRequested += (_, e) => e.Handled = true;
            // In a preview YouTube's own controls are hidden in its frame; Play trailer keeps them.
            // WebView2 runs this as each document is created, in every frame, before it has a head
            // or even a root to put the style in, where Apple's user script runs at the document's
            // end; so it waits for the document to be parsed, as there. YouTube's frame stays for
            // every trailer the page plays after.
            if (Style == EmbedStyle.Banner)
                await core.AddScriptToExecuteOnDocumentCreatedAsync(
                    "(function () { var run = function () { " + KaeriFfiMethods.TrailerChromeScript() + " };"
                    + " if (document.readyState === 'loading') { document.addEventListener('DOMContentLoaded', run, { once: true }); }"
                    + " else { run(); } })();");
            core.WebMessageReceived += (_, e) => Hear?.Invoke(e.TryGetWebMessageAsString());
            core.NavigationCompleted += (sender, e) =>
            {
                if (closed)
                    return;
                if (!e.IsSuccess)
                {
                    Fail($"the page did not load: {e.WebErrorStatus}");
                    return;
                }
                loaded = true;
                foreach (var script in queued)
                    _ = core.ExecuteScriptAsync(script);
                queued.Clear();
            };
            core.ProcessFailed += (_, e) => Fail($"its process failed: {e.ProcessFailedKind}");
            // The page is served from the app's own origin, so YouTube sees it as the Referer.
            folder = Path.Combine(Path.GetTempPath(), "kaeri-windows", "trailers", Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(folder);
            await File.WriteAllTextAsync(Path.Combine(folder, "trailer.html"), html);
            if (closed)
                return;
            core.SetVirtualHostNameToFolderMapping(Trailers.Host, folder, CoreWebView2HostResourceAccessKind.Allow);
            core.Navigate($"{Trailers.Origin}/trailer.html");
        }
        catch (Exception e)
        {
            if (!closed)
                Fail($"the trailer's web view failed: {e.GetType().Name}");
        }
    }

    void Fail(string reason)
    {
        Broken = true;
        KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "trailer", reason);
        Hear?.Invoke("error:0");
    }

    /// <summary>Run <paramref name="script"/> in the page now, or once it has loaded: a hover can come before a page loaded ahead of it is up.</summary>
    public void Run(string script)
    {
        if (closed)
            return;
        if (!loaded || View.CoreWebView2 is not { } core)
        {
            queued.Add(script);
            return;
        }
        _ = core.ExecuteScriptAsync(script);
    }

    /// <summary>The script's answer, or null when the page is not up.</summary>
    public async Task<string?> Ask(string script)
    {
        if (closed || !loaded || View.CoreWebView2 is not { } core)
            return null;
        try
        {
            return await core.ExecuteScriptAsync(script);
        }
        catch (Exception)
        {
            return null;
        }
    }

    /// <summary>Tell the page to play, pause, mute or unmute; the web view's own sound follows the last two.</summary>
    public void Control(TrailerControl control)
    {
        if (control is TrailerControl.Mute or TrailerControl.Unmute && View.CoreWebView2 is { } core)
            core.IsMuted = control == TrailerControl.Mute;
        Run(KaeriFfiMethods.TrailerScript(control));
    }

    /// <summary>Into <paramref name="parent"/> at <paramref name="index"/>, out of wherever it was. The page and its player carry on.</summary>
    public void MoveTo(Panel parent, int index)
    {
        if (View.Parent is Panel was)
        {
            if (was == parent)
                return;
            was.Children.Remove(View);
        }
        parent.Children.Insert(Math.Clamp(index, 0, parent.Children.Count), View);
    }

    /// <summary>
    /// Waiting in the dock, paused: a moment after the pause has reached the player, the view is
    /// hidden, asks for less memory and is suspended, so a kept page costs little until a hover
    /// takes it.
    /// </summary>
    public async void Sleep()
    {
        if (closed || asleep)
            return;
        asleep = true;
        var nap = ++naps;
        await Task.Delay(TimeSpan.FromSeconds(1));
        if (closed || !asleep || nap != naps || View.CoreWebView2 is not { } core)
            return;
        try
        {
            core.MemoryUsageTargetLevel = CoreWebView2MemoryUsageTargetLevel.Low;
            View.Visibility = Visibility.Collapsed;
            await core.TrySuspendAsync();
            // Taken again while it was suspending.
            if (!asleep && !closed && core.IsSuspended)
                core.Resume();
        }
        catch (Exception)
        {
            // The view closed under it.
        }
    }

    /// <summary>Taken out of the dock: resumed, seen and back to its usual memory, before any script reaches it.</summary>
    public void Wake()
    {
        if (closed)
            return;
        asleep = false;
        naps++;
        View.Visibility = Visibility.Visible;
        if (View.CoreWebView2 is not { } core)
            return;
        try
        {
            if (core.IsSuspended)
                core.Resume();
            core.MemoryUsageTargetLevel = CoreWebView2MemoryUsageTargetLevel.Normal;
        }
        catch (Exception)
        {
            // The view closed under it.
        }
    }

    /// <summary>Let go of the view, its player and its page's folder.</summary>
    public void Close()
    {
        if (closed)
            return;
        closed = true;
        Hear = null;
        queued.Clear();
        (View.Parent as Panel)?.Children.Remove(View);
        try
        {
            View.Close();
        }
        catch (Exception)
        {
            // Already closed with its window.
        }
        if (folder is not null)
        {
            var dir = folder;
            folder = null;
            _ = Task.Run(() =>
            {
                try { Directory.Delete(dir, true); } catch (IOException) { } catch (UnauthorizedAccessException) { }
            });
        }
    }
}
