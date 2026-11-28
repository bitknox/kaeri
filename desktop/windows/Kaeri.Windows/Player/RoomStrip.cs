using Kaeri.App;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Media;
using Windows.UI;

namespace Kaeri.Windows.Player;

/// <summary>
/// The room in full screen: the faces in a capsule at the top trailing edge, with the room's
/// line and, from 20 s, Carry on. It shows while anyone isn't watching and for 8 s after any
/// change; otherwise it leaves with the controls. Past four people, three faces and a "+N".
/// </summary>
sealed class RoomStrip : Grid
{
    const double Face = 24;
    readonly StackPanel row = new() { Orientation = Orientation.Horizontal, Spacing = 6, VerticalAlignment = VerticalAlignment.Center };
    readonly StackPanel faces = new() { Orientation = Orientation.Horizontal, Spacing = 6 };
    readonly TextBlock words = new() { FontSize = 13, FontWeight = FontWeights.SemiBold, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(4, 0, 6, 0), TextWrapping = TextWrapping.NoWrap };
    readonly Button carry = new() { Style = (Style)Application.Current.Resources["AccentButtonStyle"], Height = 30, Padding = new Thickness(14, 0, 14, 0), Margin = new Thickness(0, 0, -4, 0), Visibility = Visibility.Collapsed };
    readonly Dictionary<string, RoomFace> shown = [];
    string key = "";
    RoomAsk ask;

    public event Action<RoomAsk>? Asked;

    public RoomStrip()
    {
        Height = Face + 16;
        CornerRadius = new CornerRadius((Face + 16) / 2);
        Padding = new Thickness(8, 0, 8, 0);
        Background = new SolidColorBrush(Color.FromArgb(0xE0, 0x16, 0x1A, 0x2E));
        BorderBrush = new SolidColorBrush(Color.FromArgb(0x29, 0xE4, 0xE0, 0xD6));
        BorderThickness = new Thickness(1);
        HorizontalAlignment = HorizontalAlignment.Right;
        VerticalAlignment = VerticalAlignment.Top;
        RequestedTheme = ElementTheme.Dark;
        Typography.SetNumeralAlignment(words, FontNumeralAlignment.Tabular);
        AutomationProperties.SetName(this, RoomWords.Title);
        AutomationProperties.SetLiveSetting(words, Microsoft.UI.Xaml.Automation.Peers.AutomationLiveSetting.Polite);
        row.Children.Add(faces);
        row.Children.Add(words);
        carry.Click += (_, _) => Asked?.Invoke(ask);
        row.Children.Add(carry);
        Children.Add(row);
    }

    public void Show(RoomScene scene)
    {
        var (list, more, _) = Rooms.Capped(Rooms.Rows(scene));
        var now = string.Join(",", list.Select(r => r.Id)) + "+" + more;
        if (now != key)
        {
            key = now;
            faces.Children.Clear();
            shown.Clear();
            foreach (var r in list)
            {
                var face = new RoomFace(Face) { VerticalAlignment = VerticalAlignment.Center };
                shown[r.Id] = face;
                faces.Children.Add(face);
            }
            if (more > 0)
                faces.Children.Add(RoomFace.More(Face, RoomWords.More(more)));
        }
        foreach (var r in list)
        {
            shown[r.Id].Show(r.Name, r.Face);
            AutomationProperties.SetName(shown[r.Id], r.Spoken);
        }
        var text = Rooms.StripWords(scene);
        if (words.Text != text)
            words.Text = text;
        var line = Rooms.Line(scene);
        ask = line.Ask;
        var asking = line.Ask is RoomAsk.CarryOn or RoomAsk.Start;
        if (asking && carry.Content as string != line.AskText)
            carry.Content = line.AskText;
        if (asking != (carry.Visibility == Visibility.Visible))
        {
            carry.Visibility = asking ? Visibility.Visible : Visibility.Collapsed;
            if (asking)
            {
                carry.Opacity = 0;
                Motion.FadeTo(carry, 1, Motion.FadeInTime);
            }
        }
    }
}
