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
    const string Scheme = TrailerHub.BookScheme;

    readonly ReaderOverlay owner;
    readonly WebView2 web;
    readonly Grid endLayer = new() { Visibility = Visibility.Collapsed };
    readonly Border note;
    readonly TextBlock noteText;
    readonly Button noteClose;
    NovelOpenView? book;
    bool ready, failed, closed, resting;
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
        noteText = new TextBlock { FontSize = 15, TextWrapping = TextWrapping.WrapWholeWords, MaxLines = 8, TextTrimming = TextTrimming.WordEllipsis, VerticalAlignment = VerticalAlignment.Center };
        noteClose = new Button
        {
            Content = new FontIcon { Glyph = "\uE711", FontSize = 12 },
            Width = 32,
            Height = 32,
            Padding = new Thickness(0),
            CornerRadius = new CornerRadius(4),
            BorderThickness = new Thickness(0),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            VerticalAlignment = VerticalAlignment.Top,
        };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(noteClose, T.WindowsReaderNoteClose);
        ToolTipService.SetToolTip(noteClose, T.TooltipWithKey(T.WindowsReaderNoteClose, "Esc"));
        noteClose.Click += (_, _) => CloseNote();
        var noteRow = new Grid { ColumnSpacing = 8 };
        noteRow.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        noteRow.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        noteRow.Children.Add(noteText);
        Grid.SetColumn(noteClose, 1);
        noteRow.Children.Add(noteClose);
        note = new Border
        {
            Child = noteRow,
            Padding = new Thickness(16, 12, 8, 12),
            MaxWidth = 520,
            CornerRadius = new CornerRadius(8),
            VerticalAlignment = VerticalAlignment.Bottom,
            HorizontalAlignment = HorizontalAlignment.Center,
            Margin = new Thickness(16, 0, 16, 120),
            Visibility = Visibility.Collapsed,
        };
        note.Tapped += (_, e) =>
        {
            CloseNote();
            e.Handled = true;
        };
        Children.Add(note);
        _ = Start();
    }

    ReaderSession Reader => owner.Reader;

    /// <summary>The web view didn't start or its process went: this view shows nothing more.</summary>
    public bool Broken => failed;

    async Task Start()
    {
        try
        {
            // The trailers' environment, so one browser process serves both; the books get a
            // private profile of their own, apart from the trailers' session.
            var env = made = await TrailerHub.Environment();
            if (closed)
                return;
            var controller = env.CreateCoreWebView2ControllerOptions();
            controller.IsInPrivateModeEnabled = true;
            controller.ProfileName = "books";
            await web.EnsureCoreWebView2Async(env, controller);
            // The reader closed while the web view was being made: nothing of it is shown.
            if (closed)
            {
                CloseWeb();
                return;
            }
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
            // A renderer that crashed or hung leaves a blank page: the failed screen, and Try
            // again opens the chapter in a new web view.
            core.ProcessFailed += (_, e) => Lost($"its process failed: {e.ProcessFailedKind}");
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
                    Failed($"a book document didn't load: {e.WebErrorStatus} (host {HostOf(core.Source)})");
            };
            // Every request comes here: the book's from the engine, anything else refused.
            core.AddWebResourceRequestedFilter("*", CoreWebView2WebResourceContext.All);
            core.WebResourceRequested += OnResource;
            var assets = KaeriFfiMethods.NovelAssets();
            // The paginator reads its settings when it starts, so they come first: the top row's
            // room, so the first line clears the chrome.
            await core.AddScriptToExecuteOnDocumentCreatedAsync(SetupScript);
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
            if (closed)
            {
                CloseWeb();
                return;
            }
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
            if (!closed)
                Failed($"the novel's web view failed: {e.GetType().Name}");
        }
    }

    /// <summary>The chapter can't show: the failed screen, with its way to try again.</summary>
    void Failed(string why)
    {
        KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", why);
        if (!closed)
            owner.NovelFailed(this);
    }

    /// <summary>The web view's process is gone: this view is done, and the reader makes another.</summary>
    void Lost(string why)
    {
        if (closed || failed)
            return;
        failed = true;
        KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", "the novel's web view " + why);
        owner.NovelLost(this);
    }

    const string SetupScript = "window.kaeriSetup = Object.assign(window.kaeriSetup || {}, { top: 76 });";

    /// <summary>
    /// What the reader itself needs from inside the page: the keys that leave or go full screen,
    /// Tab, which brings the chrome up and goes into it, pointer movement for the group, and the
    /// wheel, which turns pages. The paginator handles
    /// the turning keys.
    /// </summary>
    const string HostScript = """
        (function () {
          if (window.top !== window) return;
          if (!/^kaeri-book:/.test(location.href)) return;
          var post = function (m) { try { window.chrome.webview.postMessage(JSON.stringify(m)); } catch (e) {} };
          document.addEventListener('keydown', function (e) {
            var k = e.key;
            if (k === 'Escape' || k === 'F11' || (k === 'Tab' && !e.ctrlKey && !e.altKey) || (e.altKey && k === 'ArrowLeft') || (e.ctrlKey && (k === '+' || k === '-' || k === '=' || k === '0'))) {
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
            var (answer, body) = await Task.Run(() =>
            {
                var res = ask?.Invoke(uri);
                // A file (a font, an image) is read as the web view asks for it, not all at once.
                Stream? stream = res?.Body switch
                {
                    BookBody.Bytes b => new MemoryStream(b.Data),
                    BookBody.File f => OpenFile(f.Path),
                    _ => null,
                };
                return (res, stream);
            });
            if (answer is null || closed)
            {
                // The host alone: it names the session, and a changed one (a web view that
                // rewrote it) is what leaves a book blank.
                if (answer is null && !closed)
                    KaeriFfiMethods.LogLine(FfiLogLevel.Info, "windows", $"no book resource for host {HostOf(uri)}");
                body?.Dispose();
                e.Response = env.CreateWebResourceResponse(null, 404, "Not Found", "");
                return;
            }
            var stream = body?.AsRandomAccessStream();
            e.Response = env.CreateWebResourceResponse(stream, answer.Status, Reason(answer.Status), ReaderSession.ResponseHeaders(answer));
        }
        catch (Exception ex)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"a book resource failed: {ex.GetType().Name}");
            if (!closed)
                e.Response = env.CreateWebResourceResponse(null, 500, "Internal Server Error", "");
        }
        finally
        {
            try
            {
                deferral.Complete();
            }
            catch (Exception) when (closed)
            {
                // The web view went while the engine answered: nobody waits on the request.
            }
        }
    }

    static string HostOf(string uri) => Uri.TryCreate(uri, UriKind.Absolute, out var at) ? at.Host : "?";

    static Stream? OpenFile(string path)
    {
        try
        {
            return new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read, 64 * 1024, useAsync: false);
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

    /// <summary>
    /// The engine opened a book's chapter: load its entry, or the place it restores. A view made
    /// again after the last one's process went opens at <paramref name="shown"/>, where that one was.
    /// </summary>
    public void Open(NovelOpenView open, NovelPos? shown = null)
    {
        book = open;
        HideEnd();
        Page = 0;
        Pages = 1;
        if ((shown ?? open.Restore) is { } at && at.Spine < open.Spine.Length)
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

    /// <summary>
    /// How the document loading now came: a turn past a document's edge, or a contents entry or
    /// link the person picked. Anything else loaded is placed.
    /// </summary>
    TurnHow? readyHow;

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
        if (ready && !failed && !closed)
            _ = web.CoreWebView2.ExecuteScriptAsync(script);
    }

    /// <summary>
    /// The window was minimised (true) or came back: hidden, the page asks for little memory and
    /// its scripts are suspended; shown, it resumes where it was.
    /// </summary>
    public async void Rest(bool hidden)
    {
        if (!ready || failed || closed || hidden == resting || web.CoreWebView2 is not { } core)
            return;
        resting = hidden;
        try
        {
            if (hidden)
            {
                core.MemoryUsageTargetLevel = CoreWebView2MemoryUsageTargetLevel.Low;
                web.Visibility = Visibility.Collapsed;
                await core.TrySuspendAsync();
            }
            else
            {
                if (core.IsSuspended)
                    core.Resume();
                core.MemoryUsageTargetLevel = CoreWebView2MemoryUsageTargetLevel.Normal;
                web.Visibility = Visibility.Visible;
            }
        }
        catch (Exception e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Debug, "windows", $"the novel's web view did not rest: {e.GetType().Name}");
        }
    }

    // The footnote

    /// <summary>The footnote over the page is up.</summary>
    public bool NoteShown { get; private set; }

    /// <summary>The footnote goes, and the keys go back to the page.</summary>
    public bool CloseNote()
    {
        if (!NoteShown)
            return false;
        NoteShown = false;
        SyncInput();
        if (owner.PaperMode)
            Motion.Set(note, false);
        else
            Motion.Hide(note);
        owner.Focus(FocusState.Programmatic);
        return true;
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
            else if (Reader.EndNext is { } next)
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
        // The person's jump: the document reports it as one once it is ready.
        readyHow = TurnHow.Jump;
        var parts = url.Split('#', 2);
        Load(parts[0], parts.Length > 1 ? parts[1] : null);
    }

    public void GoTo(NovelPos pos)
    {
        if (book is null)
            return;
        HideEnd();
        readyHow = null;
        if (CurrentSpine == (int)pos.Spine)
            Call($"window.kaeriReader && window.kaeriReader.goToPos('{PosText(pos)}', 'placed')");
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
        // A fade, which moves nothing, stays with reduced motion; Paper mode cuts.
        if (!owner.PaperMode)
            Motion.Show(endLayer);
        else
            Motion.Set(endLayer, true);
        SyncInput();
        owner.PositionChanged();
        owner.NovelTurned(this);
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
        SyncInput();
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
                    // A new document starts with no input of its own: it is told again.
                    inputSent = null;
                    Paged(root);
                    SyncInput();
                    break;
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
            "jump" => TurnHow.Jump,
            // Ready after a load: how it came, or the app's own move.
            _ => readyHow ?? TurnHow.Placed,
        };
        if (Str(root, "type") == "ready")
            readyHow = null;
        Reader.NovelAt(place, Page, Pages, how);
        owner.PositionChanged();
        owner.NovelPaintedNow(this);
        if (Str(root, "type") == "page")
            owner.NovelTurned(this);
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
                readyHow = TurnHow.Turn;
                Load(book.Spine[i].Url, null);
                return;
            }
        }
        // Past the last page: the engine counts the chapter read.
        Reader.NovelEnded();
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
                readyHow = TurnHow.Turn;
                Load(book.Spine[i].Url, "kaeri:end");
                return;
            }
        }
    }

    /// <summary>
    /// A tap the page did not turn on: the page turns itself on its edges (input), so what comes
    /// here is the middle, or any tap while the chrome, the end page or a note holds it.
    /// </summary>
    void Tap(double x)
    {
        if (EndShown)
        {
            HideEnd();
            return;
        }
        owner.ToggleChrome();
    }

    string? inputSent;

    /// <summary>
    /// How the page turns itself on a tap or a key, in one message (the paginator's input): the
    /// tap layout's edges, the side forward is on, the turning keys, and hold while the chrome,
    /// the end page or a footnote is up, when every tap and key comes to the reader instead.
    /// </summary>
    public void SyncInput()
    {
        var layout = Reader.Choice(ReaderSettingKey.TapLayout) ?? "edges";
        var edge = layout switch { "off" => "0", "sides" => "0.5", _ => "0.33" };
        var forward = book?.Progression == ReadDirection.RightToLeft ? "left" : "right";
        var hold = owner.ChromeShown || EndShown || NoteShown ? "true" : "false";
        var how = $"{{edge: {edge}, forward: '{forward}', keys: true, hold: {hold}}}";
        if (how == inputSent)
            return;
        inputSent = how;
        Call($"window.kaeriReader && window.kaeriReader.input && window.kaeriReader.input({how})");
    }

    /// <summary>Only links inside the book are followed; a footnote shows over the page.</summary>
    void Link(JsonElement root)
    {
        var noteWords = Str(root, "note");
        if (noteWords.Length > 0)
        {
            var palette = owner.PagePalette;
            noteText.Text = noteWords;
            noteText.Foreground = new SolidColorBrush(palette.Silk);
            noteClose.Foreground = new SolidColorBrush(palette.Silk);
            note.Background = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Raised, 0.97));
            note.BorderBrush = new SolidColorBrush(palette.Rule);
            note.BorderThickness = new Thickness(1);
            NoteShown = true;
            SyncInput();
            if (owner.PaperMode)
                Motion.Set(note, true);
            else
                Motion.Show(note);
            // Focus moves to the note, so Esc and Narrator find it; Narrator reads its words.
            noteClose.Focus(FocusState.Programmatic);
            ReaderSpeech.Say(note, noteWords, "kaeri-reader-note");
            return;
        }
        if (Bool(root, "internal") && Str(root, "href") is { Length: > 0 } href && href.StartsWith(Scheme + "://", StringComparison.OrdinalIgnoreCase))
        {
            var parts = href.Split('#', 2);
            readyHow = TurnHow.Jump;
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

    /// <summary>The reader let go of this view: its web view closes, now or once it is made.</summary>
    public void Close()
    {
        closed = true;
        CloseWeb();
    }

    void CloseWeb()
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
