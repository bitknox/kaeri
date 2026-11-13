using CommunityToolkit.WinUI.Controls;
using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Pages;

/// <summary>
/// One tracker's card in Settings › Accounts, as the Mac's account rows have it: who is signed
/// in and how sync stands ("Synced 2 min ago", "Not synced: …", changes waiting), Expired with
/// Sign in again when the tracker refused the token, and a ring with "Waiting for the browser…"
/// while a sign-in is under way. The card keeps itself current from the engine's events, and
/// the sync line from a 30 second clock, so it is right whether or not the pane is drawn again.
/// </summary>
sealed class AccountCard
{
    readonly Ctx ctx;
    readonly string tracker;
    readonly string name;
    readonly SettingsCard card;
    readonly ProgressRing ring = new() { Width = 16, Height = 16, VerticalAlignment = VerticalAlignment.Center };
    readonly StackPanel actions = new() { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer clock;
    string? drawn;

    public static SettingsCard For(Ctx ctx, string tracker) => new AccountCard(ctx, tracker).card;

    AccountCard(Ctx ctx, string tracker)
    {
        this.ctx = ctx;
        this.tracker = tracker;
        name = KaeriFfiMethods.TrackerName(tracker);
        var end = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        end.Children.Add(ring);
        end.Children.Add(actions);
        card = new SettingsCard { Header = name, Content = end };
        clock = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread().CreateTimer();
        clock.Interval = TimeSpan.FromSeconds(30);
        clock.Tick += (_, _) => Fill();
        card.Loaded += (_, _) =>
        {
            ctx.Batch += Heard;
            clock.Start();
            Fill();
        };
        card.Unloaded += (_, _) =>
        {
            ctx.Batch -= Heard;
            clock.Stop();
        };
        Fill();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.Sync or Event.JobDone or Event.Authorize or Event.DeviceCode))
            Fill();
    }

    /// <summary>The words and buttons for how the account stands now, drawn only when they changed.</summary>
    void Fill()
    {
        var state = ctx.Known.Sync.GetValueOrDefault(tracker);
        var signingIn = ctx.Known.SigningIn.ContainsKey(tracker);
        var code = tracker is "simkl" or "trakt" && DeviceCodeDialog.Pending?.Tracker == tracker;
        string description;
        var buttons = new List<(string Text, Action Act)>();
        if (state is { NeedsReauth: true })
        {
            description = T.AccountsStateExpired(name);
            buttons.Add((T.AccountsSignInAgain, SignIn));
            buttons.Add((T.AccountsSignOut, () => ctx.Send(new Command.Disconnect(tracker))));
        }
        else if (state is { Connected: true })
        {
            description = Status(state) is { } status ? $"{SignedIn(state)} · {status}" : SignedIn(state);
            buttons.Add((T.AccountsSignOut, () => ctx.Send(new Command.Disconnect(tracker))));
        }
        else
        {
            description = signingIn ? (code ? T.AccountsCodeWaiting(name) : T.AccountsStateWaitingBrowser)
                : tracker is "simkl" or "trakt" ? T.SettingsSignedOutCode : T.AccountsStateSignedOut;
            if (signingIn && code)
                buttons.Add((T.AccountsShowCode, () => DeviceCodeDialog.Reopen(card.XamlRoot)));
            else
                buttons.Add((T.AccountsSignIn, SignIn));
        }
        var busy = signingIn && state is not { Connected: true, NeedsReauth: false };
        // The line changes in place ("Synced 2 min ago" moves on by itself); the buttons are made
        // again only when which buttons there are changes, so one under the pointer stays.
        if (card.Description as string != description)
            card.Description = description;
        if (ring.IsActive != busy)
        {
            ring.IsActive = busy;
            ring.Visibility = busy ? Visibility.Visible : Visibility.Collapsed;
        }
        var key = string.Join("|", buttons.Select(b => b.Text)); // i18n-skip
        if (key == drawn)
            return;
        drawn = key;
        actions.Children.Clear();
        foreach (var (text, act) in buttons)
        {
            var button = new Button { Content = text };
            button.Click += (_, _) => act();
            actions.Children.Add(button);
        }
    }

    /// <summary>Kitsu takes a user name and password; the others sign in in the browser or with a code.</summary>
    void SignIn()
    {
        if (tracker == "kitsu")
            KitsuDialog.Show(ctx, card.XamlRoot);
        else
        {
            ctx.SignIn(tracker);
            Fill();
        }
    }

    /// <summary>Who is signed in, with the changes waiting to go out.</summary>
    string SignedIn(SyncState state)
    {
        var waiting = (long)state.Outbox;
        return (state.User, waiting) switch
        {
            ({ } user, > 0) => T.AccountsStateSignedInWaiting(name, user, waiting),
            ({ } user, _) => T.AccountsStateSignedIn(name, user),
            (null, > 0) => T.SettingsSignedInWaiting(waiting),
            _ => T.SettingsSignedIn,
        };
    }

    /// <summary>How sync stands, as the Mac's account row says it: failed, waiting, held for a match, or when it last synced.</summary>
    string? Status(SyncState state)
    {
        if (state.PullError is { } error)
            return T.AccountsSyncFailed(FirstLine(error.Reason));
        if (state.Outbox > 0)
            return null;
        // Held for a match: not waiting, and said apart, so the card is not stuck on a count.
        if (state.Deferred > 0)
            return T.AccountsChangesHeld((long)state.Deferred);
        if (ctx.Known.SyncedAt.TryGetValue(tracker, out var at))
            return T.DevicesRowSynced(Wording.Ago((long)(DateTimeOffset.UtcNow - at).TotalSeconds));
        return T.AccountsSyncDone;
    }

    static string FirstLine(string reason) =>
        reason.Split('\n', 2)[0].Trim() is { Length: > 0 } line ? line : reason;
}
