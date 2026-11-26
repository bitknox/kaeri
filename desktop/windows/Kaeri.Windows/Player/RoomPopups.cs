using System.Runtime.InteropServices;
using Kaeri.App;
using Kaeri.Engine.Model;
using Kaeri.Windows.Pages;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;
using Windows.UI;

namespace Kaeri.Windows.Player;

/// <summary>How a close of the player in a room was answered.</summary>
enum RoomCloseAnswer
{
    Cancel,
    Leave,
    KeepRoom,
}

/// <summary>
/// The room's popups in WinUI's own controls: Invite and Details as flyouts from the column's
/// header (Details turning into Edit relays in place), and Leave, Close and Move to a new room as
/// content dialogs. Leave always asks.
/// </summary>
static class RoomPopups
{
    static readonly FontFamily Mono = new("Cascadia Mono, Consolas");
    static Brush Haze => (Brush)Application.Current.Resources["HazeBrush"];

    static TextBlock Note(string text) => new()
    {
        Text = text,
        FontSize = 12,
        LineHeight = 16,
        Foreground = Haze,
        TextWrapping = TextWrapping.WrapWholeWords,
    };

    static TextBlock Heading(string text) => new() { Text = text, FontSize = 14, FontWeight = FontWeights.SemiBold, Margin = new Thickness(0, 0, 0, 4) };

    static Flyout Flyout(UIElement body) => new()
    {
        Content = new StackPanel { Width = 300, Spacing = 0, Children = { body } },
        Placement = FlyoutPlacementMode.BottomEdgeAlignedRight,
    };

    /// <summary>
    /// Invite: the link with Copy link, Share… and New invite, and under a rule, Move to a new
    /// room for someone already here.
    /// </summary>
    public static Flyout Invite(Ctx ctx, string link, Action newInvite, Action move, bool moveHighlight = false)
    {
        var body = new StackPanel { Spacing = 0 };
        body.Children.Add(Heading(RoomWords.Invite));
        body.Children.Add(Note(RoomWords.InviteAnyone));
        body.Children.Add(Note(RoomWords.InviteKey));
        var row = new Grid { ColumnSpacing = 8, Margin = new Thickness(0, 10, 0, 0) };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var box = new TextBox { Text = link, IsReadOnly = true, FontFamily = Mono, FontSize = 12 };
        AutomationProperties.SetName(box, RoomWords.Invite);
        row.Children.Add(box);
        var copyText = new TextBlock { Text = RoomWords.CopyLink };
        var copy = new Button
        {
            Style = (Style)Application.Current.Resources["AccentButtonStyle"],
            Content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, Children = { new FontIcon { Glyph = "", FontSize = 14 }, copyText } },
        };
        AutomationProperties.SetName(copy, RoomWords.CopyLink);
        var copied = 0;
        copy.Click += async (_, _) =>
        {
            if (!Ui.TryCopy(link))
                return;
            var mine = ++copied;
            copyText.Text = T.ActionCopied;
            await Task.Delay(2000);
            if (mine == copied)
                copyText.Text = RoomWords.CopyLink;
        };
        Grid.SetColumn(copy, 1);
        row.Children.Add(copy);
        body.Children.Add(row);
        var more = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 6, 0, 0) };
        var share = new Button { Content = RoomWords.Share };
        share.Click += (_, _) => InviteDialog.Share(ctx, link);
        var fresh = new Button { Content = RoomWords.NewInvite };
        fresh.Click += (_, _) => newInvite();
        more.Children.Add(share);
        more.Children.Add(fresh);
        body.Children.Add(more);
        var freshNote = Note(RoomWords.NewInviteBody);
        freshNote.Margin = new Thickness(0, 8, 0, 0);
        body.Children.Add(freshNote);
        body.Children.Add(new Border { Height = 1, Background = (Brush)Application.Current.Resources["FaintRuleBrush"], Margin = new Thickness(0, 10, 0, 10) });
        var moveButton = new Button { Content = RoomWords.MoveRoom };
        if (moveHighlight)
        {
            moveButton.BorderBrush = (Brush)Application.Current.Resources["AmberBrush"];
            moveButton.BorderThickness = new Thickness(2);
        }
        body.Children.Add(moveButton);
        var moveNote = Note(RoomWords.MoveRoomBody);
        moveNote.Margin = new Thickness(0, 6, 0, 0);
        body.Children.Add(moveNote);
        var flyout = Flyout(body);
        moveButton.Click += (_, _) =>
        {
            flyout.Hide();
            move();
        };
        return flyout;
    }

    /// <summary>Details: encryption and the room's relays, each with how it answers, then Edit relays in place.</summary>
    public static Flyout Details(RelayHealth[] relays, Action<string[]> saveRelays, bool startEditing = false)
    {
        var host = new ContentControl { HorizontalContentAlignment = HorizontalAlignment.Stretch };
        var flyout = Flyout(host);
        void Read()
        {
            var body = new StackPanel();
            body.Children.Add(Heading(RoomWords.Details));
            var up = relays.Count(r => r.Healthy);
            foreach (var (label, value) in new[] { (RoomWords.Encryption, RoomWords.EndToEnd), (RoomWords.Relays, RoomWords.RelaysAnswering(up, relays.Length)) })
            {
                var pair = new Grid { Height = 26 };
                pair.Children.Add(new TextBlock { Text = label, Foreground = Haze, FontSize = 13, VerticalAlignment = VerticalAlignment.Center });
                pair.Children.Add(new TextBlock { Text = value, FontSize = 13, HorizontalAlignment = HorizontalAlignment.Right, VerticalAlignment = VerticalAlignment.Center });
                body.Children.Add(pair);
            }
            var list = new StackPanel { Margin = new Thickness(0, 6, 0, 0) };
            foreach (var r in relays)
                list.Children.Add(RelayRow(r.Url, r.Healthy, r.Healthy ? (r.RttMs is { } ms ? RoomWords.Rtt(ms) : "") : RoomWords.NotAnswering, null));
            body.Children.Add(Boxed(list));
            var note = Note(RoomWords.RelaysBody);
            note.Margin = new Thickness(0, 8, 0, 0);
            body.Children.Add(note);
            var edit = new Button { Content = RoomWords.EditRelays, Margin = new Thickness(0, 10, 0, 0) };
            edit.Click += (_, _) => Edit();
            body.Children.Add(edit);
            host.Content = body;
        }
        void Edit()
        {
            var list = relays.Select(r => r.Url).ToList();
            var body = new StackPanel();
            body.Children.Add(Heading(RoomWords.EditRelaysTitle));
            body.Children.Add(Note(RoomWords.EditRelaysBody));
            var rows = new StackPanel { Margin = new Thickness(0, 8, 0, 0) };
            void Fill()
            {
                rows.Children.Clear();
                foreach (var url in list.ToList())
                    rows.Children.Add(RelayRow(url, true, null, () =>
                    {
                        list.Remove(url);
                        Fill();
                    }));
            }
            Fill();
            body.Children.Add(Boxed(rows));
            var add = new Grid { ColumnSpacing = 8, Margin = new Thickness(0, 8, 0, 0) };
            add.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            add.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var box = new TextBox { PlaceholderText = RoomWords.RelayScheme, FontFamily = Mono, FontSize = 12 };
            AutomationProperties.SetName(box, RoomWords.Relays);
            add.Children.Add(box);
            var addButton = new Button { Content = RoomWords.Add };
            addButton.Click += (_, _) =>
            {
                var url = box.Text.Trim();
                if (url.Length == 0 || list.Contains(url))
                    return;
                list.Add(url);
                box.Text = "";
                Fill();
            };
            Grid.SetColumn(addButton, 1);
            add.Children.Add(addButton);
            body.Children.Add(add);
            var note = Note(RoomWords.RelaysInSettings);
            note.Margin = new Thickness(0, 8, 0, 0);
            body.Children.Add(note);
            var done = new Button { Content = RoomWords.Done, Style = (Style)Application.Current.Resources["AccentButtonStyle"], HorizontalAlignment = HorizontalAlignment.Right, Margin = new Thickness(0, 8, 0, 0) };
            done.Click += (_, _) =>
            {
                saveRelays([.. list]);
                flyout.Hide();
            };
            body.Children.Add(done);
            host.Content = body;
        }
        if (startEditing)
            Edit();
        else
            Read();
        flyout.Closed += (_, _) => Read();
        return flyout;
    }


    /// <summary>
    /// Settings › Playback › Watch together › Relays: the list new rooms start on and invites
    /// carry, with Add and Remove, the spares under it, and Restore the defaults. Null on Cancel;
    /// an empty list means the defaults.
    /// </summary>
    public static async Task<string[]?> EditDefaultRelays(XamlRoot root, RoomRelaySettings settings)
    {
        var defaults = settings.Defaults ?? [];
        var list = new List<string>((settings.Chosen ?? []).Length > 0 ? settings.Chosen! : defaults);
        var body = new StackPanel { Spacing = 0, Width = 440 };
        body.Children.Add(Note(T.RoomRelaysIntro));
        var heading = Heading(T.RoomRelaysNewRooms);
        heading.Margin = new Thickness(0, 14, 0, 2);
        body.Children.Add(heading);
        body.Children.Add(Note(T.RoomRelaysNewRoomsLine));
        var rows = new StackPanel { Margin = new Thickness(0, 6, 0, 0) };
        body.Children.Add(Boxed(rows));
        var add = new Grid { ColumnSpacing = 8, Margin = new Thickness(0, 8, 0, 0) };
        add.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        add.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var field = new TextBox { PlaceholderText = RoomWords.RelayScheme, FontFamily = Mono, FontSize = 12 };
        AutomationProperties.SetName(field, T.RoomRelaysNewRooms);
        add.Children.Add(field);
        var addButton = new Button { Content = RoomWords.Add };
        Grid.SetColumn(addButton, 1);
        add.Children.Add(addButton);
        body.Children.Add(add);
        var restore = new HyperlinkButton { Content = T.RoomRelaysRestore, Padding = new Thickness(0), Margin = new Thickness(0, 8, 0, 0) };
        body.Children.Add(restore);
        var spares = Heading(T.RoomRelaysSpares);
        spares.Margin = new Thickness(0, 14, 0, 2);
        body.Children.Add(spares);
        body.Children.Add(Note(T.RoomRelaysSparesLine));
        var spareRows = new StackPanel { Margin = new Thickness(0, 6, 0, 0) };
        foreach (var spare in settings.Spares ?? [])
            spareRows.Children.Add(RelayRow(spare, true, "", null));
        body.Children.Add(Boxed(spareRows));
        void Fill()
        {
            rows.Children.Clear();
            foreach (var url in list.ToList())
                rows.Children.Add(RelayRow(url, true, null, () =>
                {
                    list.Remove(url);
                    Fill();
                }));
            restore.Visibility = list.SequenceEqual(defaults) ? Visibility.Collapsed : Visibility.Visible;
        }
        addButton.Click += (_, _) =>
        {
            var url = field.Text.Trim();
            if (url.Length == 0 || list.Contains(url))
                return;
            list.Add(url);
            field.Text = "";
            Fill();
        };
        restore.Click += (_, _) =>
        {
            list = [.. defaults];
            Fill();
        };
        Fill();
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.RoomRelays,
            Content = new ScrollViewer { Content = body, MaxHeight = 520 },
            PrimaryButtonText = RoomWords.Done,
            CloseButtonText = RoomWords.Cancel,
            DefaultButton = ContentDialogButton.Primary,
        };
        try
        {
            if (await Dialogs.Show(dialog) != ContentDialogResult.Primary)
                return null;
        }
        catch (COMException)
        {
            return null;
        }
        return list.SequenceEqual(defaults) ? [] : [.. list];
    }
    static Border Boxed(UIElement child) => new()
    {
        Child = child,
        CornerRadius = new CornerRadius(4),
        Background = new SolidColorBrush(Color.FromArgb(0x0D, 0xE4, 0xE0, 0xD6)),
        BorderBrush = (Brush)Application.Current.Resources["HairBrush"],
        BorderThickness = new Thickness(1),
        Padding = new Thickness(10, 2, 4, 2),
    };

    /// <summary>A relay: a dot (water when it answers, haze when not), the address, and how it answers or Remove.</summary>
    static Grid RelayRow(string url, bool healthy, string? said, Action? remove)
    {
        var row = new Grid { MinHeight = 28, ColumnSpacing = 8 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.Children.Add(new Microsoft.UI.Xaml.Shapes.Ellipse
        {
            Width = 7,
            Height = 7,
            Fill = (Brush)Application.Current.Resources[healthy ? "WaterBrush" : "HazeBrush"],
            VerticalAlignment = VerticalAlignment.Center,
        });
        var address = new TextBlock { Text = url, FontFamily = Mono, FontSize = 11, VerticalAlignment = VerticalAlignment.Center, TextTrimming = TextTrimming.CharacterEllipsis };
        Grid.SetColumn(address, 1);
        row.Children.Add(address);
        FrameworkElement end;
        if (remove is not null)
        {
            var x = new Button { Width = 28, Height = 28, Padding = new Thickness(0), Content = new FontIcon { Glyph = "", FontSize = 11 }, Style = (Style)Application.Current.Resources["SubtleButtonStyle"] };
            AutomationProperties.SetName(x, RoomWords.Remove(url));
            ToolTipService.SetToolTip(x, RoomWords.Remove(url));
            x.Click += (_, _) => remove();
            end = x;
        }
        else
            end = new TextBlock { Text = said ?? "", FontSize = 12, Foreground = Haze, VerticalAlignment = VerticalAlignment.Center };
        Grid.SetColumn(end, 2);
        row.Children.Add(end);
        AutomationProperties.SetName(row, said is { Length: > 0 } ? $"{url}, {said}" : url);
        return row;
    }

    /// <summary>Leave: always asked. The others keep watching.</summary>
    public static async Task<bool> AskLeave(XamlRoot root)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = RoomWords.LeaveTitle,
            Content = new TextBlock { Text = RoomWords.LeaveBody, TextWrapping = TextWrapping.WrapWholeWords },
            PrimaryButtonText = RoomWords.Leave,
            CloseButtonText = RoomWords.Cancel,
            DefaultButton = ContentDialogButton.Primary,
            RequestedTheme = ElementTheme.Dark,
        };
        try
        {
            return await Dialogs.Show(dialog) == ContentDialogResult.Primary;
        }
        catch (COMException)
        {
            return false;
        }
    }

    /// <summary>Closing the player in a room: Leave, Keep the room (the player closes, you stay), or Cancel.</summary>
    public static async Task<RoomCloseAnswer> AskClose(XamlRoot root)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = RoomWords.LeaveTitle,
            Content = new TextBlock { Text = RoomWords.LeaveCloseBody, TextWrapping = TextWrapping.WrapWholeWords },
            PrimaryButtonText = RoomWords.Leave,
            SecondaryButtonText = RoomWords.KeepRoom,
            CloseButtonText = RoomWords.Cancel,
            DefaultButton = ContentDialogButton.Primary,
            RequestedTheme = ElementTheme.Dark,
        };
        try
        {
            return await Dialogs.Show(dialog) switch
            {
                ContentDialogResult.Primary => RoomCloseAnswer.Leave,
                ContentDialogResult.Secondary => RoomCloseAnswer.KeepRoom,
                _ => RoomCloseAnswer.Cancel,
            };
        }
        catch (COMException)
        {
            return RoomCloseAnswer.Cancel;
        }
    }

    /// <summary>
    /// Move to a new room: everyone present, ticked unless muted; you always go. The button
    /// counts the others ticked. Answers the ids to take, or null when cancelled.
    /// </summary>
    public static async Task<string[]?> AskMove(XamlRoot root, string me, IReadOnlyList<MoveCandidate> candidates, string at, Action<ContentDialog>? shown = null)
    {
        var ticked = candidates.Where(c => c.Ticked).Select(c => c.Id).ToHashSet();
        var body = new StackPanel { Spacing = 0, Width = 360 };
        body.Children.Add(new TextBlock { Text = RoomWords.MoveBody, TextWrapping = TextWrapping.WrapWholeWords, Foreground = Haze });
        var list = new StackPanel { Margin = new Thickness(0, 12, 0, 0) };
        ContentDialog? dialog = null;
        void Count()
        {
            if (dialog is null)
                return;
            dialog.PrimaryButtonText = Rooms.MoveButton(ticked.Count);
            dialog.IsPrimaryButtonEnabled = ticked.Count > 0;
        }
        list.Children.Add(MoveRow(me, RoomWords.YouAlone, RoomWords.AlwaysGoes, true, locked: true, null));
        foreach (var c in candidates)
        {
            var id = c.Id;
            list.Children.Add(MoveRow(c.Name, c.Name, c.Muted ? RoomWords.MutedNotTicked : null, c.Ticked, locked: false, on =>
            {
                if (on)
                    ticked.Add(id);
                else
                    ticked.Remove(id);
                Count();
            }));
        }
        body.Children.Add(Boxed(list));
        var foot = new TextBlock { Text = RoomWords.MoveFoot(at), FontSize = 12, LineHeight = 16, Foreground = Haze, TextWrapping = TextWrapping.WrapWholeWords, Margin = new Thickness(0, 10, 0, 0) };
        body.Children.Add(foot);
        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = RoomWords.MoveTitle,
            Content = body,
            CloseButtonText = RoomWords.Cancel,
            DefaultButton = ContentDialogButton.Primary,
            RequestedTheme = ElementTheme.Dark,
        };
        Count();
        try
        {
            var answer = await Dialogs.Show(dialog, shown is null ? null : () => shown(dialog));
            return answer == ContentDialogResult.Primary ? [.. ticked] : null;
        }
        catch (COMException)
        {
            return null;
        }
    }

    static Grid MoveRow(string name, string shownName, string? note, bool on, bool locked, Action<bool>? changed)
    {
        var row = new Grid { MinHeight = 42, ColumnSpacing = 10 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var face = new RoomFace(24) { VerticalAlignment = VerticalAlignment.Center };
        face.Show(name, new RoomFaceLook(RingKind.Water, 1, 1, false, false));
        row.Children.Add(face);
        var words = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        words.Children.Add(new TextBlock { Text = shownName, FontSize = 13 });
        if (note is not null)
            words.Children.Add(new TextBlock { Text = note, FontSize = 11, LineHeight = 14, Foreground = Haze });
        Grid.SetColumn(words, 1);
        row.Children.Add(words);
        var box = new CheckBox { IsChecked = on, IsEnabled = !locked, MinWidth = 0, Padding = new Thickness(0), VerticalAlignment = VerticalAlignment.Center };
        AutomationProperties.SetName(box, note is null ? shownName : $"{shownName}, {note}");
        box.Checked += (_, _) => changed?.Invoke(true);
        box.Unchecked += (_, _) => changed?.Invoke(false);
        Grid.SetColumn(box, 2);
        row.Children.Add(box);
        return row;
    }
}
