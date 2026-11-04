using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Kaeri.Windows.Pages;

/// <summary>
/// One media server: its address, version and who is signed in, its libraries as check rows,
/// Report what you watch, and Remove server. A server that signed the viewer out offers Sign in again.
/// </summary>
public sealed partial class ServerPage : Page
{
    readonly Ctx ctx = Ctx.Current;
    string id = "";
    bool quiet;

    public ServerPage()
    {
        InitializeComponent();
        State.Placeholders = Placeholders;
        // The saved servers are on this device.
        State.NeedsConnection = false;
    }

    /// <summary>The server's name and line, Sign out, and its libraries as check rows, as plates.</summary>
    static UIElement Placeholders()
    {
        var plates = new StackPanel { Spacing = 12 };
        plates.Children.Add(States.Title(220));
        plates.Children.Add(States.Plate(90, 32, 4));
        plates.Children.Add(States.Heading());
        plates.Children.Add(States.Rows(3, 20, 20, rowHeight: 44, spacing: 4));
        return plates;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        id = e.Parameter as string ?? "";
        ctx.Hear(this, batch =>
        {
            if (batch.Any(x => x is Event.Server))
                Draw();
        });
        ctx.Send(new Command.Server(new ServerCommand.Load()));
        Draw();
        // The wait for AniList's limit counts down on the page.
        timer ??= Countdown.Start(this, () => ctx.Known.AnyWaiting(DateTimeOffset.UtcNow), Draw);
    }

    Microsoft.UI.Dispatching.DispatcherQueueTimer? timer;

    void Draw()
    {
        quiet = true;
        Stack.Children.Clear();
        if (!ctx.Known.ServersLoaded)
        {
            State.Wait();
            quiet = false;
            return;
        }
        State.Done();
        var row = ctx.Known.Servers.FirstOrDefault(s => s.Id == id);
        if (row is null)
        {
            quiet = false;
            return;
        }
        var title = Ui.Text(row.Name, "PageTitleStyle");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetHeadingLevel(title, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level1);
        Stack.Children.Add(title);
        Stack.Children.Add(Ui.Haze(T.ServersPageLine(row.Address, row.Version, row.User)));
        if (row.SignedOut || row.SignedOutHere)
        {
            var out_ = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
            // The server ending the sign-in is a fault; ending it here is the viewer's own choice.
            out_.Children.Add(row.SignedOutHere ? Ui.Dot(Colours.Haze, ring: true) : Ui.Dot(Ui.Vermilion));
            out_.Children.Add(Ui.Text(row.SignedOutHere ? T.ServersRowSignedOutHere : T.ServersRowSignedOut, "BodyTextBlockStyle"));
            Stack.Children.Add(out_);
            var again = Ui.Label(T.ServersSignInAgain, (_, _) => ctx.Window.ShowAddServer(row.Address), accent: true);
            again.HorizontalAlignment = HorizontalAlignment.Left;
            Stack.Children.Add(again);
        }
        else
        {
            if (Lawful.ServerMatching(row, ctx.Known, DateTimeOffset.UtcNow) is { } matching)
                Stack.Children.Add(Ui.Haze(matching));
            var signOut = Ui.Label(T.ServersSignOut, (_, _) => ctx.Send(new Command.Server(new ServerCommand.SignOut(id))));
            signOut.HorizontalAlignment = HorizontalAlignment.Left;
            Stack.Children.Add(signOut);
        }

        Stack.Children.Add(Ui.Heading(T.ServersLibrariesTitle));
        foreach (var library in row.Libraries)
        {
            var line = new StackPanel { Spacing = 0 };
            line.Children.Add(Ui.Text(library.Name, wrap: false));
            line.Children.Add(Ui.Text(Lawful.LibraryKind(library.Kind), "HazeCaptionStyle", wrap: false));
            var box = new CheckBox { IsChecked = library.Ticked, Content = line, Tag = library.Id };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, $"{library.Name}, {Lawful.LibraryKind(library.Kind)}");
            box.Click += (_, _) => SaveLibraries();
            Stack.Children.Add(box);
        }

        var report = new ToggleSwitch { IsOn = row.Report, OnContent = "", OffContent = "", Header = T.ServersReport, Margin = new Thickness(0, 12, 0, 0) };
        report.Toggled += (_, _) =>
        {
            if (!quiet)
                ctx.Send(new Command.Server(new ServerCommand.SetReport(id, report.IsOn)));
        };
        Stack.Children.Add(report);
        Stack.Children.Add(Ui.Haze(T.ServersReportLine));

        var remove = Ui.Label(T.ServersRemove, (_, _) => Remove(row));
        remove.HorizontalAlignment = HorizontalAlignment.Left;
        remove.Margin = new Thickness(0, 16, 0, 0);
        Stack.Children.Add(remove);
        quiet = false;
    }

    void SaveLibraries()
    {
        var ticked = Stack.Children.OfType<CheckBox>().Where(b => b.IsChecked == true).Select(b => (string)b.Tag).ToArray();
        ctx.Send(new Command.Server(new ServerCommand.SetLibraries(id, ticked)));
    }

    /// <summary>Nothing is deleted from the server, so no question is asked.</summary>
    void Remove(ServerRow row)
    {
        ctx.Send(new Command.Server(new ServerCommand.Remove(id)));
        // The engine holds the server and its sign-in for the notice's seconds; Undo brings it back without a password.
        var back = id;
        ctx.Window.Say(T.SourcesRemoved(row.Name), T.ActionUndo, () => ctx.Send(new Command.Server(new ServerCommand.Restore(back))));
        ctx.Go("providers");
    }
}
