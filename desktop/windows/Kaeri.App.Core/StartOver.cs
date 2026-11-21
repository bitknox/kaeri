using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>Where a start over stands, as the restore dialog shows it.</summary>
public enum StartOverStep
{
    /// <summary>Sent, and nothing heard yet: the dialog keeps what it shows for the first 600 ms.</summary>
    Sent,
    /// <summary>The folder's old sync data is being deleted (<c>rkey-clearing</c>).</summary>
    Clearing,
    /// <summary>The new group is being set up (<c>cloud-setting-up</c>).</summary>
    SettingUp,
    /// <summary>It stopped, and nothing on this PC changed; Try again sends it again.</summary>
    Failed,
    /// <summary>The new recovery key came.</summary>
    Key,
    /// <summary>It ended with no key to show: the folder held this PC's own group, which was taken.</summary>
    Ended,
}

/// <summary>
/// Start over without the key: the engine's steps for one <c>ClearFolderAndStart</c>, as the
/// restore dialog follows them. It claims the events of the start over it sent, so the picker
/// that found the other group, still closing, never takes the steps or the key; and it ignores
/// the end of that picker's own sign-in, which can arrive after the start over began.
/// </summary>
public sealed class StartOver(SyncFolderKind service)
{
    public SyncFolderKind Service { get; } = service;

    public StartOverStep Step { get; private set; } = StartOverStep.Sent;

    /// <summary>Why it stopped, when <see cref="Step"/> is <see cref="StartOverStep.Failed"/>.</summary>
    public SyncProblem? Failure { get; private set; }

    /// <summary>The new recovery key, when <see cref="Step"/> is <see cref="StartOverStep.Key"/>.</summary>
    public string? Key { get; private set; }

    /// <summary>The engine is still at it: the dialog may close, and the work carries on.</summary>
    public bool Working => Step is StartOverStep.Sent or StartOverStep.Clearing or StartOverStep.SettingUp;

    /// <summary>A step of this start over was heard, so its sign-in's end is its own.</summary>
    bool heard;

    /// <summary>
    /// Takes the engine's event, and says whether it was this start over's: the caller then gives
    /// it to nothing else. Only while <see cref="Working"/>.
    /// </summary>
    public bool Take(Event e)
    {
        if (!Working)
            return false;
        switch (e)
        {
            case Event.SyncSignIn { Step: SyncSignInStep.Clearing }:
                heard = true;
                Step = StartOverStep.Clearing;
                return true;
            case Event.SyncSignIn { Step: SyncSignInStep.Finishing }:
                heard = true;
                Step = StartOverStep.SettingUp;
                return true;
            case Event.SyncSignIn { Step: null }:
                // Before a step of its own this is the end of the sign-in that found the other
                // group, which says nothing about the start over.
                if (heard)
                    Step = StartOverStep.Ended;
                return true;
            case Event.SyncSignIn:
                // A browser or a code: a sign-in the folder needed first. The dialog stays as it is.
                return true;
            case Event.SyncRecoveryKey recovery:
                Key = recovery.V1;
                Step = StartOverStep.Key;
                return true;
            case Event.SyncProblem problem:
                Failure = problem.V1;
                Step = StartOverStep.Failed;
                return true;
            default:
                return false;
        }
    }
}
