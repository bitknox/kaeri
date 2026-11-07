using Medium = Kaeri.Engine.Core.Medium;
using Microsoft.UI.Xaml.Automation.Peers;
using System.Text;
using System.Text.Json;
using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.Web.WebView2.Core;

namespace Kaeri.Windows.Pages;

/// <summary>
/// A light novel in a WebView2. The book's documents come from the engine over the
/// <c>kaeri-book://</c> scheme, answered in WebResourceRequested; nothing else loads, and the
/// engine's answers carry the content security policy. The paginator script lays each
/// document out in pages and talks to the reader in JSON messages.
/// </summary>
sealed class NovelView : Grid
{
    const string Scheme = "kaeri-book";

    static Task<CoreWebView2Environment>? environment;

    readonly ReaderOverlay owner;
    readonly WebView2 web;
    readonly Grid endLayer = new() { Visibility = Visibility.Collapsed };
    readonly Border note;
    readonly TextBlock noteText;
    NovelOpenView? book;
    bool ready, failed;
    CoreWebView2Environment? made;
    (string Url, string? Fragment)? pendingLoad;
    string currentUrl = "";

    /// <summary>The document's page and how many it has, as the paginator last said.</summary>
    public int Page { get; private set; }
    public int Pages { get; private set; } = 1;
    public bool EndShown => endLayer.Visibility == Visibility.Visible;

    public NovelView(ReaderOverlay owner)
    {
        this.owner = owner;
        web = new WebView2 { DefaultBackgroundColor = owner.NovelGround };
        Children.Add(web);
        Children.Add(endLayer);
        noteText = new TextBlock { FontSize = 15, TextWrapping = TextWrapping.WrapWholeWords, MaxLines = 8, TextTrimming = TextTrimming.WordEllipsis };
        note = new Border
        {
            Child = noteText,
            Padding = new Thickness(16),
            MaxWidth = 520,
            CornerRadius = new CornerRadius(8),
            VerticalAlignment = VerticalAlignment.Bottom,
            HorizontalAlignment = HorizontalAlignment.Center,
            Margin = new Thickness(16, 0, 16, 120),
            Visibility = Visibility.Collapsed,
        };
        note.Tapped += (_, e) =>
        {
            Motion.Hide(note);
            e.Handled = true;
        };
        Children.Add(note);
        _ = Start();
    }

    ReaderSession Reader => owner.Reader;

    static Task<CoreWebView2Environment> Environment() => environment ??= Make();

    /// <summary>
    /// The books' own environment, apart from the trailers': the custom scheme has to be
    /// registered when the environment is made.
    /// </summary>
    static async Task<CoreWebView2Environment> Make()
    {
        var folder = Path.Combine(Path.GetTempPath(), "kaeri-windows", "webview2-books");
        var options = new CoreWebView2EnvironmentOptions();
        var scheme = new CoreWebView2CustomSchemeRegistration(Scheme)
        {
            // The WinRT projection takes the flag as a BOOL.
            TreatAsSecure = 1,
            HasAuthorityComponent = true,
        };
        options.CustomSchemeRegistrations.Add(scheme);
        return await CoreWebView2Environment.CreateWithOptionsAsync(null, folder, options);
    }

    async Task Start()
    {
        try
        {
            var env = made = await Environment();
            var controller = env.CreateCoreWebView2ControllerOptions();
            controller.IsInPrivateModeEnabled = true;
            await web.EnsureCoreWebView2Async(env, controller);
            var core = web.CoreWebView2;
            var settings = core.Settings;
            settings.AreDefaultContextMenusEnabled = false;
            settings.AreDevToolsEnabled = false;
            settings.AreBrowserAcceleratorKeysEnabled = false;
            settings.IsStatusBarEnabled = false;
            settings.IsZoomControlEnabled = false;
            settings.IsPinchZoomEnabled = false;
            settings.IsSwipeNavigationEnabled = false;
            settings.IsPasswordAutosaveEnabled = false;
            settings.IsGeneralAutofillEnabled = false;
            settings.IsWebMessageEnabled = true;
            core.NewWindowRequested += (_, e) => e.Handled = true;
            core.DownloadStarting += (_, e) =>
            {
                e.Cancel = true;
                // A book document the web view would rather save than show never paints: say so.
                if (e.DownloadOperation.Uri.StartsWith(Scheme + "://", StringComparison.OrdinalIgnoreCase))
                    Failed("a book document came as a download");
            };
            core.PermissionRequested += (_, e) => e.State = CoreWebView2PermissionState.Deny;
            core.NavigationStarting += (_, e) =>
            {
                // Only the book's own documents: a link out of it is never followed.
                if (!e.Uri.StartsWith(Scheme + "://", StringComparison.OrdinalIgnoreCase) && e.Uri != "about:blank")
                    e.Cancel = true;
            };
            core.NavigationCompleted += (_, e) =>
            {
                currentUrl = core.Source;
                // A document that didn't load never says ready, and the reader would wait on it.
                if (!e.IsSuccess && e.WebErrorStatus != CoreWebView2WebErrorStatus.OperationCanceled)
                    Failed($"a book document didn't load: {e.WebErrorStatus}");
            };
            // Every request comes here: the book's from the engine, anything else refused.
            core.AddWebResourceRequestedFilter("*", CoreWebView2WebResourceContext.All);
            core.WebResourceRequested += OnResource;
            var assets = KaeriFfiMethods.NovelAssets();
            await core.AddScriptToExecuteOnDocumentCreatedAsync(assets.Script);
            await core.AddScriptToExecuteOnDocumentCreatedAsync(HostScript);
            core.WebMessageReceived += (_, e) =>
            {
                string? text;
                try
                {
                    text = e.TryGetWebMessageAsString();
                }
                catch (ArgumentException)
                {
                    return;
                }
                if (text is not null)
                    Heard(text);
            };
            ready = true;
            if (pendingLoad is { } load)
            {
                pendingLoad = null;
                Load(load.Url, load.Fragment);
            }
        }
        catch (Exception e)
        {
            failed = true;
            Failed($"the novel's web view failed: {e.GetType().Name}");
        }
    }

    /// <summary>The chapter can't show: the failed screen, with its way to try again.</summary>
    void Failed(string why)
    {
        KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", why);
        owner.NovelFailed();
    }

    /// <summary>
    /// What the reader itself needs from inside the page: the keys that leave or go full screen,
    /// pointer movement for the group, and the wheel, which turns pages. The paginator handles
    /// the turning keys.
    /// </summary>
    const string HostScript = """
        (function () {
          if (window.top !== window) return;
          if (!/^kaeri-book:/.test(location.href)) return;
          var post = function (m) { try { window.chrome.webview.postMessage(JSON.stringify(m)); } catch (e) {} };
          document.addEventListener('keydown', function (e) {
            var k = e.key;
            if (k === 'Escape' || k === 'F11' || (e.altKey && k === 'ArrowLeft') || (e.ctrlKey && (k === '+' || k === '-' || k === '=' || k === '0'))) {
              e.preventDefault();
              post({ type: 'hostkey', key: k, alt: e.altKey, ctrl: e.ctrlKey, shift: e.shiftKey });
            }
          }, true);
          var last = 0;
          document.addEventListener('mousemove', function () {
            var t = Date.now();
            if (t - last > 200) { last = t; post({ type: 'hostmove' }); }
          }, true);
          window.addEventListener('wheel', function (e) {
            if (e.ctrlKey) { e.preventDefault(); return; }
            post({ type: 'hostwheel', dy: e.deltaY });
          }, { passive: false, capture: true });
        })();
        """;

    async void OnResource(CoreWebView2 sender, CoreWebView2WebResourceRequestedEventArgs e)
    {
        var uri = e.Request.Uri;
        if (made is not { } env)
            return;
        if (!uri.StartsWith(Scheme + "://", StringComparison.OrdinalIgnoreCase))
        {
            e.Response = env.CreateWebResourceResponse(null, 403, "Forbidden", "");
            return;
        }
        var deferral = e.GetDeferral();
        try
        {
            var ask = Reader.BookResource;
            var (answer, bytes) = await Task.Run(() =>
            {
                var res = ask?.Invoke(uri);
                byte[]? body = res?.Body switch
                {
                    BookBody.Bytes b => b.Data,
                    BookBody.File f => ReadFile(f.Path),
                    _ => null,
                };
                return (res, body);
            });
            if (answer is null)
            {
                e.Response = env.CreateWebResourceResponse(null, 404, "Not Found", "");
                return;
            }
            var stream = bytes is null ? null : new MemoryStream(bytes).AsRandomAccessStream();
            e.Response = env.CreateWebResourceResponse(stream, answer.Status, Reason(answer.Status), ReaderSession.ResponseHeaders(answer));
        }
        catch (Exception ex)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"a book resource failed: {ex.GetType().Name}");
            e.Response = env.CreateWebResourceResponse(null, 500, "Internal Server Error", "");
        }
        finally
        {
            deferral.Complete();
        }
    }

    static byte[]? ReadFile(string path)
    {
        try
        {
            return File.ReadAllBytes(path);
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            return null;
        }
    }

    static string Reason(int status) => status switch
    {
        200 => "OK",
        206 => "Partial Content",
        304 => "Not Modified",
        403 => "Forbidden",
        404 => "Not Found",
        _ => "",
    };

    // Opening

    /// <summary>The engine opened a book's chapter: load its entry, or the place it restores.</summary>
    public void Open(NovelOpenView open)
    {
        book = open;
        HideEnd();
        Page = 0;
        Pages = 1;
        if (open.Restore is { } at && at.Spine < open.Spine.Length)
            Load(open.Spine[(int)at.Spine].Url, "kaeri:pos=" + PosText(at));
        else
            Load(open.Entry, null);
    }

    public void Ground(global::Windows.UI.Color colour) => web.DefaultBackgroundColor = colour;

    void Load(string url, string? fragment)
    {
        if (failed)
            return;
        if (!ready)
        {
            pendingLoad = (url, fragment);
            return;
        }
        var bare = url.Split('#')[0];
        var target = fragment is null ? url : bare + "#" + fragment;
        currentUrl = bare;
        web.CoreWebView2.Navigate(target);
    }

    static string PosText(NovelPos pos) => $"{pos.Spine}/{string.Join('.', pos.Path)}/{pos.Offset}";

    /// <summary>The index in the spine of the document showing.</summary>
    int? CurrentSpine
    {
        get
        {
            if (book is null)
                return null;
            var url = (ready ? web.CoreWebView2?.Source : null) ?? currentUrl;
            var bare = url.Split('#')[0];
            for (var i = 0; i < book.Spine.Length; i++)
                if (book.Spine[i].Url == bare)
                    return i;
            return null;
        }
    }

    void Call(string script)
    {
        if (ready && !failed)
            _ = web.CoreWebView2.ExecuteScriptAsync(script);
    }

    /// <summary>The engine's style for the person's choices.</summary>
    public void ApplyStyle(string style)
    {
        if (string.IsNullOrEmpty(style))
            return;
        Call($"window.kaeriReader && window.kaeriReader.configure({style})");
    }

    // Turning

    public void Turn(bool forward, string how)
    {
        if (EndShown)
        {
            if (!forward)
                HideEnd();
            else if ((Reader.End?.Next ?? Reader.View?.Next) is { } next)
                Reader.OpenChapter(next.Id);
            return;
        }
        Call($"window.kaeriReader && window.kaeriReader.{(forward ? "next" : "prev")}('{how}')");
        owner.Turned();
    }

    public void Ends(bool last)
    {
        if (EndShown)
            return;
        Call($"window.kaeriReader && window.kaeriReader.goTo({(last ? Math.Max(Pages - 1, 0) : 0)}, 'key')");
    }

    /// <summary>A page of the document showing, counted from 0: the scrubber.</summary>
    public void JumpTo(int page)
    {
        HideEnd();
        Call($"window.kaeriReader && window.kaeriReader.goTo({Math.Clamp(page, 0, Math.Max(Pages - 1, 0))}, 'jump')");
    }

    /// <summary>A contents entry or a place the engine names.</summary>
    public void GoToUrl(string url)
    {
        HideEnd();
        var parts = url.Split('#', 2);
        Load(parts[0], parts.Length > 1 ? parts[1] : null);
    }

    public void GoTo(NovelPos pos)
    {
        if (book is null)
            return;
        HideEnd();
        if (CurrentSpine == (int)pos.Spine)
            Call($"window.kaeriReader && window.kaeriReader.goToPos('{PosText(pos)}', 'jump')");
        else if (pos.Spine < book.Spine.Length)
            Load(book.Spine[(int)pos.Spine].Url, "kaeri:pos=" + PosText(pos));
    }

#if KAERI_TESTING
    /// <summary>Shots: the end page, as passing the last page shows it.</summary>
    internal void ShotEnd() => ShowEnd();
#endif

    void ShowEnd()
    {
        endLayer.Children.Clear();
        endLayer.Children.Add(owner.BuildEndPage());
        if (Motion.Enabled && !owner.PaperMode)
            Motion.Show(endLayer);
        else
            Motion.Set(endLayer, true);
        owner.PositionChanged();
    }

    public void RefreshEnd()
    {
        if (!EndShown)
            return;
        endLayer.Children.Clear();
        endLayer.Children.Add(owner.BuildEndPage());
    }

    void HideEnd()
    {
        if (!EndShown)
            return;
        Motion.Set(endLayer, false);
        endLayer.Children.Clear();
        owner.PositionChanged();
    }

    // Hearing

    void Heard(string text)
    {
        JsonDocument json;
        try
        {
            json = JsonDocument.Parse(text);
        }
        catch (JsonException)
        {
            return;
        }
        using (json)
        {
            var root = json.RootElement;
            if (root.ValueKind != JsonValueKind.Object || !root.TryGetProperty("type", out var typeEl))
                return;
            switch (typeEl.GetString())
            {
                case "ready":
                case "page":
                    Paged(root);
                    break;
                case "layout":
                    Page = Int(root, "page");
                    Pages = Math.Max(Int(root, "pages"), 1);
                    owner.PositionChanged();
                    break;
                case "end":
                    Next();
                    break;
                case "start":
                    Previous();
                    break;
                case "tap":
                    if (root.TryGetProperty("x", out var x) && x.ValueKind == JsonValueKind.Number)
                        Tap(x.GetDouble());
                    break;
                case "link":
                    Link(root);
                    break;
                case "key":
                    if (root.TryGetProperty("key", out var key))
                        owner.PageKey(key.GetString() ?? "");
                    break;
                case "hostkey":
                    owner.HostKey(Str(root, "key"), Bool(root, "alt"), Bool(root, "ctrl"), Bool(root, "shift"));
                    break;
                case "hostmove":
                    owner.PointerActive();
                    break;
                case "hostwheel":
                    if (root.TryGetProperty("dy", out var dy) && dy.ValueKind == JsonValueKind.Number)
                        owner.NovelWheel(dy.GetDouble());
                    break;
                case "failed":
                    KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"the paginator failed: {Str(root, "why")}");
                    break;
            }
        }
    }

    void Paged(JsonElement root)
    {
        Page = Int(root, "page");
        Pages = Math.Max(Int(root, "pages"), 1);
        if (!root.TryGetProperty("pos", out var pos) || pos.ValueKind != JsonValueKind.Object)
            return;
        var path = new List<uint>();
        if (pos.TryGetProperty("path", out var p) && p.ValueKind == JsonValueKind.Array)
            foreach (var item in p.EnumerateArray())
                if (item.TryGetUInt32(out var v))
                    path.Add(v);
        var place = new NovelPos((uint)Int(pos, "spine"), (uint)Int(pos, "offset"), [.. path]);
        var how = Str(root, "how") switch
        {
            "swipe" => TurnHow.Swipe,
            "key" => TurnHow.Key,
            "scroll" => TurnHow.Scroll,
            "turn" => TurnHow.Turn,
            _ => TurnHow.Jump,
        };
        Reader.NovelAt(place, Page, Pages, how);
        owner.PositionChanged();
        owner.NovelPaintedNow();
    }

    /// <summary>Past the last page of a document: the next one in the spine, or the end page.</summary>
    void Next()
    {
        if (book is null || CurrentSpine is not { } at)
            return;
        for (var i = at + 1; i < book.Spine.Length; i++)
        {
            if (book.Spine[i].Linear)
            {
                Load(book.Spine[i].Url, null);
                return;
            }
        }
        ShowEnd();
    }

    void Previous()
    {
        if (book is null || CurrentSpine is not { } at)
            return;
        for (var i = at - 1; i >= 0; i--)
        {
            if (book.Spine[i].Linear)
            {
                Load(book.Spine[i].Url, "kaeri:end");
                return;
            }
        }
    }

    void Tap(double x)
    {
        var layout = Reader.Choice(ReaderSettingKey.TapLayout) ?? "edges";
        if (EndShown)
        {
            HideEnd();
            return;
        }
        if (owner.ChromeShown || layout == "off")
        {
            owner.ToggleChrome();
            return;
        }
        var forwardIsLeft = book?.Progression == ReadDirection.RightToLeft;
        var edge = layout == "sides" ? 0.5 : 0.33;
        if (x < edge || x > 1 - edge || layout == "sides")
            Turn((x < 0.5) == forwardIsLeft, "turn");
        else
            owner.ToggleChrome();
    }

    /// <summary>Only links inside the book are followed; a footnote shows over the page.</summary>
    void Link(JsonElement root)
    {
        var noteWords = Str(root, "note");
        if (noteWords.Length > 0)
        {
            noteText.Text = noteWords;
            noteText.Foreground = new SolidColorBrush(owner.Palette.Silk);
            note.Background = new SolidColorBrush(owner.Palette.Glass);
            Motion.Show(note);
            return;
        }
        if (Bool(root, "internal") && Str(root, "href") is { Length: > 0 } href && href.StartsWith(Scheme + "://", StringComparison.OrdinalIgnoreCase))
        {
            var parts = href.Split('#', 2);
            Load(parts[0], parts.Length > 1 ? parts[1] : null);
        }
    }

    /// <summary>The direction pages run: right to left for a vertical Japanese book.</summary>
    public bool Rtl => book?.Progression == ReadDirection.RightToLeft;

    public NovelTocEntry[] Contents => book?.Toc ?? [];

    static int Int(JsonElement e, string name) =>
        e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number && v.TryGetInt32(out var n) ? n : 0;

    static string Str(JsonElement e, string name) =>
        e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() ?? "" : "";

    static bool Bool(JsonElement e, string name) =>
        e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.True;

    public void Close()
    {
        try
        {
            web.Close();
        }
        catch (Exception)
        {
            // Already gone with the window.
        }
    }
}
