using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Xunit;
using Medium = Kaeri.Engine.Core.Medium;

namespace Kaeri.App.Tests;

/// <summary>Reading's models: the medium, sending, the reader's settings, and the engine's reading scenes.</summary>
public class ReadingTests
{
    /// <summary>KAERI_PROBE=1 runs the tests skipped for open bugs, to see them fail.</summary>
    public static bool Probing => Environment.GetEnvironmentVariable("KAERI_PROBE") == "1";

    static ExportTarget Kobo(bool chosen = false) =>
        new("kobo", "Kobo", 1264, 1680, Colour: false, Formats: [ExportFormat.Kepub, ExportFormat.Cbz],
            MangaFormats: [ExportFormat.Cbz, ExportFormat.Kepub], NovelFormats: [ExportFormat.Kepub], Split: false, Chosen: chosen);

    [Fact]
    public void AStripPlatesWordsSitInTheMiddleOfThePartOnScreen()
    {
        Assert.Equal((1600d, 600d), StripPlate.WordsMargins(3000, (1600, 2400), 50));
        // A sliver on screen still leaves the words their room, inside the image.
        Assert.Equal((0d, 2950d), StripPlate.WordsMargins(3000, (-500, 10), 50));
        Assert.Equal((2950d, 0d), StripPlate.WordsMargins(3000, (2990, 3400), 50));
        // Off screen, or not yet laid out: the middle.
        Assert.Equal((0d, 0d), StripPlate.WordsMargins(3000, null, 50));
        Assert.Equal((0d, 0d), StripPlate.WordsMargins(3000, (3200, 3600), 50));
    }

    [Fact]
    public void AReaderNoticeHoldsLongerForLongTextAndWaitsForAScreenReader()
    {
        Assert.Equal(6_000u, ReaderSession.NoticeHold(6_000, 4, true, false));
        Assert.Equal(8_000u, ReaderSession.NoticeHold(8_000, 12, true, false));
        Assert.Equal(9_000u, ReaderSession.NoticeHold(8_000, 13, true, false));
        Assert.Equal(10_000u, ReaderSession.NoticeHold(8_000, 32, true, false));
        Assert.Null(ReaderSession.NoticeHold(8_000, 4, true, true));
        Assert.Equal(4_000u, ReaderSession.NoticeHold(4_000, 4, false, true));
        Assert.Equal(0u, ReaderSession.NoticeHold(0, 30, false, false));
    }

    [Fact]
    public void ASendBindsToTheFirstJobTheEngineNames()
    {
        List<Command> sent = [];
        var sending = new SendState { Send = sent.Add };
        var id = sending.Begin("a:900301", ["c1", "c2"], Kobo(), novel: false);
        Assert.NotNull(id);
        Assert.Contains(sent, c => c is Command.Reading { V1: ReadingCommand.Prepare { Format: ExportFormat.Cbz } });
        Assert.Equal(ReadingPart.Send, sending.Apply(new ReadingEvent.ExportProgress(7, 1, 4, 1, 2)));
        var send = sending.Get(id!.Value)!;
        Assert.Equal(7ul, send.Job);
        Assert.Equal(0.25, send.Fraction);
        Assert.Equal((1, 2), (send.Done, send.Total));
        sending.Apply(new ReadingEvent.ExportReady(7, []));
        Assert.Equal(SendPhase.Ready, send.Phase);
        sending.Shared(id.Value);
        Assert.Contains(sent, c => c is Command.Reading { V1: ReadingCommand.Shared { Work: "a:900301" } });
        Assert.Empty(sending.Sends);
    }

    [Fact]
    public void AReadySendWhoseFilesWentFailsForSendAgain()
    {
        HomeGuard.Check();
        List<Command> sent = [];
        var sending = new SendState { Send = sent.Add };
        var id = sending.Begin("a:900301", ["c1", "c2"], Kobo(), novel: false)!.Value;
        var dir = AppDirs.NewScratchRoot("cs-send-files-gone");
        Directory.CreateDirectory(dir);
        try
        {
            var kept = Path.Combine(dir, "Chapter 1.cbz");
            var gone = Path.Combine(dir, "Chapter 2.cbz");
            File.WriteAllBytes(kept, [1]);
            File.WriteAllBytes(gone, [1]);
            sending.Apply(new ReadingEvent.ExportReady(7, [new ExportFile(kept, "Chapter 1.cbz", 1, null), new ExportFile(gone, "Chapter 2.cbz", 1, null)]));
            Assert.True(sending.FilesThere(id));
            // A day on, the engine cleared one: the send says so rather than passing it on.
            File.Delete(gone);
            Assert.False(sending.FilesThere(id));
            var send = sending.Get(id)!;
            Assert.Equal(SendPhase.Failed, send.Phase);
            Assert.True(send.FilesGone);
            Assert.Equal(T.SendFilesGone, send.Line);
            Assert.Empty(send.Files);
            Assert.DoesNotContain(sent, c => c is Command.Reading { V1: ReadingCommand.Shared });
        }
        finally
        {
            Directory.Delete(dir, true);
        }
    }

    [Fact]
    public void ASendStoppedBeforeItsJobIsNamedStopsWhenItIs()
    {
        List<Command> sent = [];
        var sending = new SendState { Send = sent.Add };
        var id = sending.Begin("a:900301", ["c1"], Kobo(), novel: true)!.Value;
        sending.Cancel(id);
        Assert.Single(sending.Sends);
        sending.Apply(new ReadingEvent.ExportProgress(9, 0, 0, 0, 0));
        Assert.Empty(sending.Sends);
        Assert.Contains(sent, c => c is Command.Reading { V1: ReadingCommand.CancelPrepare { Job: 9 } });
    }

    [Fact]
    public void ProgressFromAStoppedSendDoesNotTakeOverTheNextOne()
    {
        List<Command> sent = [];
        var sending = new SendState { Send = sent.Add };
        var first = sending.Begin("a:900301", ["c1"], Kobo(), novel: false)!.Value;
        sending.Apply(new ReadingEvent.ExportProgress(5, 1, 4, 0, 1));
        sending.Cancel(first);
        Assert.Contains(sent, c => c is Command.Reading { V1: ReadingCommand.CancelPrepare { Job: 5 } });
        var second = sending.Begin("a:900302", ["c2"], Kobo(), novel: false)!.Value;
        // The stopped job's last progress was already on its way.
        sending.Apply(new ReadingEvent.ExportProgress(5, 2, 4, 0, 1));
        sending.Apply(new ReadingEvent.ExportProgress(6, 1, 4, 0, 1));
        sending.Apply(new ReadingEvent.ExportReady(6, []));
        var send = Assert.Single(sending.Sends);
        Assert.Equal(second, send.Id);
        Assert.Equal(6ul, send.Job);
        Assert.Equal(SendPhase.Ready, send.Phase);
    }

    [Fact]
    public void ProgressFromAStoppedSendLeavesNoRow()
    {
        var sending = new SendState { Send = _ => { } };
        var id = sending.Begin("a:900301", ["c1"], Kobo(), novel: false)!.Value;
        sending.Apply(new ReadingEvent.ExportProgress(5, 1, 4, 0, 1));
        sending.Cancel(id);
        Assert.Empty(sending.Sends);
        sending.Apply(new ReadingEvent.ExportProgress(5, 2, 4, 0, 1));
        Assert.Empty(sending.Sends);
    }

    [Fact]
    public void ASendHeldAcrossARestartShowsAsItsOwnRow()
    {
        var sending = new SendState { Send = _ => { } };
        sending.Apply(new ReadingEvent.ExportWaiting(3, "a:900302", "kobo", new Msg.Reading(new ReadingMsg.ExportWaitsForWifi(2))));
        var send = Assert.Single(sending.Sends);
        Assert.Equal(SendPhase.Waiting, send.Phase);
        Assert.Equal("a:900302", send.Work);
        Assert.Contains("Wi-Fi", send.Line);
        Assert.Same(send, sending.Running("a:900302"));
    }

    [Fact]
    public void TheChosenTargetComesFromTheEngineAndATargetWithoutTheFormatIsRefused()
    {
        var sending = new SendState { Send = _ => { } };
        sending.Apply(new ReadingEvent.ExportTargets([new("other", "Other", 600, 800), Kobo(chosen: true)]));
        Assert.Equal("kobo", sending.DefaultTarget?.Id);
        Assert.Null(SendState.Format(new ExportTarget("x", "X", 1, 1, MangaFormats: [], NovelFormats: []), novel: false));
        Assert.Null(sending.Begin("a:900301", ["c1"], new ExportTarget("x", "X", 1, 1, MangaFormats: [], NovelFormats: []), novel: false));
    }

    [Fact]
    public void AFeedPasswordIsEightDigits()
    {
        for (var i = 0; i < 50; i++)
            Assert.Matches("^[0-9]{8}$", SendState.NewPassword());
    }

    [Fact]
    public void TheMediumHoldsItsQuestionUntilAnswered()
    {
        List<Command> sent = [];
        var medium = new MediumState { Send = sent.Add };
        Assert.Equal(KaeriIsFor.Watch, medium.Answer);
        medium.Apply(new MediumView(KaeriIsFor.Both, AppEdition.Full, false, true, new MediumAsk.Updated(), null));
        Assert.True(medium.Both && medium.Reads && medium.Watches && medium.Choosable);
        Assert.IsType<MediumAsk.Updated>(medium.Asking);
        medium.Answered(medium.Asking!, KaeriIsFor.Read);
        Assert.Null(medium.Asking);
        Assert.Contains(sent, c => c is Command.Medium { V1: MediumCommand.Answer { AnswerValue: KaeriIsFor.Read } });
        // Reading not in the build: Watch whatever the view says.
        medium.Apply(new MediumView(KaeriIsFor.Read, AppEdition.Full, false, false, null, null));
        Assert.Equal(KaeriIsFor.Watch, medium.Answer);
        Assert.False(medium.Choosable);
    }

    [Fact]
    public void ReaderChoicesFollowThePersonAndLayoutStaysOnTheDevice()
    {
        Assert.Equal(SettingScope.Medium, ReaderSession.HomeScope(ReaderSettingKey.Mode));
        Assert.Equal(SettingScope.Medium, ReaderSession.HomeScope(ReaderSettingKey.Direction));
        Assert.Equal(SettingScope.Device, ReaderSession.HomeScope(ReaderSettingKey.Fit));
        Assert.Equal(SettingScope.Device, ReaderSession.HomeScope(ReaderSettingKey.CropBorders));
    }

    /// <summary>A reading scene with the models wired as the window wires them.</summary>
    sealed class Scene : IDisposable
    {
        public readonly object Gate = new();
        public readonly EngineSession Session;
        /// <summary>The scene's own folders: its config, data and cache are under it.</summary>
        public readonly string Root;

        public Scene(FfiScene scene, string name)
        {
            HomeGuard.Check();
            var root = Root = AppDirs.NewScratchRoot(name);
            var engine = FfiEngine.StartScene(AppDirs.Under(root), new QuietHost(), scene, EngineSession.AppStart(signIn: false));
            Session = new EngineSession(engine, a => { lock (Gate) a(); });
            Session.Known.SendReadingWith(c => Session.Send(c));
            Session.Known.Reader.BookResource = engine.BookResource;
            Session.Start();
        }

        public Known Known => Session.Known;

        public void Do(Action act)
        {
            lock (Gate)
                act();
        }

        public async Task<T> Until<T>(Func<Known, T?> look, int seconds = 30) where T : class
        {
            var deadline = DateTime.UtcNow.AddSeconds(seconds);
            while (DateTime.UtcNow < deadline)
            {
                lock (Gate)
                    if (look(Known) is { } found)
                        return found;
                await Task.Delay(50, TestContext.Current.CancellationToken);
            }
            throw new TimeoutException("the engine did not say it in time");
        }

        public void Dispose() => Session.Dispose();
    }

    [Fact]
    public async Task ReadingHomeSceneShowsBothMediaAndAHome()
    {
        using var scene = new Scene(new FfiScene.ReadingHome(), "cs-reading-home");
        scene.Do(() => scene.Known.Medium.Load());
        var view = await scene.Until(k => k.Medium.View is { Answer: KaeriIsFor.Both } v ? v : null);
        Assert.True(view.ReadingAvailable);
        scene.Do(() => scene.Known.Reading.LoadHome());
        var home = await scene.Until(k => k.Reading.Home);
        Assert.NotNull(home);
        scene.Do(() => scene.Known.Reading.LoadChapters("a:900301"));
        var chapters = await scene.Until(k => k.Reading.Chapters.GetValueOrDefault("a:900301") is { Failure: null } c && c.All.Any() ? c : null);
        Assert.NotEmpty(chapters.All);
    }

    [Fact]
    public async Task TheReaderOpensARightToLeftMangaOnItsSurface()
    {
        using var scene = new Scene(new FfiScene.ReaderMangaRtl(), "cs-reader-rtl");
        var reader = scene.Known.Reader;
        scene.Do(() =>
        {
            reader.Open("a:900301");
            Assert.Equal(ReaderPhase.Opening, reader.Phase);
            // Nothing goes to the engine before the view has a size.
            reader.SurfaceChanged(new ReaderSurface(1280, 720, 1f, false, false, true));
        });
        var view = await scene.Until(k => k.Reader.View);
        Assert.Equal(Medium.Manga, view.Medium);
        Assert.Equal(ReaderMode.Paged, view.Mode);
        Assert.Equal(ReadDirection.RightToLeft, view.Direction);
        await scene.Until(k => k.Reader.Leaves.Length > 0 ? k.Reader.Leaves : null);
        await scene.Until(k => k.Reader.Pages.Count > 0 ? k.Reader.Pages : null);
        await scene.Until(k => k.Reader.Settings);
        scene.Do(() =>
        {
            Assert.Equal(ReaderPhase.Open, reader.Phase);
            Assert.True(reader.PageCount > 0);
            reader.Shown(Math.Min(1, reader.Leaves.Length - 1), TurnHow.Jump);
            reader.Close();
            Assert.Equal(ReaderPhase.Closed, reader.Phase);
            Assert.False(reader.IsOpen);
        });
    }

    [Fact]
    public async Task ASurfaceChangeWhileOpeningReachesTheEngine()
    {
        using var scene = new Scene(new FfiScene.ReaderMangaRtl(), "cs-reader-surface-opening");
        var reader = scene.Known.Reader;
        var narrow = new ReaderSurface(720, 1280, 1f, false, false, true);
        var wide = new ReaderSurface(2560, 1440, 1f, false, true, true);
        scene.Do(() =>
        {
            reader.Open("a:900301");
            reader.SurfaceChanged(narrow);
        });
        await scene.Until(k => k.Reader.Phase == ReaderPhase.Open && k.Reader.Leaves.Length > 0 ? k.Reader.Leaves : null);
        int single = 0;
        scene.Do(() => single = reader.Leaves.Length);
        // An open chapter lays out again for two pages: the counts differ.
        scene.Do(() => reader.SurfaceChanged(wide));
        var spreads = (await scene.Until(k => k.Reader.Leaves.Length > 0 && k.Reader.Leaves.Length != single ? k.Reader.Leaves : null)).Length;
        scene.Do(reader.Close);
        await scene.Until(k => k.Reader.SessionId == 0 ? k : null);
        scene.Do(() =>
        {
            reader.SurfaceChanged(narrow);
            // The window goes full screen before the engine says the chapter is open.
            reader.Open("a:900301");
            reader.SurfaceChanged(wide);
        });
        await scene.Until(k => k.Reader.Phase == ReaderPhase.Open && k.Reader.Leaves.Length > 0 ? k.Reader.Leaves : null);
        try
        {
            await scene.Until(k => k.Reader.Leaves.Length == spreads ? k.Reader.Leaves : null, seconds: 10);
        }
        catch (TimeoutException)
        {
            int now = 0;
            scene.Do(() => now = reader.Leaves.Length);
            Assert.Fail($"laid out for {now} leaves, the old surface's count was {single} and the surface on screen needs {spreads}");
        }
    }

    [Fact]
    public async Task TheReaderOpensANovelWithItsStyle()
    {
        using var scene = new Scene(new FfiScene.ReaderNovel(), "cs-reader-novel");
        var reader = scene.Known.Reader;
        scene.Do(() =>
        {
            reader.Open("a:900302");
            reader.SurfaceChanged(new ReaderSurface(1280, 720, 1f, false, false, true));
        });
        var novel = await scene.Until(k => k.Reader.Novel);
        Assert.Equal(Medium.Novel, reader.Medium);
        Assert.NotEmpty(reader.NovelStyle);
        Assert.NotNull(novel);
        var assets = KaeriFfiMethods.NovelAssets();
        Assert.NotEmpty(assets.Script);
        // The page the web view loads first is served.
        var first = novel.Restore is { } at && at.Spine < novel.Spine.Length ? novel.Spine[(int)at.Spine].Url : novel.Entry;
        var page = reader.BookResource!(first.Split('#')[0]);
        Assert.NotNull(page);
        Assert.Equal(200, page.Status);
        // One Content-Type, the engine's with its charset.
        var lines = ReaderSession.ResponseHeaders(page).Split("\r\n", StringSplitOptions.RemoveEmptyEntries);
        var types = lines.Where(l => l.StartsWith("Content-Type:", StringComparison.OrdinalIgnoreCase)).ToList();
        Assert.Single(types);
        Assert.Contains("charset=utf-8", types[0]);
        Assert.Contains(lines, l => l.StartsWith("Content-Security-Policy:"));
    }

    [Fact]
    public async Task ANovelKeepsThePlaceItsViewLastToldForAViewMadeAgain()
    {
        using var scene = new Scene(new FfiScene.ReaderNovel(), "cs-reader-novel-shown");
        var reader = scene.Known.Reader;
        scene.Do(() =>
        {
            reader.Open("a:900302");
            reader.SurfaceChanged(new ReaderSurface(1280, 720, 1f, false, false, true));
        });
        await scene.Until(k => k.Reader.Novel);
        NovelPos? before = null;
        scene.Do(() => before = reader.NovelShown);
        Assert.Null(before);
        // The web view read on; its process then went, and the new view opens here, not where the chapter opened.
        var place = new NovelPos(0, 40, [2, 3]);
        NovelPos? kept = null;
        scene.Do(() =>
        {
            reader.NovelAt(place, 3, 12, TurnHow.Swipe);
            kept = reader.NovelShown;
        });
        Assert.Same(place, kept);
    }

    [Fact]
    public async Task ANovelsEndPageCountsTheChapterRead()
    {
        using var scene = new Scene(new FfiScene.ReaderNovel(), "cs-reader-novel-end");
        var reader = scene.Known.Reader;
        scene.Do(() =>
        {
            reader.Open("a:900302");
            reader.SurfaceChanged(new ReaderSurface(1280, 720, 1f, false, false, true));
        });
        var novel = await scene.Until(k => k.Reader.Novel);
        var last = (uint)Math.Max(novel.Spine.Length - 1, 0);
        scene.Do(() =>
        {
            // Into the last document, then past its end: the end page shows.
            reader.NovelAt(new NovelPos(last, 0, []), 1, 3, TurnHow.Turn);
            reader.NovelEnded();
        });
        await scene.Until(k => k.Reader.Read ? k : null, seconds: 10);
    }

    /// <summary>The strip scene's reader, open at its place in chapter 2, and its images as the strip lays them out.</summary>
    static async Task<(ReaderSession Reader, List<LeafPart> Parts)> OpenStrip(Scene scene)
    {
        var reader = scene.Known.Reader;
        scene.Do(() =>
        {
            reader.Open("a:900303");
            reader.SurfaceChanged(new ReaderSurface(1280, 720, 1f, false, false, true));
        });
        await scene.Until(k => k.Reader.View is { Mode: ReaderMode.Strip } v && k.Reader.ReadingLeaves > 0 && k.Reader.EndNext is not null ? v : null);
        List<LeafPart> parts = [];
        scene.Do(() => parts = [.. reader.Leaves.Where(l => !l.EndPage).SelectMany(l => l.Parts)]);
        return (reader, parts);
    }

    [Fact]
    public async Task AStripOpensAtThePageKeptInPagedModeWhereItSaysItResumed()
    {
        using var scene = new Scene(new FfiScene.ReaderMangaRtl(), "cs-reader-strip-from-paged");
        var reader = scene.Known.Reader;
        var surface = new ReaderSurface(1280, 720, 1f, false, false, true);
        scene.Do(() =>
        {
            reader.Open("a:900301");
            reader.SurfaceChanged(surface);
        });
        var paged = await scene.Until(k => k.Reader.View is { Mode: ReaderMode.Paged } v && k.Reader.Leaves.Length > 0 ? v : null);
        var page = paged.Start.Page ?? 0;
        Assert.True(page > 0);
        // The series goes to long strip, and the chapter opens again from its place kept in pages.
        scene.Do(() => reader.ChooseMode(ReaderSession.ModeChoice.Strip));
        scene.Do(reader.Close);
        await scene.Until(k => k.Reader.SessionId == 0 ? k : null);
        scene.Do(() =>
        {
            reader.SurfaceChanged(surface);
            reader.Open("a:900301");
        });
        await scene.Until(k => k.Reader.View is { Mode: ReaderMode.Strip } v ? v : null);
        var notice = await scene.Until(k => k.Reader.Notice);
        scene.Do(() =>
        {
            // The strip scrolls to the top of the page's image, the one the notice names.
            Assert.Equal(page, reader.StripStart?.Image);
            Assert.Equal(0f, reader.StripStart?.Offset);
            Assert.Equal(T.ReaderResumedImage(page + 1), notice.Text);
        });
    }

    [Fact]
    public async Task AStripsFootUnderImagesStillComingIsNoEnd()
    {
        using var scene = new Scene(new FfiScene.ReaderStrip(), "cs-reader-strip-unloaded");
        var (reader, parts) = await OpenStrip(scene);
        var last = (uint)(parts.Count - 1);
        scene.Do(() =>
        {
            // The images as a slow source leaves them: not in yet.
            reader.Pages.Clear();
            Assert.False(reader.StripLanded(last));
            reader.StripAt(new StripPos(last, 1f), footVisible: true);
            reader.FlowOn(last);
            Assert.False(reader.Flowing);
        });
        // Longer than the engine waits on a last page before it counts the chapter read.
        await Task.Delay(3500, TestContext.Current.CancellationToken);
        scene.Do(() =>
        {
            Assert.False(reader.Read);
            reader.Retry(parts[^1].Page);
        });
        // Once the last image is in, the foot under it is the chapter's end.
        await scene.Until(k => k.Reader.StripLanded(last) ? k : null);
        scene.Do(() => reader.StripAt(new StripPos(last, 1f), footVisible: true));
        await scene.Until(k => k.Reader.Read ? k : null, seconds: 15);
    }

    [Fact]
    public async Task AStripsFailedFlowKeepsTheChapterReadAndEndsAtTheJoin()
    {
        using var scene = new Scene(new FfiScene.ReaderStrip(), "cs-reader-strip-flow-failed");
        var (reader, parts) = await OpenStrip(scene);
        var last = (uint)(parts.Count - 1);
        string read = "";
        ChapterRow next = null!;
        scene.Do(() =>
        {
            read = reader.View!.Chapter.Id;
            next = reader.EndNext!;
        });
        // The next chapter can't be had, as with its source out of reach.
        File.Delete(Path.Combine(scene.Root, "data", "fixture-books", "fixture-paper-kites", "Chapter 3.cbz"));
        scene.Do(() => reader.StripAt(new StripPos(last, 0f), footVisible: false));
        await scene.Until(k => k.Reader.StripLanded(last) ? k : null);
        scene.Do(() =>
        {
            reader.FlowOn(last);
            Assert.True(reader.Flowing);
        });
        var failed = await scene.Until(k => k.Reader.FlowFailed);
        scene.Do(() =>
        {
            Assert.Equal(next.Id, failed.Chapter.Id);
            // The chapter read stays, with its images, and the strip ends at the join for the next one.
            Assert.False(reader.Flowing);
            Assert.Equal(ReaderPhase.Open, reader.Phase);
            Assert.Equal(read, reader.View!.Chapter.Id);
            Assert.True(reader.ReadingLeaves > 0);
            Assert.Equal(next.Id, reader.EndNext?.Id);
            // Scrolling past the join again asks nothing; Try again does, for that chapter.
            reader.FlowOn(last);
            Assert.False(reader.Flowing);
            reader.RetryFlow();
            Assert.True(reader.Flowing);
            Assert.Null(reader.FlowFailed);
        });
        failed = await scene.Until(k => k.Reader.FlowFailed);
        scene.Do(() =>
        {
            Assert.Equal(next.Id, failed.Chapter.Id);
            Assert.Equal(ReaderPhase.Open, reader.Phase);
            Assert.Equal(read, reader.View!.Chapter.Id);
        });
    }

    [Fact]
    public async Task ANovelsAddressesHaveAHostTheWebViewKeepsAsItIs()
    {
        using var scene = new Scene(new FfiScene.ReaderNovel(), "cs-reader-novel-host");
        var reader = scene.Known.Reader;
        scene.Do(() =>
        {
            reader.Open("a:900302");
            reader.SurfaceChanged(new ReaderSurface(1280, 720, 1f, false, false, true));
        });
        var novel = await scene.Until(k => k.Reader.Novel);
        // A host of digits alone reaches the engine as an IPv4 address from a standard scheme.
        var host = new Uri(novel.Entry).Host;
        Assert.False(host.All(char.IsAsciiDigit), novel.Entry);
        Assert.NotNull(reader.BookResource!(novel.Entry.Split('#')[0]));
        Assert.Null(reader.BookResource!(novel.Entry.Split('#')[0].Replace($"//{host}/", "//0.0.0.7/")));
    }

    [Fact]
    public async Task AReaderLeftWhileItOpensStaysClosedAndTheNextOneOpens()
    {
        using var scene = new Scene(new FfiScene.ReaderMangaRtl(), "cs-reader-left");
        var reader = scene.Known.Reader;
        scene.Do(() =>
        {
            reader.Open("a:900301");
            reader.SurfaceChanged(new ReaderSurface(1280, 720, 1f, false, false, true));
            reader.Close();
            Assert.Equal(ReaderPhase.Closed, reader.Phase);
        });
        await Task.Delay(1500, TestContext.Current.CancellationToken);
        scene.Do(() =>
        {
            Assert.Equal(ReaderPhase.Closed, reader.Phase);
            Assert.Null(reader.View);
            Assert.Empty(reader.Leaves);
            reader.Open("a:900301");
            reader.SurfaceChanged(new ReaderSurface(1280, 720, 1f, false, false, true));
        });
        await scene.Until(k => k.Reader.Leaves.Length > 0 ? k.Reader.Leaves : null);
        scene.Do(() => Assert.Equal(ReaderPhase.Open, reader.Phase));
    }

    [Fact]
    public async Task ANewLayoutKeepsThePageOnScreen()
    {
        using var scene = new Scene(new FfiScene.ReaderMangaRtl(), "cs-reader-relayout");
        var reader = scene.Known.Reader;
        scene.Do(() =>
        {
            reader.Open("a:900301");
            reader.SurfaceChanged(new ReaderSurface(1280, 720, 1f, false, false, true));
        });
        var first = await scene.Until(k => k.Reader.Leaves.Length > 3 ? k.Reader.Leaves : null);
        uint page = 0;
        scene.Do(() =>
        {
            var at = first.Length / 2;
            reader.Shown(at, TurnHow.Jump);
            page = first[at].Parts.Min(p => p.Page);
            // Full screen: the same book in a taller window lays out again.
            reader.SurfaceChanged(new ReaderSurface(720, 1280, 1f, false, false, true));
        });
        var after = await scene.Until(k => !ReferenceEquals(k.Reader.Leaves, first) && k.Reader.Leaves.Length > 0 ? k.Reader.Leaves : null);
        scene.Do(() => Assert.Contains(after[reader.Current].Parts, p => p.Page == page));
    }

    [Fact]
    public async Task ARoundTripThroughSpreadsKeepsThePageShownAlone()
    {
        using var scene = new Scene(new FfiScene.ReaderMangaRtl(), "cs-reader-spreads");
        var reader = scene.Known.Reader;
        var single = new ReaderSurface(1280, 720, 1f, false, false, true);
        var spreads = new ReaderSurface(1280, 720, 1f, false, true, true);
        scene.Do(() =>
        {
            reader.Open("a:900301");
            reader.SurfaceChanged(spreads);
        });
        var wide = await scene.Until(k => Paired(k.Reader.Leaves) ? k.Reader.Leaves : null);
        uint page = 0;
        scene.Do(() =>
        {
            // The second page of a spread, shown alone.
            page = wide.First(l => Pages(l) == 2).Parts.Max(p => p.Page);
            reader.SurfaceChanged(single);
        });
        var alone = await scene.Until(k => Alone(k.Reader.Leaves) ? k.Reader.Leaves : null);
        scene.Do(() =>
        {
            reader.Shown(Array.FindIndex(alone, l => l.Parts.Any(p => p.Page == page)), TurnHow.Jump);
            reader.SurfaceChanged(spreads);
        });
        wide = await scene.Until(k => Paired(k.Reader.Leaves) ? k.Reader.Leaves : null);
        scene.Do(() =>
        {
            Assert.Contains(wide[reader.Current].Parts, p => p.Page == page);
            reader.SurfaceChanged(single);
        });
        // Alone again: that page, not the spread's first.
        alone = await scene.Until(k => Alone(k.Reader.Leaves) ? k.Reader.Leaves : null);
        scene.Do(() => Assert.Equal(new[] { page }, alone[reader.Current].Parts.Select(p => p.Page)));

        static int Pages(Leaf leaf) => leaf.Parts.Select(p => p.Page).Distinct().Count();
        static bool Paired(Leaf[] leaves) => leaves.Any(l => Pages(l) == 2);
        static bool Alone(Leaf[] leaves) => leaves.Length > 0 && leaves.All(l => Pages(l) <= 1);
    }

    [Fact]
    public async Task DownloadsNameEachSeriesByItsTitle()
    {
        using var scene = new Scene(new FfiScene.ReadingDownloads(), "cs-reading-downloads");
        scene.Do(() => scene.Known.Reading.LoadDownloads());
        var downloads = await scene.Until(k => k.Reading.Downloads is { Series.Length: > 0 } d ? d : null);
        Assert.All(downloads.Series, s => Assert.NotEqual(s.Work.ToString(), s.Title));
        Assert.Contains(downloads.Series, s => s.Title == "The Lantern Ferry");
    }
}
