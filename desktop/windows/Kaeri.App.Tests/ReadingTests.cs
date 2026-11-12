using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Xunit;
using Medium = Kaeri.Engine.Core.Medium;

namespace Kaeri.App.Tests;

/// <summary>Reading's models: the medium, sending, the reader's settings, and the engine's reading scenes.</summary>
public class ReadingTests
{
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

        public Scene(FfiScene scene, string name)
        {
            HomeGuard.Check();
            var root = AppDirs.NewScratchRoot(name);
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
    public async Task DownloadsNameEachSeriesByItsTitle()
    {
        using var scene = new Scene(new FfiScene.ReadingDownloads(), "cs-reading-downloads");
        scene.Do(() => scene.Known.Reading.LoadDownloads());
        var downloads = await scene.Until(k => k.Reading.Downloads is { Series.Length: > 0 } d ? d : null);
        Assert.All(downloads.Series, s => Assert.NotEqual(s.Work.ToString(), s.Title));
        Assert.Contains(downloads.Series, s => s.Title == "The Lantern Ferry");
    }
}
