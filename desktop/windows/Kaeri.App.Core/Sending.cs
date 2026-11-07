using Medium = Kaeri.Engine.Core.Medium;
using System.Security.Cryptography;
using Kaeri.Engine.Model;

namespace Kaeri.App;

// Send to e-reader and the OPDS feed as the engine tells them. A send is asked for from a dialog,
// told as progress, may wait for Wi-Fi (the engine says why and keeps it across a restart), and
// ends with files to save or share. The feed is this PC's catalogue for e-readers at home.
// Nothing here decides anything: the formats a target takes, the words of a failure and the line
// of a held send all come from the engine. The twin of the Apple app's SendModel.

public enum SendPhase { Preparing, Waiting, Ready, Failed }

/// <summary>One send, from the moment the dialog asks for it until the files are shared or it is dropped.</summary>
public sealed record EReaderSend
{
    /// <summary>Ours, from 1, so a send is the same row before the engine has named its job.</summary>
    public required int Id { get; init; }
    public ulong? Job { get; set; }
    /// <summary>Empty for a send the engine started again after a restart, until it says which.</summary>
    public string Work { get; set; } = "";
    public string Target { get; set; } = "";
    /// <summary>The chapters asked for, in reading order; empty for a restarted send.</summary>
    public string[] Chapters { get; init; } = [];
    public SendPhase Phase { get; set; } = SendPhase.Preparing;
    /// <summary>How far, when the engine has said.</summary>
    public double? Fraction { get; set; }
    public int Done { get; set; }
    public int Total { get; set; }
    /// <summary>Why it waits, or why it failed, in words.</summary>
    public string Line { get; set; } = "";
    public ExportFile[] Files { get; set; } = [];
    internal bool CancelOnBind { get; set; }

    public bool IsRunning => Phase is SendPhase.Preparing or SendPhase.Waiting;
}

public sealed class SendState
{
    internal Action<Command>? Send { get; set; }

    /// <summary>The e-readers the engine knows, once it has said.</summary>
    public ExportTarget[] Targets { get; private set; } = [];
    public List<EReaderSend> Sends { get; } = [];
    /// <summary>The feed, as last told.</summary>
    public FeedView? Feed { get; private set; }
    /// <summary>The e-reader the dialog offers first: the engine keeps it, on this device only.</summary>
    public string? ChosenTarget { get; private set; }

    int lastId;
    bool feedStaged;

    void Ask(ReadingCommand command) => Send?.Invoke(new Command.Reading(command));

    public void LoadTargets() => Ask(new ReadingCommand.LoadExportTargets());

    /// <summary>The target the dialog starts on: the one chosen here, else the engine's first.</summary>
    public ExportTarget? DefaultTarget => Targets.FirstOrDefault(t => t.Id == ChosenTarget) ?? Targets.FirstOrDefault();

    public void Choose(string target)
    {
        if (ChosenTarget == target)
            return;
        ChosenTarget = target;
        Ask(new ReadingCommand.SetSendTarget(target));
    }

    /// <summary>The format a target is sent in: the first it takes, as the engine lists them.</summary>
    public static ExportFormat? Format(ExportTarget target, bool novel) =>
        (novel ? target.NovelFormats : target.MangaFormats) is { Length: > 0 } formats ? formats[0] : null;

    /// <summary>Asks for files of <paramref name="chapters"/> for <paramref name="target"/>. The send is the one returned.</summary>
    public int? Begin(string work, IReadOnlyList<string> chapters, ExportTarget target, bool novel)
    {
        if (Format(target, novel) is not { } format)
            return null;
        lastId++;
        Sends.Add(new EReaderSend { Id = lastId, Work = work, Target = target.Id, Chapters = [.. chapters] });
        Choose(target.Id);
        Ask(new ReadingCommand.Prepare(work, new DownloadPick.Chapters([.. chapters]), target.Id, format));
        return lastId;
    }

    /// <summary>Stops a send that is under way, or drops one that has ended.</summary>
    public void Cancel(int id)
    {
        var send = Sends.FirstOrDefault(s => s.Id == id);
        if (send is null)
            return;
        if (send.IsRunning)
        {
            if (send.Job is { } job)
                Ask(new ReadingCommand.CancelPrepare(job));
            else
            {
                send.CancelOnBind = true;
                return;
            }
        }
        Sends.Remove(send);
    }

    /// <summary>The files were saved or shared: the engine keeps that the chapters were sent, and the series page asks where you stopped.</summary>
    public void Shared(int id)
    {
        var send = Sends.FirstOrDefault(s => s.Id == id);
        if (send is null)
            return;
        if (send.Work.Length > 0)
            Ask(new ReadingCommand.Shared(send.Work, send.Chapters, send.Target));
        Sends.Remove(send);
    }

    public void Dismiss(int id) => Sends.RemoveAll(s => s.Id == id);

    public EReaderSend? Get(int id) => Sends.FirstOrDefault(s => s.Id == id);

    /// <summary>The send a series has under way, for its page.</summary>
    public EReaderSend? Running(string work) => Sends.FirstOrDefault(s => s.Work == work && s.IsRunning);

    public void StoppedAt(string work, string? chapter) => Ask(new ReadingCommand.StoppedAt(work, chapter, null));

    // The feed

    public void LoadFeed() => Ask(new ReadingCommand.LoadFeed());

    public void SetFeed(bool on, string? password = null) => Ask(new ReadingCommand.SetFeed(on, password));

    public void SetShare(FeedShelf shelf, bool on) => Ask(new ReadingCommand.SetFeedShare(shelf, on));

    /// <summary>A password for e-readers: eight digits, which a person can type on a device with no keyboard.</summary>
    public static string NewPassword() => string.Concat(Enumerable.Range(0, 8).Select(_ => RandomNumberGenerator.GetInt32(10)));

    /// <summary>A shot's feed, told by hand: the engine's own view no longer replaces it.</summary>
    public void StageFeed(FeedView view)
    {
        feedStaged = true;
        Feed = view;
    }

    /// <summary>A shot's send in a state, never asked of the engine.</summary>
    public int Stage(string state, string work, IReadOnlyList<string> chapters, string target)
    {
        lastId++;
        var send = new EReaderSend { Id = lastId, Work = work, Target = target, Chapters = [.. chapters] };
        switch (state)
        {
            case "waiting":
                send.Phase = SendPhase.Waiting;
                send.Line = T.ExportWaitsForWifi(Math.Max(1, chapters.Count - 1));
                break;
            case "failed":
                send.Phase = SendPhase.Failed;
                send.Line = T.SendFailedChapter(chapters.LastOrDefault() ?? "1", new ReadingFailure.Offline().Words());
                break;
            case "ready":
                send.Phase = SendPhase.Ready;
                break;
            default:
                send.Fraction = 0.62;
                send.Done = 2;
                send.Total = Math.Min(3, Math.Max(chapters.Count, 1));
                break;
        }
        Sends.Add(send);
        return lastId;
    }

    // Hearing

    /// <summary>Takes a reading event; says whether it was about sending.</summary>
    public ReadingPart? Apply(ReadingEvent e)
    {
        switch (e)
        {
            case ReadingEvent.ExportTargets targets:
                Targets = targets.V1;
                if (Targets.FirstOrDefault(t => t.Chosen) is { } kept)
                    ChosenTarget = kept.Id;
                return ReadingPart.Send;
            case ReadingEvent.ExportProgress progress:
                Update(progress.Job, null, null, s =>
                {
                    s.Phase = SendPhase.Preparing;
                    s.Fraction = progress.Total > 0 ? Math.Clamp((double)progress.Done / progress.Total, 0, 1) : null;
                    s.Done = (int)progress.ChaptersDone;
                    s.Total = (int)progress.ChaptersValue;
                });
                return ReadingPart.Send;
            case ReadingEvent.ExportFailed failed:
                var cause = failed.Failure.Words();
                var words = failed.Chapter is { } chapter ? T.SendFailedChapter(chapter, cause) : T.SendFailed(cause);
                Update(failed.Job, null, null, s => { s.Phase = SendPhase.Failed; s.Line = words; });
                return ReadingPart.Send;
            case ReadingEvent.ExportWaiting waiting:
                Update(waiting.Job, waiting.Work, waiting.Target, s => { s.Phase = SendPhase.Waiting; s.Line = waiting.Line.Text(); });
                return ReadingPart.Send;
            case ReadingEvent.ExportReady ready:
                Update(ready.Job, null, null, s => { s.Phase = SendPhase.Ready; s.Files = ready.Files; });
                return ReadingPart.Send;
            case ReadingEvent.Feed feed:
                if (!feedStaged)
                    Feed = feed.V1;
                return ReadingPart.Feed;
            case ReadingEvent.Failed { About: ReadingAbout.Export } failed:
                // A send the engine could not even start names no job: it is the one that was
                // waiting for an answer, else the latest still going.
                var send = Sends.FirstOrDefault(s => s.Job is null && s.IsRunning) ?? Sends.LastOrDefault(s => s.IsRunning);
                if (send is not null)
                {
                    send.Phase = SendPhase.Failed;
                    send.Line = failed.Failure.Words();
                }
                return ReadingPart.Send;
            default:
                return null;
        }
    }

    /// <summary>
    /// The send a job belongs to: the one already bound to it, else the first one asked for that
    /// the engine had not named yet, else one the app never started (a send held across a restart).
    /// </summary>
    void Update(ulong job, string? work, string? target, Action<EReaderSend> change)
    {
        var send = Sends.FirstOrDefault(s => s.Job == job);
        if (send is null && Sends.FirstOrDefault(s => s.Job is null && s.IsRunning) is { } unbound)
        {
            unbound.Job = job;
            send = unbound;
        }
        if (send is null)
        {
            lastId++;
            send = new EReaderSend { Id = lastId, Job = job, Work = work ?? "", Target = target ?? "" };
            Sends.Add(send);
        }
        if (work is not null && send.Work.Length == 0)
            send.Work = work;
        if (target is not null && send.Target.Length == 0)
            send.Target = target;
        if (send.CancelOnBind && send.Job is { } bound)
        {
            Ask(new ReadingCommand.CancelPrepare(bound));
            Sends.Remove(send);
            return;
        }
        change(send);
    }
}
