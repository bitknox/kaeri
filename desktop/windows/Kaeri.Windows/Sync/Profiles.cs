using System.Diagnostics;
using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Kaeri.Windows.Pages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Sync;

/// <summary>
/// Profiles, optional. One person is the default and meets no picker, no names and no prompt:
/// Settings says Only you and offers Add a profile. From the second profile on, the Profiles pane
/// lists them, the account item in the navigation footer names the one in use and opens a flyout
/// to switch, and a switch starts the app again on the profile chosen, so nothing of the last
/// stays in memory. The engine keeps every profile; this shows what it last said.
/// </summary>
static partial class Profiles
{
    static Ctx Ctx => Ctx.Current;
    static XamlRoot? Root => Ctx.Window.Content?.XamlRoot;

    /// <summary>Hears the engine for a switch, once, and asks what the profiles are.</summary>
    public static void Install()
    {
        Ctx.Batch += batch =>
        {
            foreach (var e in batch)
                if (e is Event.ProfileSwitch)
                    Restart();
        };
        Ctx.Send(new Command.LoadProfiles());
    }

    /// <summary>Initials on one raised colour, never a colour of each profile's own.</summary>
    public static Border Avatar(ProfileView profile, double size)
    {
        var circle = new Border
        {
            Width = size,
            Height = size,
            CornerRadius = new CornerRadius(size / 2),
            VerticalAlignment = VerticalAlignment.Center,
            Background = Ui.Res<Brush>("ControlFillColorSecondaryBrush"),
            Child = new TextBlock
            {
                Text = ProfileWords.Initials(profile),
                FontSize = Math.Round(size * 0.42),
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            },
        };
        AutomationProperties.SetAccessibilityView(circle, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        return circle;
    }

    // ---------------------------------------------------------------- add and edit

    static ContentDialog? editing;

    /// <summary>Add a profile, or edit <paramref name="existing"/>: its name and whether it is a kids profile.</summary>
    public static void Edit(ProfileView? existing)
    {
        editing?.Hide();
        // The first add also names the profile that holds today's history, from AniList when signed in.
        var first = existing is null && !ProfileWords.Picking(Ctx.Known.Profiles);
        var body = new StackPanel { Spacing = 12 };
        var name = new TextBox { Header = T.ProfilesNameField, MaxLength = 40, Text = existing?.Name ?? "" };
        body.Children.Add(name);
        var kids = new ToggleSwitch { IsOn = existing?.Kids ?? false, OnContent = "", OffContent = "", MinWidth = 0 };
        AutomationProperties.SetName(kids, T.ProfilesKids);
        var kidsCard = new CommunityToolkit.WinUI.Controls.SettingsCard
        {
            Header = T.ProfilesKids,
            Description = T.ProfilesKidsLine,
            Content = kids,
        };
        kidsCard.Resources["SettingsCardWrapThreshold"] = 0.0;
        kidsCard.Resources["SettingsCardWrapNoIconThreshold"] = 0.0;
        body.Children.Add(kidsCard);
        var signedIn = Ctx.Known.Anilist?.User is { Length: > 0 } user ? user : null;
        var yours = new TextBox { Header = T.ProfilesYours, MaxLength = 40, Text = signedIn ?? "" };
        if (first)
        {
            body.Children.Add(yours);
            // Said only when the name came from AniList.
            if (signedIn is not null)
                body.Children.Add(Ui.Text(T.ProfilesYoursNote, "HazeCaptionStyle"));
        }
        var dialog = new ContentDialog
        {
            XamlRoot = Root,
            Title = existing?.Name ?? T.ProfilesAdd,
            Content = new Border { Child = body, Width = 400 },
            PrimaryButtonText = existing is null ? T.ProfilesAddButton : T.ActionSave,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
            PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle"),
            IsPrimaryButtonEnabled = ProfileWords.Typed(name.Text) is not null,
        };
        name.TextChanged += (_, _) => dialog.IsPrimaryButtonEnabled = ProfileWords.Typed(name.Text) is not null;
        dialog.Loaded += (_, _) => name.Focus(FocusState.Programmatic);
        dialog.PrimaryButtonClick += (_, _) =>
        {
            if (ProfileWords.Typed(name.Text) is not { } typed)
                return;
            Ctx.Send(existing is { } p
                ? new Command.EditProfile(p.Id, typed, kids.IsOn)
                : new Command.AddProfile(typed, kids.IsOn, first ? ProfileWords.Typed(yours.Text) : null));
        };
        dialog.Closed += (_, _) =>
        {
            if (editing == dialog)
                editing = null;
        };
        editing = dialog;
        _ = Dialogs.Show(dialog);
    }

    /// <summary>Delete a profile that is not in use, after asking.</summary>
    public static async void Delete(ProfileView profile)
    {
        if (Root is not { } root)
            return;
        if (await Ui.Confirm(root, T.ProfilesDeleteAlert(profile.Name), T.ProfilesDeleteAlertLine(profile.Name), T.ProfilesDelete))
            Ctx.Send(new Command.DeleteProfile(profile.Id));
    }

    /// <summary>The menu on a profile's card: Edit, and Delete for one not in use.</summary>
    public static Button More(ProfileView profile)
    {
        var menu = new MenuFlyout();
        var edit = new MenuFlyoutItem { Text = T.ProfilesEditAction };
        edit.Click += (_, _) => Edit(profile);
        menu.Items.Add(edit);
        if (!profile.Active)
        {
            var delete = new MenuFlyoutItem { Text = T.ProfilesDelete, Foreground = Ui.Res<Brush>("SystemFillColorCriticalBrush") };
            delete.Click += (_, _) => Delete(profile);
            menu.Items.Add(delete);
        }
        var more = new Button
        {
            Content = new FontIcon { Glyph = "", FontSize = 14 },
            Flyout = menu,
            Padding = new Thickness(8),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetName(more, T.ProfilesMoreFor(profile.Name));
        ToolTipService.SetToolTip(more, T.ProfilesMoreFor(profile.Name));
        return more;
    }

    // ---------------------------------------------------------------- the account flyout

    /// <summary>
    /// The account item's flyout with two or more profiles: each profile with its initials, the
    /// one in use ticked, then Manage profiles. Choosing another switches to it.
    /// </summary>
    public static Flyout Flyout()
    {
        var list = new StackPanel { Spacing = 2, Width = 252 };
        foreach (var p in Ctx.Known.Profiles)
        {
            var row = new Grid { ColumnSpacing = 12 };
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            row.Children.Add(Avatar(p, 24));
            var name = Ui.Text(p.Name, "BodyTextBlockStyle", wrap: false);
            name.VerticalAlignment = VerticalAlignment.Center;
            name.TextTrimming = TextTrimming.CharacterEllipsis;
            Grid.SetColumn(name, 1);
            row.Children.Add(name);
            if (p.Kids)
            {
                var tag = Ui.Text(T.ProfilesKidsTag, "HazeCaptionStyle", wrap: false);
                tag.VerticalAlignment = VerticalAlignment.Center;
                Grid.SetColumn(tag, 2);
                row.Children.Add(tag);
            }
            if (p.Active)
            {
                var check = new FontIcon { Glyph = "", FontSize = 14, VerticalAlignment = VerticalAlignment.Center };
                Grid.SetColumn(check, 3);
                row.Children.Add(check);
            }
            list.Children.Add(Row(row, p.Active ? p.Name : T.ProfilesAvatarDesc(p.Name), p.Active ? null : p.Id));
        }
        list.Children.Add(new Border { Height = 1, Margin = new Thickness(0, 4, 0, 4), Background = Ui.Res<Brush>("DividerStrokeColorDefaultBrush") });
        var manage = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        manage.Children.Add(new FontIcon { Glyph = "", FontSize = 16 });
        manage.Children.Add(Ui.Text(T.ProfilesManage, "BodyTextBlockStyle", wrap: false));
        var flyout = new Flyout { Placement = FlyoutPlacementMode.TopEdgeAlignedLeft };
        var manageButton = Row(manage, T.ProfilesManage, null);
        manageButton.Click += (_, _) =>
        {
            flyout.Hide();
            Ctx.Window.OpenSettings("profiles");
        };
        list.Children.Add(manageButton);
        flyout.Content = list;
        var style = new Style(typeof(FlyoutPresenter));
        style.Setters.Add(new Setter(Control.PaddingProperty, new Thickness(4)));
        style.Setters.Add(new Setter(FrameworkElement.MinWidthProperty, 0.0));
        flyout.FlyoutPresenterStyle = style;
        foreach (var b in list.Children.OfType<Button>())
            if (b.Tag is string id)
                b.Click += (_, _) =>
                {
                    flyout.Hide();
                    Ctx.Send(new Command.SwitchProfile(id));
                };
        return flyout;
    }

    static Button Row(UIElement content, string name, string? switchTo)
    {
        var b = new Button
        {
            Content = content,
            Tag = switchTo,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Padding = new Thickness(12, 8, 12, 8),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
        };
        AutomationProperties.SetName(b, name);
        return b;
    }

    // ---------------------------------------------------------------- a switch

    /// <summary>
    /// The engine switched: the app opens again on the profile chosen once this copy has ended,
    /// so nothing of the last profile stays in memory. A fixtures scene stays as it is.
    /// </summary>
    static void Restart() => Relaunch.Now("a profile switch");
}
