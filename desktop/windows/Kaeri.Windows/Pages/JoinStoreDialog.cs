using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Joining a plugin store in two steps, as the other apps do: paste the invite link and let the
/// engine check it, then see who runs the store, what it offers and which hosts those plugins
/// contact, and confirm. Nothing is joined or installed until the second step's Join, and the
/// dialog stays up with a ring until the engine says the store was joined or why it was not.
/// Both steps sit in a box of one size, so the dialog does not jump between them.
/// </summary>
sealed class JoinStoreDialog
{
    readonly Ctx ctx;
    readonly ContentDialog dialog;
    readonly TextBox invite = new() { PlaceholderText = T.StoresInviteLinkHint, AcceptsReturn = false };
    readonly TextBlock problem = Ui.Text("", "HazeCaptionStyle");
    readonly StackPanel body = new() { Spacing = 12 };
    readonly ProgressRing ring = new() { Width = 16, Height = 16, HorizontalAlignment = HorizontalAlignment.Left, Visibility = Visibility.Collapsed };
    InvitedStore? preview;
    TextBox? deviceName, yourName;
    /// <summary>Join was pressed and the engine has not answered yet; its job, when it gave one.</summary>
    bool joining;
    ulong? joinJob;
    /// <summary>The last alert while joining: why the store would not take this device.</summary>
    string? refusal;

    /// <summary>The store to open once the dialog has closed: the one just joined, or the one joined before.</summary>
    public string? Store { get; private set; }

    public JoinStoreDialog(Ctx ctx, XamlRoot root, string? pasted = null)
    {
        this.ctx = ctx;
        AutomationProperties.SetName(invite, T.StoresInviteLink);
        invite.Text = pasted ?? "";
        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.StoresAddTitle,
            Content = new ScrollViewer { Content = body, Width = 460, Height = 360, VerticalScrollBarVisibility = ScrollBarVisibility.Auto },
            PrimaryButtonText = T.StoresCheckInvite,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
        };
        dialog.PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle");
        dialog.PrimaryButtonClick += OnPrimary;
        invite.TextChanged += (_, _) => Update();
        FirstStep();
    }

    /// <summary>Show it; with <paramref name="checkAtOnce"/> the invite is checked as soon as it is up, for a screenshot.</summary>
    public async Task ShowAsync(bool checkAtOnce = false)
    {
        if (checkAtOnce)
            _ = Task.Delay(700).ContinueWith(_ => dialog.DispatcherQueue.TryEnqueue(Check));
        ctx.Known.Preview = null;
        ctx.Known.Joined = null;
        Store = null;
        ctx.Batch += Heard;
        try
        {
            await Dialogs.Show(dialog);
        }
        finally
        {
            ctx.Batch -= Heard;
        }
    }

    /// <summary>Another invite link opened while this is up: back to the first step, filled in with it.</summary>
    public void Fill(string link)
    {
        // A join on its way finishes first.
        if (joining)
            return;
        invite.Text = link;
        FirstStep();
    }

    void FirstStep()
    {
        preview = null;
        body.Children.Clear();
        body.Children.Add(Ui.Text(T.StoresJoinDetailDesktop, "BodyTextBlockStyle"));
        // The rule of an invite, above the field.
        body.Children.Add(Ui.Haze(T.StoresInviteRules));
        body.Children.Add(invite);
        body.Children.Add(problem);
        dialog.Title = T.StoresAddTitle;
        dialog.PrimaryButtonText = T.StoresCheckInvite;
        Update();
    }

    void Update()
    {
        if (preview is not null)
            return;
        dialog.IsPrimaryButtonEnabled = Stores.InviteIn(invite.Text) is not null;
        problem.Text = invite.Text.Trim().Length > 0 && !dialog.IsPrimaryButtonEnabled ? T.StoresNotAnInvite : "";
    }

    void OnPrimary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        if (preview is null)
        {
            args.Cancel = true;
            Check();
            return;
        }
        // Joined before: Open store goes to it, and nothing is sent.
        if (preview.AlreadyJoined)
        {
            Store = ctx.Known.Stores.FirstOrDefault(s => s.Url == preview.Url)?.Id
                ?? ctx.Known.Stores.FirstOrDefault(s => s.Fingerprint == preview.Fingerprint)?.Id;
            return;
        }
        args.Cancel = true;
        if (joining)
            return;
        var label = preview.Anonymous ? null : deviceName?.Text.Trim() is { Length: > 0 } name ? name : null;
        var joiner = preview.NamesJoiner && yourName?.Text.Trim() is { Length: > 0 } given ? given : null;
        joining = true;
        refusal = null;
        problem.Text = "";
        Working(true);
        joinJob = ctx.Send(new Command.JoinStore(preview.Link, label, joiner));
    }

    /// <summary>The ring while the join runs, with Join and the fields held.</summary>
    void Working(bool on)
    {
        ring.IsActive = on;
        ring.Visibility = on ? Visibility.Visible : Visibility.Collapsed;
        dialog.IsPrimaryButtonEnabled = !on;
        if (deviceName is not null)
            deviceName.IsEnabled = !on;
        if (yourName is not null)
            yourName.IsEnabled = !on;
    }

    /// <summary>Step one: ask the engine what the link leads to, and stay until it answers.</summary>
    void Check()
    {
        if (preview is not null || Stores.InviteIn(invite.Text) is not { } link)
            return;
        problem.Text = T.StoresCheckingLink;
        dialog.IsPrimaryButtonEnabled = false;
        ctx.Send(new Command.PreviewStore(link));
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.StorePreview { V1: InvitePreview.Store s } when !joining:
                    SecondStep(s.V1);
                    break;
                case Event.StorePreview { V1: InvitePreview.Failed f }:
                    problem.Text = Failure(f.Reason) + (f.Contact is { Length: > 0 } contact ? "\n" + T.StoreInviteContact(contact) : "");
                    if (joining)
                    {
                        // The invite stopped working since it was checked: joining again won't help.
                        joining = false;
                        Working(false);
                        dialog.IsPrimaryButtonEnabled = false;
                    }
                    else
                        dialog.IsPrimaryButtonEnabled = true;
                    break;
                case Event.StoreJoined joined:
                    joining = false;
                    Store = joined.Store;
                    dialog.Hide();
                    break;
                case Event.Toast { V1.Kind: NoticeKind.Alert } toast when joining:
                    refusal = toast.V1.Msg?.Text();
                    break;
                case Event.JobDone done when joining && joinJob is { } job && done.V1 == job:
                    // Over without a join: why, in place, and Join to try again.
                    joining = false;
                    Working(false);
                    problem.Text = refusal is { Length: > 0 } why ? why : T.StoresUnreachable;
                    break;
            }
        }
    }

    void SecondStep(InvitedStore store)
    {
        preview = store;
        body.Children.Clear();
        dialog.Title = store.AlreadyJoined ? T.StoresAlreadyJoinedTitle(store.Name) : T.StoresJoinConfirmTitle(store.Name);
        body.Children.Add(Ui.Haze(Stores.RunBy(store.Owners, store.Host)));
        body.Children.Add(Ui.Text(T.StoresKeyMatches(store.Fingerprint), "HazeCaptionStyle"));
        if (store.Open)
            body.Children.Add(Ui.Haze(T.StoresAnyoneNote));
        var plugins = store.Plugins ?? [];
        if (plugins.Length > 0)
        {
            body.Children.Add(Ui.Text(T.StoresOffersPlugins(plugins.Length), "BodyStrongTextBlockStyle"));
            var list = new StackPanel { Spacing = 8 };
            foreach (var plugin in plugins)
            {
                var line = new StackPanel();
                line.Children.Add(Ui.Text(T.StoresPluginNameVersion(plugin.Name, plugin.Version), wrap: false));
                line.Children.Add(Ui.Text(Stores.ContactsLine(plugin), "HazeCaptionStyle", wrap: false));
                list.Children.Add(line);
            }
            body.Children.Add(new ScrollViewer { Content = list, MaxHeight = 200, VerticalScrollBarVisibility = ScrollBarVisibility.Auto });
        }
        body.Children.Add(Ui.Haze(T.StoresJoinInstallsNothing));
        if (!store.Anonymous && !store.AlreadyJoined)
        {
            deviceName = new TextBox { Text = store.DeviceLabel };
            AutomationProperties.SetName(deviceName, T.StoresDeviceName);
            body.Children.Add(Ui.Text(T.StoresOwnerSeesAs, "HazeCaptionStyle"));
            body.Children.Add(deviceName);
        }
        // A link that works more than once names whoever joins: their own name, optional.
        if (store.NamesJoiner && !store.AlreadyJoined)
        {
            yourName = new TextBox { PlaceholderText = T.StoresJoinYourName };
            AutomationProperties.SetName(yourName, T.StoresJoinYourName);
            body.Children.Add(yourName);
            body.Children.Add(Ui.Text(T.StoresJoinYourNameDetail, "HazeCaptionStyle"));
        }
        // The ring and why a join failed, under the rest.
        problem.Text = "";
        body.Children.Add(ring);
        body.Children.Add(problem);
        dialog.PrimaryButtonText = store.AlreadyJoined ? T.StoresOpenStore : T.StoresJoinButton;
        dialog.IsPrimaryButtonEnabled = true;
        dialog.DefaultButton = ContentDialogButton.Primary;
    }

    /// <summary>
    /// Why an invite would not join, in a line that says what to do next. The engine sends its
    /// reason as English text, so the known ones are told apart by their English start.
    /// </summary>
    static string Failure(string reason)
    {
        // The engine words an invite that stopped working, with who to ask.
        if (reason.StartsWith("This invite", StringComparison.Ordinal))
            return reason;
        if (reason.StartsWith("that invite was already used", StringComparison.OrdinalIgnoreCase))
            return T.StoreInviteUnusable;
        if (reason.StartsWith("that link no longer lets anyone join", StringComparison.OrdinalIgnoreCase))
            return T.StoreOpenLinkUnusable;
        if (reason.Contains("not a link", StringComparison.OrdinalIgnoreCase) || reason.Contains("invite", StringComparison.OrdinalIgnoreCase) && reason.Contains("parse", StringComparison.OrdinalIgnoreCase))
            return T.StoresNotAnInvite;
        return T.StoresUnreachable;
    }
}
