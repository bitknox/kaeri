using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

// Credit pages and parts in the reader: the pill on page 1, the pages shown in place of a page,
// the scrubber's place on them and its ticks for the parts.
public sealed partial class ReaderOverlay
{
    /// <summary>How long the pill stays on page 1 the first time, before it comes only with the chrome.</summary>
    static readonly TimeSpan PillHold = TimeSpan.FromSeconds(4);

    DispatcherQueueTimer? pillHold;
    /// <summary>The chapter whose pill Narrator heard: once a chapter.</summary>
    string? pillSaidFor;
    (string? Line, bool Shown) endLine;

    /// <summary>The ground under the leaves, for what stands in a page's place.</summary>
    internal Brush? StageGround => root.Background;

    /// <summary>The session's credit pages or parts changed: the views, the pill and the place follow.</summary>
    void CreditsChanged()
    {
        paged?.SyncCredit();
        strip?.RefreshCredits();
        // The end page's line, only when its words or whether the pages show changed.
        var line = (Reader.SkippedLine(true), Reader.ShownEnd);
        if (line != endLine)
        {
            endLine = line;
            paged?.RefreshEnd();
            if (strip is not null && !Reader.Flowing)
                strip.RefreshFoot();
        }
        Pill();
        PositionChanged();
    }

    /// <summary>
    /// The pill shows on page 1 while pages were skipped at its start: the first time for 4 s,
    /// said once, politely; after that with the chrome. It goes with a turn, a shown credit page
    /// or the chrome.
    /// </summary>
    void Pill()
    {
        var pill = chrome.CreditPill;
        var line = Reader.SkippedLine(false);
        var onFirst = kind == Kind.Paged && paged is { Current: 0 } && Reader.Leaves.Length > 0 && !Reader.Leaves[0].EndPage;
        // A turn away from page 1 takes the pill's first time with it.
        if (kind == Kind.Paged && paged is { Current: > 0 } && pillSaidFor is not null)
            Reader.CreditLineDone = true;
        var want = showing && onFirst && line is not null && Reader.CreditOn is null && !Reader.ShownStart && !StateShown
            && (chromeShown || !Reader.CreditLineDone);
        chrome.PillUnder(chromeShown);
        if (!want)
        {
            if (PaperMode)
                Motion.Set(pill, false);
            else
                Motion.Hide(pill);
            return;
        }
        chrome.PillWords(line!);
        if (PaperMode)
            Motion.Set(pill, true);
        else
            Motion.Show(pill);
        var chapter = Reader.View?.Chapter.Id;
        if (pillSaidFor == chapter)
            return;
        pillSaidFor = chapter;
        announcer.Text = line;
        ReaderSpeech.Raise(announcer);
        if (Reader.CreditLineDone)
            return;
        pillHold ??= Timers.Weak(this, PillHold, false, o => o.PillDone());
        pillHold.Stop();
        pillHold.Start();
    }

    void PillDone()
    {
        pillHold?.Stop();
        Reader.CreditLineDone = true;
        Pill();
    }

    /// <summary>The pill, the strip's row or the end page's line: the pages on that side show.</summary>
    internal void ShowCredits(bool atEnd)
    {
        pillHold?.Stop();
        var first = Reader.ShowCredits(atEnd);
        if (kind == Kind.Strip)
        {
            if (strip?.RevealCredits(atEnd) != true)
                Reader.CreditFailed(atEnd);
            return;
        }
        if (kind != Kind.Paged || first is null || !ShowCreditPage(first))
            Reader.CreditFailed(atEnd);
    }

    internal void ShowEndCredits() => ShowCredits(true);

    /// <summary>A credit page on screen, by the same crossfade as a turn. False when its file can't be read.</summary>
    bool ShowCreditPage(CreditPage page)
    {
        if (paged is null || !System.IO.File.Exists(page.File.Path))
            return false;
        Reader.ShowCredit(page);
        HideChrome();
        PositionChanged();
        Announce();
        return true;
    }

    int LastPageLeaf => Math.Max(0, Reader.ReadingLeaves - 1);

    Kaeri.App.CreditTurn StepFor(bool forward)
    {
        if (paged is null || Reader.Credits.Length == 0)
            return Kaeri.App.CreditTurn.None;
        var leaves = Reader.Leaves;
        var leaf = paged.Current;
        var onEnd = leaf >= 0 && leaf < leaves.Length && leaves[leaf].EndPage;
        var end = Array.FindIndex(leaves, l => l.EndPage);
        return ReaderSession.CreditStep(Reader.Credits, Reader.CreditOn, Reader.ShownStart, Reader.ShownEnd,
            leaf, onEnd, LastPageLeaf, end >= 0 ? end : null, forward);
    }

    /// <summary>Whether a turn that way goes onto or from a credit page, for a swipe to commit.</summary>
    internal bool CreditTurnWaits(bool forward) => StepFor(forward).Kind is not CreditTurnKind.None;

    /// <summary>
    /// A turn onto, between or off shown credit pages. True when it was one: back past the
    /// first start page goes nowhere, as back from page 1 does.
    /// </summary>
    internal bool CreditTurn(bool forward)
    {
        var step = StepFor(forward);
        switch (step.Kind)
        {
            case CreditTurnKind.None:
                return false;
            case CreditTurnKind.Credit:
                var page = Reader.Credits[step.At];
                if (!ShowCreditPage(page))
                    Reader.CreditFailed(page.AtEnd);
                return true;
            case CreditTurnKind.Leaf:
                paged!.Land(step.At);
                Turned();
                return true;
            case CreditTurnKind.Continue:
                Reader.LeaveCredit();
                if ((Reader.End?.Next ?? Reader.View?.Next) is { } next)
                    Reader.OpenChapter(next.Id);
                return true;
            default:
                return true;
        }
    }

    /// <summary>
    /// A credit page in the group's line and on the scrubber: "Chapter 87 · credit page", the
    /// thumb at that end and its number blank. A page kept with Undo still offered reads as one
    /// of the chapter's. False when no credit page is on screen.
    /// </summary>
    bool CreditPlace(string label, int total)
    {
        if (kind != Kind.Paged || Reader.CreditOn is not { } on)
            return false;
        var (at, all) = Reader.HeldPages(on.AtEnd ? total : 1, total);
        if (Reader.Held(on.Fingerprint))
        {
            chrome.Line(T.WindowsReaderGroupLine(label, at, all));
            chrome.Scrub(at, all, Rtl, null);
            return true;
        }
        var side = Reader.Credits.Where(c => c.AtEnd == on.AtEnd).ToList();
        var n = side.FindIndex(c => c.Fingerprint == on.Fingerprint);
        var place = side.Count > 1 ? T.ReaderCreditPageOf(n + 1, side.Count) : T.ReaderCreditPage;
        chrome.Line(T.ReaderGroupLinePlace(label, place));
        chrome.Scrub(at, all, Rtl, null, place);
        return true;
    }

    /// <summary>The scrubber's ticks for the open chapter's parts, or none.</summary>
    void PartTicks(int total) =>
        chrome.Ticks(kind != Kind.Novel && Reader.Parts is { } parts && total > 1 ? ReaderSession.Ticks(parts, total) : []);

    /// <summary>What the scrubber says at a part's start: "Part 4 starts at page 14".</summary>
    string? PartStatus(int at, int total) =>
        kind != Kind.Novel && Reader.Parts is { } parts ? ReaderSession.PartWords(parts, at, total) : null;

    /// <summary>The scrub preview's words on a tick: "Page 11 · part 2", or null for the number alone.</summary>
    internal string? PartTip(int value)
    {
        if (kind == Kind.Novel || Reader.Parts is not { } parts)
            return null;
        var total = kind == Kind.Paged ? Reader.PageCount : strip?.Position.Count ?? 0;
        return ReaderSession.TipWords(parts, value, total);
    }

    /// <summary>A credit page is what a screen reader hears, unless it is kept.</summary>
    bool CreditSaid(out string text)
    {
        text = T.ReaderCreditPageLabel;
        return kind == Kind.Paged && Reader.CreditOn is { } on && !Reader.Held(on.Fingerprint);
    }

#if KAERI_TESTING
    /// <summary>
    /// Shots: the credit pages and parts a source would send, for a chapter the fixtures open.
    /// The pages are drawn here (Lanternlight Scans, lanternlight.example) as PNGs in the temp
    /// folder, so no real credit page is ever in a shot.
    /// </summary>
    internal async void ShotCredits(string what)
    {
        if (Reader.View is not { } view || Reader.Leaves.Length == 0)
            return;
        var session = Reader.SessionId;
        var chapter = view.Chapter.Id;
        var start = await ShotCreditFile("start");
        var end = await ShotCreditFile("end");
        // reader-credit-two: two pages skipped at the start. reader-credit-failed: a page whose file is gone.
        var pages = new List<CreditPage> { new(0, start, 0x11) };
        if (what == "reader-credit-two")
            pages.Add(new CreditPage(1, await ShotCreditFile("second"), 0x33));
        if (what == "reader-credit-failed")
            pages[0] = new CreditPage(0, start with { Path = start.Path + ".gone" }, 0x11);
        pages.Add(new CreditPage((uint)Reader.PageCount + (uint)pages.Count, end, 0x22, true));
        Reader.Apply(new ReaderEvent.CreditPages(session, chapter, [.. pages],
            new Msg.Reading(new ReadingMsg.ReaderCreditPagesSkipped(pages.Count(p => !p.AtEnd)))));
        var total = (uint)Math.Max(Reader.PageCount, 1);
        Reader.Apply(new ReaderEvent.ChapterParts(session, chapter,
            [new ChapterPart(1, 1, "Lanternlight Scans", false), new ChapterPart(2, Math.Max(2, total * 2 / 5), "Lanternlight Scans", false),
             new ChapterPart(4, Math.Max(3, total * 3 / 4), "Saltmarsh Team", true)],
            4, [3]));
        switch (what)
        {
            case "reader-credit-pill":
                paged?.Jump(0);
                ShowChrome(stay: true);
                break;
            case "reader-credit-line":
                // Page 1 with the chrome down: the pill alone, in its first 4 s.
                paged?.Jump(0);
                HideChrome();
                break;
            case "reader-credit" or "reader-credit-two" or "reader-credit-failed":
                paged?.Jump(0);
                ShowCredits(false);
                break;
            case "reader-credit-chrome":
                paged?.Jump(0);
                ShowCredits(false);
                ShowChrome(stay: true);
                break;
            case "reader-credit-kept":
                paged?.Jump(0);
                ShowCredits(false);
                Reader.ActLocal(ReaderLocal.NotACredit);
                ShowChrome(stay: true);
                break;
            case "reader-parts":
                ShowChrome(stay: true);
                chrome.ShotPreview(Math.Max(2, (int)total * 2 / 5));
                break;
            case "reader-parts-missing":
                ShowChrome(stay: true);
                chrome.ShotPreview(Math.Max(3, (int)total * 3 / 4));
                break;
            case "reader-credit-end":
                if (paged is not null)
                    paged.Jump(Math.Max(0, Reader.Leaves.Length - 1));
                strip?.Ends(true);
                break;
            case "reader-strip-row":
                strip?.Ends(false);
                ShowChrome(stay: true);
                break;
            case "reader-strip-credits":
                strip?.Ends(false);
                ShowCredits(false);
                ShowChrome(stay: true);
                break;
            case "reader-strip-end":
                ShowCredits(true);
                break;
        }
    }

    async System.Threading.Tasks.Task<PageFile> ShotCreditFile(string side)
    {
        var path = System.IO.Path.Combine(System.IO.Path.GetTempPath(), $"kaeri-shot-credit-{side}.png");
        const int w = 900, h = 1280;
        if (!System.IO.File.Exists(path))
        {
            var lines = new Microsoft.UI.Xaml.Controls.StackPanel { Width = w, Height = h, Spacing = 18, Padding = new Thickness(80, 420, 80, 0), Background = new SolidColorBrush(Microsoft.UI.Colors.White) };
            var who = side == "second" ? "Saltmarsh Team" : "Lanternlight Scans"; // i18n-skip
            foreach (var (text, size) in new[] { (who, 64.0), ("Tidebell Cartographer", 34.0), (side == "second" ? "saltmarsh.example" : "lanternlight.example", 28.0) })
                lines.Children.Add(new Microsoft.UI.Xaml.Controls.TextBlock { Text = text, FontSize = size, HorizontalAlignment = HorizontalAlignment.Center, Foreground = new SolidColorBrush(Microsoft.UI.Colors.Black) });
            var host = new Microsoft.UI.Xaml.Controls.Canvas { Width = 1, Height = 1, Opacity = 0.01 };
            host.Children.Add(lines);
            root.Children.Add(host);
            lines.UpdateLayout();
            var bitmap = new Microsoft.UI.Xaml.Media.Imaging.RenderTargetBitmap();
            await bitmap.RenderAsync(lines, w, h);
            var pixels = await bitmap.GetPixelsAsync();
            root.Children.Remove(host);
            using var stream = new global::Windows.Storage.Streams.InMemoryRandomAccessStream();
            var encoder = await global::Windows.Graphics.Imaging.BitmapEncoder.CreateAsync(
                global::Windows.Graphics.Imaging.BitmapEncoder.PngEncoderId, stream);
            encoder.SetPixelData(global::Windows.Graphics.Imaging.BitmapPixelFormat.Bgra8, global::Windows.Graphics.Imaging.BitmapAlphaMode.Premultiplied,
                (uint)bitmap.PixelWidth, (uint)bitmap.PixelHeight, 96, 96, System.Runtime.InteropServices.WindowsRuntime.WindowsRuntimeBufferExtensions.ToArray(pixels));
            await encoder.FlushAsync();
            var bytes = new byte[stream.Size];
            using (var reader = new global::Windows.Storage.Streams.DataReader(stream.GetInputStreamAt(0)))
            {
                await reader.LoadAsync((uint)stream.Size);
                reader.ReadBytes(bytes);
            }
            await System.IO.File.WriteAllBytesAsync(path, bytes);
        }
        return new PageFile(path, new PageEncoding.Original("image/png"), w, h);
    }
#endif
}
