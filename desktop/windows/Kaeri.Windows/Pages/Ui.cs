using Kaeri.App;
using Kaeri.Engine.Ffi;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;

namespace Kaeri.Windows.Pages;

/// <summary>Small pieces the pages built in code share: styled text, icon buttons, dots and rows.</summary>
static class Ui
{
    public static T Res<T>(string key) => (T)Application.Current.Resources[key];

    /// <summary>
    /// The row an <see cref="ListViewBase.ItemClick"/> was for. When a list holds its rows as
    /// ListViewItems, WinUI names the row's content as the clicked item rather than the row, so a
    /// handler that looked for the row's tag found nothing and every click did nothing.
    /// </summary>
    public static ListViewItem? ClickedRow(ListViewBase list, object? clicked) =>
        clicked as ListViewItem ?? list.Items.OfType<ListViewItem>().FirstOrDefault(i => ReferenceEquals(i.Content, clicked));

    /// <summary>The first <typeparamref name="T"/> in <paramref name="at"/>'s visual tree, such as a NumberBox's own text field.</summary>
    public static T? Descendant<T>(DependencyObject at) where T : DependencyObject
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(at); i++)
        {
            var child = VisualTreeHelper.GetChild(at, i);
            if (child is T found)
                return found;
            if (Descendant<T>(child) is { } deeper)
                return deeper;
        }
        return null;
    }

    public static TextBlock Text(string text, string? style = null, bool wrap = true) => new()
    {
        Text = text,
        Style = style is null ? null : Res<Style>(style),
        TextWrapping = wrap ? TextWrapping.WrapWholeWords : TextWrapping.NoWrap,
        TextTrimming = wrap ? TextTrimming.None : TextTrimming.CharacterEllipsis,
    };

    public static TextBlock Haze(string text, bool wrap = true) => Text(text, "HazeBodyStyle", wrap);

    public static TextBlock Heading(string text) => new()
    {
        Text = text,
        Style = Res<Style>("BodyStrongTextBlockStyle"),
        Margin = new Thickness(0, 16, 0, 4),
    };

    /// <summary>A square icon button with a tooltip that is also its name.</summary>
    public static Button Icon(string glyph, string name, RoutedEventHandler click)
    {
        var button = new Button { Content = new FontIcon { Glyph = glyph, FontSize = 14 }, Width = 36, Height = 32, Padding = new Thickness(0) };
        ToolTipService.SetToolTip(button, name);
        AutomationProperties.SetName(button, name);
        button.Click += click;
        return button;
    }

    public static Button Label(string text, RoutedEventHandler click, bool accent = false)
    {
        var button = new Button { Content = text, MinHeight = 32 };
        if (accent)
            button.Style = Res<Style>("AccentButtonStyle");
        button.Click += click;
        return button;
    }

    /// <summary>A status dot in a colour or, with <paramref name="ring"/>, an outline.</summary>
    public static Ellipse Dot(global::Windows.UI.Color colour, bool ring = false) => new()
    {
        Width = 8,
        Height = 8,
        VerticalAlignment = VerticalAlignment.Center,
        Fill = ring ? null : new SolidColorBrush(colour),
        Stroke = ring ? new SolidColorBrush(colour) : null,
        StrokeThickness = ring ? 1.5 : 0,
    };

    public static readonly global::Windows.UI.Color Vermilion = global::Windows.UI.Color.FromArgb(0xFF, 0xE0, 0x6B, 0x54);

    /// <summary>The default button with its words in the critical colour: Delete, Remove, Leave.</summary>
    static Style Destructive
    {
        get
        {
            var style = new Style(typeof(Button)) { BasedOn = Res<Style>("DefaultButtonStyle") };
            style.Setters.Add(new Setter(Control.ForegroundProperty, Res<Brush>("SystemFillColorCriticalBrush")));
            return style;
        }
    }

    /// <summary>A flat block set off from the next by a hairline: hierarchy from spacing, not boxes.</summary>
    public static Border Card(UIElement child, double padding = 12) => new()
    {
        Child = child,
        Padding = new Thickness(0, padding, 0, padding),
        BorderBrush = Res<Brush>("DividerStrokeColorDefaultBrush"),
        BorderThickness = new Thickness(0, 0, 0, 1),
    };

    /// <summary>
    /// A cover thumbnail, in its placeholder colour until the engine's art arrives, which then
    /// fades in over it. Art the app already has shows at once, so a rebuilt row never flashes.
    /// </summary>
    public static Border Cover(Ctx ctx, uint id, double width, double height)
    {
        var image = new Image { Stretch = Stretch.UniformToFill };
        AutomationProperties.SetAccessibilityView(image, AccessibilityView.Raw);
        var frame = new Border
        {
            Width = width,
            Height = height,
            CornerRadius = Art.Corner(width),
            Background = Res<Brush>("RaisedBrush"),
            Child = image,
        };
        if (ctx.Known.Entries.TryGetValue(id, out var entry))
        {
            if (Colours.Parse(entry.CoverColor) is { } colour)
                frame.Background = new SolidColorBrush(colour);
            ArtFade.Watch(image);
            ctx.Art.Bind(entry.CoverUrl, art => image.Source = art);
        }
        return frame;
    }

    public static string Bytes(ulong n) => Wording.Bytes(n);

    /// <summary>The focused element when it is <paramref name="root"/> or inside it, otherwise null.</summary>
    public static DependencyObject? FocusWithin(FrameworkElement root)
    {
        if (root.XamlRoot is not { } xamlRoot || Microsoft.UI.Xaml.Input.FocusManager.GetFocusedElement(xamlRoot) is not DependencyObject focused)
            return null;
        for (var at = focused; at is not null; at = VisualTreeHelper.GetParent(at))
            if (ReferenceEquals(at, root))
                return focused;
        return null;
    }

    /// <summary>Which of <paramref name="children"/> holds the focus, if one does.</summary>
    public static T? HoldingFocus<T>(FrameworkElement root, IEnumerable<T> children) where T : DependencyObject
    {
        if (FocusWithin(root) is not { } focused)
            return null;
        var set = new HashSet<DependencyObject>(children, ReferenceEqualityComparer.Instance);
        for (DependencyObject? at = focused; at is not null; at = VisualTreeHelper.GetParent(at))
            if (set.Contains(at))
                return (T)at;
        return null;
    }

    /// <summary>Where the focus was inside a part that is about to be drawn again, to put it back after.</summary>
    public sealed record FocusMark(Type Type, string Name, int Index, FocusState How);

    /// <summary>
    /// The focused control inside <paramref name="root"/> by its kind, its name and which of the
    /// controls of that kind and name it is, so the same one can be found in a new drawing.
    /// </summary>
    public static FocusMark? MarkFocus(FrameworkElement root)
    {
        if (FocusWithin(root) is not Control focused)
            return null;
        var name = AutomationProperties.GetName(focused);
        if (string.IsNullOrEmpty(name) && focused is ContentControl { Content: string words })
            name = words;
        if (string.IsNullOrEmpty(name))
            return null;
        var index = Controls(root).Where(c => c.GetType() == focused.GetType() && NameOf(c) == name).ToList().IndexOf(focused);
        return new FocusMark(focused.GetType(), name, Math.Max(0, index), focused.FocusState);
    }

    /// <summary>Puts the focus back on the control <paramref name="mark"/> names, if the new drawing has one.</summary>
    public static void RestoreFocus(FrameworkElement root, FocusMark? mark)
    {
        if (mark is null)
            return;
        root.UpdateLayout();
        var same = Controls(root).Where(c => c.GetType() == mark.Type && NameOf(c) == mark.Name).ToList();
        if (same.Count == 0)
            return;
        same[Math.Min(mark.Index, same.Count - 1)].Focus(mark.How == FocusState.Unfocused ? FocusState.Programmatic : mark.How);
    }

    static string NameOf(Control c) => AutomationProperties.GetName(c) is { Length: > 0 } name ? name : c is ContentControl { Content: string words } ? words : "";

    static IEnumerable<Control> Controls(DependencyObject under)
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(under); i++)
        {
            var child = VisualTreeHelper.GetChild(under, i);
            if (child is Control c)
                yield return c;
            foreach (var deeper in Controls(child))
                yield return deeper;
        }
    }

    /// <summary>Whether someone is typing in a box inside <paramref name="root"/>, which a redraw would take from them.</summary>
    public static bool TypingIn(FrameworkElement root) => FocusWithin(root) is TextBox or PasswordBox;

    /// <summary>
    /// Puts <paramref name="text"/> on the clipboard. Windows refuses while another app holds it
    /// open, and the refusal is an exception: then the window says so and this is false.
    /// </summary>
    public static bool TryCopy(string text)
    {
        try
        {
            var package = new global::Windows.ApplicationModel.DataTransfer.DataPackage();
            package.SetText(text);
            global::Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(package);
            return true;
        }
        catch (Exception e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"the clipboard refused a copy: {e.Message}");
            Ctx.Current?.Window.Say(T.ClipboardBusy);
            return false;
        }
    }

    /// <summary>
    /// A destructive action's confirmation: the action names itself in vermilion on a plain
    /// button, never the amber accent, and Enter presses neither, so a key typed on does not
    /// confirm it. Escape is Cancel.
    /// </summary>
    public static async Task<bool> Confirm(XamlRoot root, string title, string text, string action)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = title,
            Content = Text(text, "BodyTextBlockStyle"),
            PrimaryButtonText = action,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.None,
            PrimaryButtonStyle = Destructive,
        };
        return await Dialogs.Show(dialog) == ContentDialogResult.Primary;
    }
}
