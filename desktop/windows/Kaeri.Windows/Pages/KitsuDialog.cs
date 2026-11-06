using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Kitsu's sign-in: an email or user name and a password, sent once for a token
/// (<c>ConnectKitsu</c>). Kitsu has no browser sign-in for apps, so this is the only way in. The
/// form stays up with a ring while the engine asks Kitsu, closes once signed in, and says under
/// the fields why it did not work, with the name typed still there to correct.
/// </summary>
static class KitsuDialog
{
    public static async void Show(Ctx ctx, XamlRoot root)
    {
        var kitsu = KaeriFfiMethods.TrackerName("kitsu");
        var user = new TextBox { Header = T.AccountsUserOrEmail, InputScope = Scope(Microsoft.UI.Xaml.Input.InputScopeNameValue.EmailSmtpAddress) };
        var password = new PasswordBox { Header = T.AccountsPassword };
        var ring = new ProgressRing { Width = 16, Height = 16, HorizontalAlignment = HorizontalAlignment.Left, Visibility = Visibility.Collapsed };
        var problem = Ui.Text("", "HazeCaptionStyle");
        var body = new StackPanel { Spacing = 12, MinWidth = 340 };
        body.Children.Add(Ui.Text(T.AccountsFormDetail(kitsu), "BodyTextBlockStyle"));
        body.Children.Add(user);
        body.Children.Add(password);
        body.Children.Add(ring);
        body.Children.Add(problem);
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.AccountsFormTitle(kitsu),
            Content = body,
            PrimaryButtonText = T.ActionSignIn,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
            IsPrimaryButtonEnabled = false,
        };
        ulong? job = null;
        var working = false;
        string? refusal = null;
        void Complete() => dialog.IsPrimaryButtonEnabled = !working && user.Text.Trim().Length > 0 && password.Password.Length > 0;
        void Working(bool on)
        {
            working = on;
            ring.IsActive = on;
            ring.Visibility = on ? Visibility.Visible : Visibility.Collapsed;
            user.IsEnabled = !on;
            password.IsEnabled = !on;
            Complete();
        }
        void SignIn()
        {
            if (working || password.Password.Length == 0 || user.Text.Trim().Length == 0)
                return;
            refusal = null;
            problem.Text = "";
            Working(true);
            job = ctx.Send(new Command.ConnectKitsu(user.Text.Trim(), password.Password));
            password.Password = "";
            // No job means the engine has gone: nothing will answer.
            if (job is null)
                Working(false);
        }
        void Heard(IReadOnlyList<Event> batch)
        {
            foreach (var e in batch)
            {
                switch (e)
                {
                    case Event.Sync { V1: { Tracker: "kitsu", Connected: true } }:
                        dialog.Hide();
                        return;
                    case Event.Toast { V1.Kind: NoticeKind.Alert } toast when working:
                        refusal = toast.V1.Msg?.Text();
                        break;
                    case Event.JobDone done when working && job is { } mine && done.V1 == mine:
                        Working(false);
                        if (ctx.Known.Sync.GetValueOrDefault("kitsu") is { Connected: true })
                        {
                            dialog.Hide();
                            return;
                        }
                        problem.Text = refusal ?? "";
                        password.Focus(FocusState.Programmatic);
                        break;
                }
            }
        }
        user.TextChanged += (_, _) => Complete();
        password.PasswordChanged += (_, _) => Complete();
        password.KeyDown += (_, e) =>
        {
            if (e.Key == global::Windows.System.VirtualKey.Enter && dialog.IsPrimaryButtonEnabled)
            {
                e.Handled = true;
                SignIn();
            }
        };
        dialog.PrimaryButtonClick += (_, args) =>
        {
            args.Cancel = true;
            SignIn();
        };
        ctx.Batch += Heard;
        try
        {
            await Dialogs.Show(dialog, () => user.Focus(FocusState.Programmatic));
        }
        finally
        {
            ctx.Batch -= Heard;
            password.Password = "";
        }
    }

    static Microsoft.UI.Xaml.Input.InputScope Scope(Microsoft.UI.Xaml.Input.InputScopeNameValue name)
    {
        var scope = new Microsoft.UI.Xaml.Input.InputScope();
        scope.Names.Add(new Microsoft.UI.Xaml.Input.InputScopeName(name));
        return scope;
    }
}
