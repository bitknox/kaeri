using CommunityToolkit.WinUI.Controls;
using Kaeri.App;
using Kaeri.Engine.Model;
using Kaeri.Windows.Sync;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Settings, Profiles. With one profile it says Only you and offers Add a profile, nothing more.
/// From two on, a card for each with its initials, Kids when it is one, the check on the one in
/// use or Switch profile, and a menu to edit or delete; then Add profile.
/// </summary>
public sealed partial class SettingsPage
{
    void ShowProfiles()
    {
        var profiles = ctx.Known.Profiles;
        Cards.Children.Add(Heading(T.ProfilesTitle));
        if (!ProfileWords.Picking(profiles))
        {
            // One profile has no name to show, so the account glyph stands in for its initials.
            Cards.Children.Add(CardFrame(T.ProfilesOnlyYou, null, null, "\uE77B"));
            Cards.Children.Add(Foot(T.ProfilesOneFoot));
            var add = CardFrame(T.ProfilesAdd, T.ProfilesAddLine, null, "");
            add.IsClickEnabled = true;
            add.Margin = new Thickness(0, 12, 0, 0);
            add.Click += (_, _) => Profiles.Edit(null);
            Cards.Children.Add(add);
            return;
        }
        foreach (var p in profiles)
            Cards.Children.Add(ProfileCard(p));
        Cards.Children.Add(Foot(T.ProfilesSharedFoot));
        var button = new Button { Content = T.ProfilesAddButton, Margin = new Thickness(0, 12, 0, 0) };
        button.Click += (_, _) => Profiles.Edit(null);
        Cards.Children.Add(button);
    }

    static TextBlock Foot(string text)
    {
        var foot = Ui.Text(text, "HazeCaptionStyle");
        foot.Margin = new Thickness(2, 4, 0, 0);
        return foot;
    }

    /// <summary>Initials, then the name, and Kids after it for a kids profile.</summary>
    static StackPanel Named(UIElement avatar, string name, bool kids)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        row.Children.Add(avatar);
        row.Children.Add(new TextBlock { Text = name, VerticalAlignment = VerticalAlignment.Center });
        if (kids)
        {
            var tag = Ui.Text(T.ProfilesKidsTag, "HazeCaptionStyle", wrap: false);
            tag.VerticalAlignment = VerticalAlignment.Center;
            row.Children.Add(tag);
        }
        return row;
    }

    SettingsCard ProfileCard(ProfileView p)
    {
        var end = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
        if (p.Active)
        {
            var check = new FontIcon { Glyph = "", FontSize = 16, Foreground = Res<Microsoft.UI.Xaml.Media.Brush>("AccentTextFillColorPrimaryBrush"), VerticalAlignment = VerticalAlignment.Center };
            end.Children.Add(check);
        }
        else
        {
            var go = new Button { Content = T.ProfilesSwitch, VerticalAlignment = VerticalAlignment.Center };
            AutomationProperties.SetName(go, T.ProfilesAvatarDesc(p.Name));
            go.Click += (_, _) => ctx.Send(new Command.SwitchProfile(p.Id));
            end.Children.Add(go);
        }
        end.Children.Add(Profiles.More(p));
        return new SettingsCard { Header = Named(Profiles.Avatar(p, 32), p.Name, p.Kids), Content = end };
    }
}
