using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Pages;

/// <summary>
/// First launch, as the spec has it everywhere: three short pages (Welcome, AniList, Sources),
/// each skippable, then the app on Continue. It never mentions torrents. Every step is in
/// Settings afterwards. On Windows the pages are one ContentDialog whose content and buttons change.
/// </summary>
sealed class WelcomeDialog
{
    readonly Ctx ctx;
    readonly ContentDialog dialog;
    readonly StackPanel body = new() { Spacing = 12, MinWidth = 480, MaxWidth = 520 };
    readonly TextBox invite = new() { PlaceholderText = T.StoresInviteLinkHint, AcceptsReturn = false };
    readonly TextBlock problem = Ui.Text("", "HazeCaptionStyle");
    readonly Button join = new() { Content = T.OnboardingJoin, MinWidth = 72, IsEnabled = false };
    int page;
    bool linked;

    /// <summary>An invite link to join once the dialog has closed: the join dialog takes it from there.</summary>
    public string? Invite { get; private set; }

    /// <summary>"folder" or "server" when the viewer chose to add their own media: that dialog opens once this closes.</summary>
    public string? Then { get; private set; }

    public WelcomeDialog(Ctx ctx, XamlRoot root)
    {
        this.ctx = ctx;
        AutomationProperties.SetName(invite, T.OnboardingInviteHint);
        invite.TextChanged += (_, _) => Check();
        join.Click += (_, _) => JoinNow();
        // Enter in the field joins when it holds an invite; with nothing in it, Enter plays the demo.
        invite.KeyDown += (_, e) =>
        {
            if (e.Key == global::Windows.System.VirtualKey.Enter && Stores.InviteIn(invite.Text) is not null)
            {
                e.Handled = true;
                JoinNow();
            }
        };
        dialog = new ContentDialog { XamlRoot = root, Content = body };
        dialog.PrimaryButtonClick += OnPrimary;
        dialog.SecondaryButtonClick += OnSecondary;
        Show(0);
    }

    public Task ShowAsync() => Dialogs.Show(dialog);

    /// <summary>Opens on a later page, for a screenshot: anilist or sources.</summary>
    public void Step(string name) => Show(name switch { "anilist" => 1, "sources" => 2, _ => 0 });

    /// <summary>An invite link opened while this is up: on to the sources page, filled in.</summary>
    public void Fill(string link)
    {
        invite.Text = link;
        linked = true;
        Show(2);
    }

    void Show(int to)
    {
        page = to;
        body.Children.Clear();
        dialog.PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle");
        dialog.SecondaryButtonText = "";
        dialog.CloseButtonText = "";
        dialog.IsPrimaryButtonEnabled = true;
        dialog.DefaultButton = ContentDialogButton.Primary;
        switch (page)
        {
            case 0:
                dialog.Title = null;
                var nagare = new Nagare(5) { HorizontalAlignment = HorizontalAlignment.Left, Margin = new Thickness(0, 4, 0, 4) };
                AutomationProperties.SetAccessibilityView(nagare, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
                body.Children.Add(nagare);
                var name = Ui.Text(T.AppName, "TitleTextBlockStyle");
                AutomationProperties.SetHeadingLevel(name, AutomationHeadingLevel.Level1);
                body.Children.Add(name);
                body.Children.Add(Ui.Text(T.OnboardingTagline, "BodyTextBlockStyle"));
                dialog.PrimaryButtonText = T.OnboardingContinue;
                dialog.CloseButtonText = T.OnboardingSkip;
                break;
            case 1:
                var anilist = KaeriFfiMethods.TrackerName("anilist");
                dialog.Title = anilist;
                body.Children.Add(Ui.Text(T.AccountsWelcomeDetail(anilist), "BodyTextBlockStyle"));
                body.Children.Add(Ui.Text(T.AccountsOtherTrackersNote, "HazeCaptionStyle"));
                dialog.PrimaryButtonText = T.AccountsSignInTo(anilist);
                dialog.SecondaryButtonText = T.OnboardingNotNow;
                break;
            default:
                // Where episodes come from: your own media, the demo to try now, and the invite field for a store.
                dialog.Title = T.OnboardingWhereTitle;
                body.Children.Add(Ui.Text(T.OnboardingWhereHosts, "BodyTextBlockStyle"));
                body.Children.Add(Ui.Haze(T.OnboardingWhereStores));
                var own = Heading(T.OnboardingOwnHeading);
                AutomationProperties.SetHeadingLevel(own, AutomationHeadingLevel.Level2);
                body.Children.Add(own);
                var own2 = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
                own2.Children.Add(Ui.Label(T.SourcesAddFolderButton, (_, _) => Leave("folder")));
                own2.Children.Add(Ui.Label(T.SourcesAddServerButton, (_, _) => Leave("server")));
                body.Children.Add(own2);
                var demo = Heading(T.OnboardingDemoHeading);
                AutomationProperties.SetHeadingLevel(demo, AutomationHeadingLevel.Level2);
                body.Children.Add(demo);
                body.Children.Add(Ui.Text(T.OnboardingDemoDetail, "BodyTextBlockStyle"));
                var link = Heading(T.OnboardingInviteHeading);
                AutomationProperties.SetHeadingLevel(link, AutomationHeadingLevel.Level2);
                body.Children.Add(link);
                var row = new Grid { ColumnSpacing = 8 };
                row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
                row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
                row.Children.Add(invite);
                Grid.SetColumn(join, 1);
                row.Children.Add(join);
                body.Children.Add(row);
                body.Children.Add(problem);
                body.Children.Add(Ui.Haze(T.PluginRiskWarning));
                // One amber button: Play the demo, or Join when an invite link opened this page.
                dialog.PrimaryButtonText = T.OnboardingPlayDemo;
                dialog.PrimaryButtonStyle = linked ? null : Ui.Res<Style>("AccentButtonStyle");
                join.Style = linked ? Ui.Res<Style>("AccentButtonStyle") : null;
                dialog.CloseButtonText = T.OnboardingSkipForNow;
                dialog.DefaultButton = linked ? ContentDialogButton.None : ContentDialogButton.Primary;
                Check();
                break;
        }
    }

    void Leave(string then)
    {
        Then = then;
        dialog.Hide();
    }

    void Check()
    {
        if (page != 2)
            return;
        var link = Stores.InviteIn(invite.Text);
        join.IsEnabled = link is not null;
        problem.Text = invite.Text.Trim().Length > 0 && link is null ? T.StoresNotAnInvite : "";
        problem.Visibility = problem.Text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    void OnPrimary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        switch (page)
        {
            case 0:
                args.Cancel = true;
                Show(1);
                break;
            case 1:
                args.Cancel = true;
                ctx.Send(new Command.Connect("anilist"));
                Show(2);
                break;
            default:
                // Play the demo: first launch closes into the player on part 1.
                ctx.Known.DemoPlay = true;
                ctx.Send(new Command.PlayDemo());
                break;
        }
    }

    /// <summary>The join dialog shows the store before anything is joined; it opens once this closes.</summary>
    void JoinNow()
    {
        if (Stores.InviteIn(invite.Text) is not { } link)
            return;
        Invite = link;
        dialog.Hide();
    }

    static TextBlock Heading(string text) => new()
    {
        Text = text,
        Style = Ui.Res<Style>("BodyStrongTextBlockStyle"),
        Margin = new Thickness(0, 8, 0, 0),
    };

    void OnSecondary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        if (page == 1)
        {
            args.Cancel = true;
            Show(2);
        }
    }
}
