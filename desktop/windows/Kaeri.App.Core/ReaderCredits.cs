using Kaeri.Engine.Model;

namespace Kaeri.App;

// Credit pages and chapter parts in the open chapter: the pages the engine left out, which the
// person can show and keep, and the parts a chapter is read in, ticked on the scrubber.

/// <summary>The open chapter's parts: each part there is, how many in all, and those no group has.</summary>
public sealed record ReaderParts(ChapterPart[] Parts, uint Total, uint[] Missing);

/// <summary>A notice button the reader offers itself, rather than the engine.</summary>
public enum ReaderLocal { NotACredit, Undo }

public enum CreditTurnKind { None, Credit, Leaf, PastStart, Continue }

/// <summary>Where a turn goes from or onto a shown credit page: a credit page, a leaf, past the chapter's start, or on.</summary>
public readonly record struct CreditTurn(CreditTurnKind Kind, int At = 0)
{
    public static readonly CreditTurn None = new(CreditTurnKind.None);
}

public sealed partial class ReaderSession
{
    /// <summary>How long Undo holds after Not a credit page, doubled while a screen reader runs.</summary>
    public const uint KeptHoldMs = 8_000;

    /// <summary>The hold Undo has, which a test shortens.</summary>
    internal uint KeptHold { get; set; } = KeptHoldMs;

    /// <summary>The pages the layout left out as credit pages, the start's first, each side in order.</summary>
    public CreditPage[] Credits { get; private set; } = [];
    public ReaderParts? Parts { get; private set; }
    /// <summary>The credit page on screen in place of a page.</summary>
    public CreditPage? CreditOn { get; private set; }
    /// <summary>The credit pages at the start, or at the end, show from now on until the chapter closes.</summary>
    public bool ShownStart { get; private set; }
    public bool ShownEnd { get; private set; }
    /// <summary>The pill had its time on page 1: from now on it comes back only with the chrome.</summary>
    public bool CreditLineDone { get; set; }

    sealed record Keeping(ulong Session, ulong Fingerprint, bool AtEnd);

    Keeping? keeping;
    (ulong Fingerprint, uint Page)? relay;
    ulong? creditWanted;
    CancellationTokenSource? wantedTimer;
    /// <summary>The chapter another is opening over: a layout of it that comes late is not the new one's.</summary>
    string? leaving;
    /// <summary>The credit pages and parts of the chapter a strip flowed out of, until the next one opens.</summary>
    (CreditPage[] Credits, ReaderParts? Parts, bool ShownStart, bool ShownEnd)? left;

    /// <summary>A page kept with Not a credit page, while Undo is still offered.</summary>
    public bool Held(ulong fingerprint) => keeping?.Fingerprint == fingerprint;

    public bool KeepHeld => keeping is not null;

    /// <summary>"2 credit pages skipped", for the pages at the start or the end, or null when none are.</summary>
    public string? SkippedLine(bool atEnd)
    {
        var count = Credits.Count(c => c.AtEnd == atEnd);
        return count == 0 ? null : T.ReaderCreditPagesSkipped(count);
    }

    /// <summary>
    /// The pill or a line was clicked: the pages on that side show from now on, until the
    /// chapter closes. The first of them, which the view turns to.
    /// </summary>
    public CreditPage? ShowCredits(bool atEnd)
    {
        if (atEnd)
            ShownEnd = true;
        else
            ShownStart = true;
        CreditLineDone = true;
        Changed?.Invoke(ReaderChange.Credits);
        return Credits.FirstOrDefault(c => c.AtEnd == atEnd);
    }

    /// <summary>The pages could not be shown: they stay skipped and a notice says so.</summary>
    public void CreditFailed(bool atEnd)
    {
        if (atEnd)
            ShownEnd = false;
        else
            ShownStart = false;
        CreditOn = null;
        var text = T.ReaderCreditFailed;
        Put(new ReaderNoticeLine(++noticeCount, text, null), NoticeHold(4_000, Words(text), false, ScreenReader?.Invoke() ?? false));
        Changed?.Invoke(ReaderChange.Credits);
    }

    /// <summary>A credit page is on screen: its notice offers Not a credit page, unless it is the one kept.</summary>
    public void ShowCredit(CreditPage page)
    {
        CreditOn = page;
        if (!Held(page.Fingerprint))
            CreditNotice();
        else if (Notice?.Local == ReaderLocal.NotACredit)
            DropNotice();
        Changed?.Invoke(ReaderChange.Credits);
    }

    /// <summary>A turn or a jump away from a credit page: its notice goes.</summary>
    public void LeaveCredit()
    {
        if (CreditOn is null)
            return;
        CreditOn = null;
        if (Notice?.Local == ReaderLocal.NotACredit)
            DropNotice();
        Changed?.Invoke(ReaderChange.Credits);
    }

    /// <summary>The button of a notice the reader put up itself.</summary>
    public void ActLocal(ReaderLocal local)
    {
        switch (local)
        {
            case ReaderLocal.NotACredit when CreditOn is { } on:
                Keep(on.Fingerprint);
                break;
            case ReaderLocal.Undo:
                UndoKeep();
                break;
        }
    }

    void CreditNotice()
    {
        if (Notice?.Local == ReaderLocal.NotACredit)
            return;
        // No time of its own: it stays while the page shows.
        Put(new ReaderNoticeLine(++noticeCount, T.ReaderCreditShown, null, ReaderLocal.NotACredit), null);
    }

    /// <summary>
    /// Not a credit page: the page counts as one of the chapter's at once, and the notice offers
    /// Undo for 8 s (16 s while a screen reader runs). The engine hears of it when that notice
    /// goes, at a second keep, or as the chapter closes or another opens.
    /// </summary>
    public void Keep(ulong fingerprint)
    {
        CommitKeep();
        if (Credits.FirstOrDefault(c => c.Fingerprint == fingerprint) is not { } page)
            return;
        keeping = new Keeping(session, fingerprint, page.AtEnd);
        var hold = KeptHold * ((ScreenReader?.Invoke() ?? false) ? 2u : 1u);
        Put(new ReaderNoticeLine(++noticeCount, T.ReaderCreditKept, null, ReaderLocal.Undo), hold);
        Changed?.Invoke(ReaderChange.Credits);
    }

    /// <summary>The held keep goes to the engine; the next layout numbers the pages afresh.</summary>
    public void CommitKeep()
    {
        if (keeping is not { } keep)
            return;
        keeping = null;
        if (KeptNumber(Credits, keep.Fingerprint) is { } page)
            relay = (keep.Fingerprint, page);
        Send?.Invoke(new Command.Reader(new ReaderCommand.NotACreditPage(keep.Session, keep.Fingerprint)));
        if (Notice?.Local == ReaderLocal.Undo)
            DropNotice();
        Changed?.Invoke(ReaderChange.Credits);
    }

    /// <summary>Undo, or Ctrl+Z: the page is a credit page again and nothing is sent. False when no keep is held.</summary>
    public bool UndoKeep()
    {
        if (keeping is not { } keep)
            return false;
        keeping = null;
        if (Notice?.Local == ReaderLocal.Undo)
            DropNotice();
        if (CreditOn?.Fingerprint == keep.Fingerprint)
            CreditNotice();
        Changed?.Invoke(ReaderChange.Credits);
        return true;
    }

    /// <summary>A notice went: when it offered Undo, the keep it held goes to the engine.</summary>
    void NoticeGone(ReaderNoticeLine? old)
    {
        if (old?.Local == ReaderLocal.Undo && keeping is not null)
            CommitKeep();
    }

    void DropNotice()
    {
        var old = Notice;
        noticeTimer?.Cancel();
        noticeTimer = null;
        noticeLeft = null;
        Notice = null;
        Changed?.Invoke(ReaderChange.Notice);
        NoticeGone(old);
    }

    /// <summary>The scrubber's place and length with a held keep counted, as the chapter will have it.</summary>
    public (int At, int Total) HeldPages(int at, int total) =>
        keeping is { } keep ? HeldCounts(at, total, keep.AtEnd, CreditOn is { } on ? Held(on.Fingerprint) : null) : (at, total);

    void TakeCredits(CreditPage[] pages)
    {
        Credits = [.. pages.OrderBy(c => c.AtEnd).ThenBy(c => c.Page)];
        if (creditWanted is { } wanted)
        {
            creditWanted = null;
            wantedTimer?.Cancel();
            CreditOn = Credits.FirstOrDefault(c => c.Fingerprint == wanted);
            if (CreditOn is null && Notice?.Local == ReaderLocal.NotACredit)
                DropNotice();
        }
        else if (CreditOn is { } on)
            CreditOn = Credits.FirstOrDefault(c => c.Fingerprint == on.Fingerprint);
        Changed?.Invoke(ReaderChange.Credits);
    }

    /// <summary>No credit pages follow a layout that skipped none: after 200 ms the page shown is a page again.</summary>
    async void WaitForCredits()
    {
        wantedTimer?.Cancel();
        var timer = wantedTimer = new CancellationTokenSource();
        try
        {
            await Task.Delay(200, timer.Token);
        }
        catch (TaskCanceledException)
        {
            return;
        }
        if (wantedTimer != timer || creditWanted is null)
            return;
        creditWanted = null;
        CreditOn = null;
        if (Notice?.Local == ReaderLocal.NotACredit)
            DropNotice();
        Changed?.Invoke(ReaderChange.Credits);
    }

    /// <summary>
    /// The pages from the kept one on move one further: the files under the old numbers stand
    /// under the new ones, and the kept page's file under its own, so nothing on screen blinks.
    /// </summary>
    void Renumber(string chapter, uint kept, PageFile? file)
    {
        var moved = Pages.Where(p => p.Key.Chapter == chapter && p.Key.Page >= kept).ToList();
        foreach (var (key, _) in moved)
            Pages.Remove(key);
        foreach (var (key, value) in moved)
            Pages[key with { Page = key.Page + 1 }] = value;
        var failed = PageFailures.Where(p => p.Key.Chapter == chapter && p.Key.Page >= kept).ToList();
        foreach (var (key, _) in failed)
            PageFailures.Remove(key);
        foreach (var (key, value) in failed)
            PageFailures[key with { Page = key.Page + 1 }] = value;
        if (file is not null)
            Pages[new ReaderPageKey(chapter, kept)] = file;
    }

    /// <summary>
    /// The leaf that shows what showed before a kept page was laid out: the same page, one on
    /// when the kept page comes before it, and the end page for the end page.
    /// </summary>
    public static int RelaidLeaf(Leaf[] before, int was, Leaf[] after, uint kept)
    {
        if (after.Length == 0)
            return 0;
        if (was >= 0 && was < before.Length && before[was].EndPage && Array.FindIndex(after, l => l.EndPage) is var end and >= 0)
            return end;
        if (was < 0 || was >= before.Length || before[was].Parts is not { Length: > 0 } parts)
            return Math.Clamp(was, 0, after.Length - 1);
        var page = parts.Min(p => p.Page);
        var target = page + (kept <= page ? 1u : 0u);
        for (var i = 0; i < after.Length; i++)
            if (after[i].Parts.Any(p => p.Page == target))
                return i;
        return Math.Clamp(was, 0, after.Length - 1);
    }

    /// <summary>The chapter goes for another: a held keep is sent, and its credit pages and parts go with it.</summary>
    void LeaveChapter()
    {
        var kept = keeping?.Fingerprint;
        left = (Credits.Where(c => c.Fingerprint != kept).ToArray(), Parts, ShownStart, ShownEnd);
        ForgetCredits();
        leaving = View?.Chapter.Id;
    }

    /// <summary>
    /// The chapter a strip flowed out of stays, as when the next would not open: its layouts count
    /// again, and its credit pages and parts come back, less a page kept on the way out.
    /// </summary>
    void StayInChapter()
    {
        leaving = null;
        if (left is not { } was)
            return;
        left = null;
        Credits = was.Credits;
        Parts = was.Parts;
        ShownStart = was.ShownStart && Credits.Any(c => !c.AtEnd);
        ShownEnd = was.ShownEnd && Credits.Any(c => c.AtEnd);
        CreditLineDone = true;
        Changed?.Invoke(ReaderChange.Credits);
        if (Parts is not null)
            Changed?.Invoke(ReaderChange.Parts);
    }

    /// <summary>The chapter closed or another opened: its credit pages and parts go with it.</summary>
    void ForgetCredits()
    {
        CommitKeep();
        var had = Credits.Length > 0 || Parts is not null || CreditOn is not null || ShownStart || ShownEnd;
        ResetCredits();
        if (had)
            Changed?.Invoke(ReaderChange.Credits);
    }

    void ResetCredits()
    {
        keeping = null;
        relay = null;
        Credits = [];
        Parts = null;
        CreditOn = null;
        ShownStart = false;
        ShownEnd = false;
        CreditLineDone = false;
        creditWanted = null;
        wantedTimer?.Cancel();
        wantedTimer = null;
    }

    // The rules, for the views and the tests

    /// <summary>
    /// The number a kept page has once the engine lays the chapter out with it: its place in the
    /// chapter less the credit pages still left out before it.
    /// </summary>
    public static uint? KeptNumber(IReadOnlyList<CreditPage> credits, ulong fingerprint)
    {
        if (credits.FirstOrDefault(c => c.Fingerprint == fingerprint) is not { } kept)
            return null;
        var before = (uint)credits.Count(c => c.Fingerprint != fingerprint && c.Page < kept.Page);
        return kept.Page >= before ? kept.Page - before : 0;
    }

    /// <summary>
    /// Page <paramref name="at"/> of <paramref name="total"/> with a held keep counted: one more
    /// page, and one on for the pages after a kept page at the start. <paramref name="on"/> says
    /// whether a credit page is on screen and whether it is the held one, which reads as the first
    /// or the last page.
    /// </summary>
    public static (int At, int Total) HeldCounts(int at, int total, bool atEnd, bool? on)
    {
        if (total == 0)
            return (at, total);
        return on switch
        {
            true => (atEnd ? total + 1 : 1, total + 1),
            false => (at, total + 1),
            null => (at + (atEnd ? 0 : 1), total + 1),
        };
    }

    /// <summary>Where a missing part would start: at the next part there is, or at the end.</summary>
    public static double MissingAt(IReadOnlyList<ChapterPart> parts, uint missing, double total) =>
        parts.FirstOrDefault(p => p.Number > missing) is { } next ? next.FirstPage : total;

    /// <summary>The part past the first that starts at page <paramref name="at"/>, and the missing part whose place is there.</summary>
    static (uint? Part, uint? Missing) PartsAt(ReaderParts parts, int at, int total)
    {
        var first = parts.Parts.FirstOrDefault()?.Number;
        uint? part = parts.Parts.FirstOrDefault(p => p.Number != first && p.FirstPage == at)?.Number;
        uint? missing = null;
        foreach (var m in parts.Missing)
            if ((int)MissingAt(parts.Parts, m, total) == at)
            {
                missing = m;
                break;
            }
        return (part, missing);
    }

    /// <summary>
    /// The part preview's words at page <paramref name="at"/>, or null off the ticks: "Page 11 ·
    /// part 2", with a part no group has last, "Page 14 · part 4 · part 3 missing".
    /// </summary>
    public static string? TipWords(ReaderParts parts, int at, int total)
    {
        var (part, missing) = PartsAt(parts, at, total);
        return (part, missing) switch
        {
            ({ } p, { } m) => T.ReaderPartPreviewMissing(at, p, m),
            ({ } p, null) => T.ReaderPartPreview(at, p),
            (null, { } m) => T.ChapterStatePage(at) + " · " + T.ReadingRowPartMissing(1, m),
            _ => null,
        };
    }

    /// <summary>What the scrubber says there: "Part 4 starts at page 14, part 3 missing".</summary>
    public static string? PartWords(ReaderParts parts, int at, int total)
    {
        var (part, missing) = PartsAt(parts, at, total);
        return (part, missing) switch
        {
            ({ } p, { } m) => T.ReaderPartStarts(p, at) + ", " + T.ReadingRowPartMissing(1, m),
            ({ } p, null) => T.ReaderPartStarts(p, at),
            (null, { } m) => ReadingLines.Capitalized(T.ReadingRowPartMissing(1, m)),
            _ => null,
        };
    }

    /// <summary>The places on the track a tick marks: where each part after the first starts, and where each missing part would.</summary>
    public static IReadOnlyList<double> Ticks(ReaderParts parts, int total)
    {
        var first = parts.Parts.FirstOrDefault()?.Number;
        var ticks = parts.Parts.Where(p => p.Number != first).Select(p => (double)p.FirstPage).ToList();
        ticks.AddRange(parts.Missing.Select(m => MissingAt(parts.Parts, m, total)));
        return ticks;
    }

    /// <summary>
    /// A turn onto or from a shown credit page, or None when the turn is the leaves' own. From a
    /// start page: on to the next, then page 1; back to the one before, then nothing past the start.
    /// From an end page: on to the next, then the end page (or on); back to the one before, then
    /// the last page. Back from page 1 reaches the last start page, and on from the last page the
    /// first end page, once they show.
    /// </summary>
    public static CreditTurn CreditStep(IReadOnlyList<CreditPage> credits, CreditPage? on, bool shownStart, bool shownEnd,
        int leaf, bool onEnd, int lastPageLeaf, int? endLeaf, bool forward)
    {
        var starts = Enumerable.Range(0, credits.Count).Where(i => !credits[i].AtEnd).ToList();
        var ends = Enumerable.Range(0, credits.Count).Where(i => credits[i].AtEnd).ToList();
        if (on is not null)
        {
            var i = Enumerable.Range(0, credits.Count).FirstOrDefault(j => credits[j].Fingerprint == on.Fingerprint, -1);
            if (starts.IndexOf(i) is var s and >= 0)
                return forward
                    ? s + 1 < starts.Count ? new(CreditTurnKind.Credit, starts[s + 1]) : new(CreditTurnKind.Leaf, 0)
                    : s > 0 ? new(CreditTurnKind.Credit, starts[s - 1]) : new(CreditTurnKind.PastStart);
            if (ends.IndexOf(i) is var e and >= 0)
                return forward
                    ? e + 1 < ends.Count ? new(CreditTurnKind.Credit, ends[e + 1])
                        : endLeaf is { } end ? new(CreditTurnKind.Leaf, end) : new(CreditTurnKind.Continue)
                    : e > 0 ? new(CreditTurnKind.Credit, ends[e - 1]) : new(CreditTurnKind.Leaf, lastPageLeaf);
            // A page no longer in the list: the leaf under it.
            return new(CreditTurnKind.Leaf, leaf);
        }
        if (!forward && !onEnd && leaf == 0 && shownStart && starts.Count > 0)
            return new(CreditTurnKind.Credit, starts[^1]);
        if (forward && !onEnd && leaf == lastPageLeaf && shownEnd && ends.Count > 0)
            return new(CreditTurnKind.Credit, ends[0]);
        if (!forward && onEnd && shownEnd && ends.Count > 0)
            return new(CreditTurnKind.Credit, ends[^1]);
        return CreditTurn.None;
    }
}
