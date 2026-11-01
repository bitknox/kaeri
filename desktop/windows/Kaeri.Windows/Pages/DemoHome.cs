using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;

namespace Kaeri.Windows.Pages;

/// <summary>
/// The built-in demo on Home. Windows has no Home spotlight, so the demo is a band above
/// Continue until it is played or a store source is installed: its label, title, credit block
/// with the licence links, Play part 1, Details, and a menu with About the demo and Remove demo.
/// While the demo is all there is, a closing line points to plugin stores.
/// </summary>
static class DemoHome
{
    /// <summary>The work's name: the first part of the demo's line in Sources.</summary>
    public static string Title => T.SourcesDemoLine.Split(" · ")[0];

    public static FrameworkElement Band(Ctx ctx)
    {
        var info = KaeriFfiMethods.DemoInfo();
        var stack = new StackPanel { Spacing = 8 };
        stack.Children.Add(Ui.Haze(T.HomeSpotlightDemo, wrap: false));
        stack.Children.Add(Ui.Text(Title, "ShowTitleStyle"));
        stack.Children.Add(Credit(info));
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 4, 0, 0) };
        buttons.Children.Add(Ui.Label(T.TitlePlayPart("1"), (_, _) =>
        {
            ctx.Known.DemoPlay = true;
            ctx.Send(new Command.PlayDemo());
        }, accent: true));
        buttons.Children.Add(Ui.Label(T.TitleOpen, (_, _) => ctx.Window.ShowSource("demo")));
        var more = new Button { Content = new FontIcon { Glyph = "", FontSize = 14 }, Width = 36, Padding = new Thickness(0) };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(more, T.ActionMore);
        var menu = new MenuFlyout();
        var about = new MenuFlyoutItem { Text = T.SourcesDemoAbout };
        about.Click += (_, _) => ctx.Window.ShowSource("demo");
        var remove = new MenuFlyoutItem { Text = T.SourcesDemoRemove };
        remove.Click += (_, _) => ctx.Send(new Command.RemoveDemo());
        menu.Items.Add(about);
        menu.Items.Add(remove);
        more.Flyout = menu;
        buttons.Children.Add(more);
        stack.Children.Add(buttons);
        return Ui.Card(stack, 16);
    }

    /// <summary>The credit block, with its three links: the licence, Wikimedia Commons and the makers.</summary>
    public static FrameworkElement Credit(DemoInfo info)
    {
        var panel = new StackPanel { Spacing = 2 };
        panel.Children.Add(Ui.Haze(T.SourcesDemoCredit));
        var links = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        foreach (var (text, url) in new[]
        {
            (T.SourcesDemoLinkLicence, info.LicenceUrl),
            (T.SourcesDemoLinkCommons, info.FilePageUrl),
            (T.SourcesDemoLinkMaker, info.MakerUrl),
        })
            links.Children.Add(new HyperlinkButton { Content = text, NavigateUri = new Uri(url), Padding = new Thickness(0) });
        panel.Children.Add(links);
        return panel;
    }

    /// <summary>"Only the demo is on. Join a plugin store to ..." with Plugin stores as its link.</summary>
    public static FrameworkElement Closing(Ctx ctx)
    {
        var line = Ui.Haze(T.SourcesDemoOnly);
        var link = new HyperlinkButton { Content = T.NavPluginStores, Padding = new Thickness(0) };
        link.Click += (_, _) => ctx.Window.Go("stores");
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        row.Children.Add(line);
        row.Children.Add(link);
        return row;
    }
}
