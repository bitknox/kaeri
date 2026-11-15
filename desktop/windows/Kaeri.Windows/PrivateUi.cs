using Kaeri.App;
using Kaeri.Windows.Pages;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows;

/// <summary>
/// The pieces private watching and reading draws in more than one place: the eye with the slash,
/// the line under a title or series page's main action, and the switch in silk. Private is a
/// state of the app, never an accent, so nothing here takes amber.
/// </summary>
static class PrivateUi
{
    /// <summary>Segoe Fluent Icons' eye with a slash.</summary>
    public const string Glyph = "";

    /// <summary>The eye in the text colour at <paramref name="size"/>.</summary>
    public static FontIcon Eye(double size = 16) => new()
    {
        Glyph = Glyph,
        FontSize = size,
        Foreground = Ui.Res<Brush>("SilkBrush"),
        VerticalAlignment = VerticalAlignment.Center,
    };

    /// <summary>
    /// A switch whose on track is silk, not the app's amber: the settings card's and the show
    /// settings' Always private.
    /// </summary>
    public static T Silk<T>(T control) where T : FrameworkElement
    {
        // High contrast keeps the system's own switch colours.
        if (new global::Windows.UI.ViewManagement.AccessibilitySettings().HighContrast)
            return control;
        var silk = Ui.Res<Brush>("SilkBrush");
        control.Resources["ToggleSwitchFillOn"] = silk;
        control.Resources["ToggleSwitchFillOnPointerOver"] = silk;
        control.Resources["ToggleSwitchFillOnPressed"] = Ui.Res<Brush>("HazeBrush");
        return control;
    }

    /// <summary>
    /// The line's place under a main action: the eye, then the head in silk and semibold, " · " and
    /// the rest in haze, 14 px. Collapsed until <see cref="Draw"/> gives it words.
    /// </summary>
    public static StackPanel Line()
    {
        var line = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Margin = new Thickness(0, 10, 0, 0),
            Visibility = Visibility.Collapsed,
            HorizontalAlignment = HorizontalAlignment.Left,
        };
        line.Children.Add(Eye());
        line.Children.Add(new TextBlock
        {
            FontSize = 14,
            Foreground = Ui.Res<Brush>("HazeBrush"),
            TextWrapping = TextWrapping.WrapWholeWords,
            VerticalAlignment = VerticalAlignment.Center,
        });
        return line;
    }

    /// <summary>Puts <paramref name="said"/> in a line from <see cref="Line"/>, fading it in or out on the standard token when it comes or goes.</summary>
    public static void Draw(StackPanel line, PrivateLine? said)
    {
        if (said is null)
        {
            if (line.Visibility == Visibility.Visible)
                Motion.HideStandard(line);
            return;
        }
        var text = (TextBlock)line.Children[1];
        text.Inlines.Clear();
        text.Inlines.Add(new Run { Text = said.Head, FontWeight = FontWeights.SemiBold, Foreground = Ui.Res<Brush>("SilkBrush") });
        text.Inlines.Add(new Run { Text = " · " + said.Tail });
        AutomationProperties.SetName(line, said.Spoken);
        if (line.Visibility != Visibility.Visible || line.Opacity < 1)
            Motion.ShowStandard(line);
    }
}
