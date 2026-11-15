using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;

namespace Kaeri.Windows.Pages;

// The Group row on the series page and its picker.
public sealed partial class SeriesPage
{
    /// <summary>The parts of the page a group pick redraws, which crossfade when the series comes back.</summary>
    readonly HashSet<string> groupPicked = [];
    /// <summary>A pick redraws the row: the keys go back to its Change, which has new words.</summary>
    bool refocusGroup;
#if KAERI_TESTING
    /// <summary>Shots: the next Group row drawn opens its picker.</summary>
    internal static bool ShotGroupPicker { get; set; }
#endif

    /// <summary>"Group · Lanternlight Scans · most chapters", with Change opening the picker.</summary>
    void AddGroup(BookEditionRow edition)
    {
        side.Children.Add(SeriesWords.Rule());
        var row = new Grid { ColumnSpacing = 12, Padding = new Thickness(0, 10, 0, 10) };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var value = GroupPicker.Value(edition);
        row.Children.Add(SeriesWords.Pair(T.SeriesGroup, value));
        var change = new HyperlinkButton { Content = T.SeriesChange, Style = Ui.Res<Style>("SilkLinkStyle"), VerticalAlignment = VerticalAlignment.Center };
        // "Group, Lanternlight Scans, most chapters. Change, button": the row's words lead.
        AutomationProperties.SetName(change, $"{T.SeriesGroup}, {value.Replace(" · ", ", ")}. {T.SeriesChange}");
        AutomationProperties.SetHelpText(change, T.SeriesGroupHint);
        var flyout = new Flyout { Placement = FlyoutPlacementMode.BottomEdgeAlignedRight };
        flyout.Opening += (_, _) => flyout.Content = GroupFlyout(edition, flyout);
        var style = new Style(typeof(FlyoutPresenter));
        style.Setters.Add(new Setter(Control.PaddingProperty, new Thickness(4)));
        style.Setters.Add(new Setter(FrameworkElement.MaxWidthProperty, 320.0));
        style.Setters.Add(new Setter(Control.CornerRadiusProperty, new CornerRadius(8)));
        flyout.FlyoutPresenterStyle = style;
        change.Click += (_, _) => flyout.ShowAt(change);
#if KAERI_TESTING
        if (ShotGroupPicker)
        {
            ShotGroupPicker = false;
            change.Loaded += async (_, _) =>
            {
                await System.Threading.Tasks.Task.Delay(600);
                flyout.ShowAt(change);
            };
        }
#endif
        if (refocusGroup)
        {
            refocusGroup = false;
            change.Loaded += (_, _) => change.Focus(FocusState.Programmatic);
        }
        Grid.SetColumn(change, 1);
        row.Children.Add(change);
        side.Children.Add(row);
    }

    /// <summary>
    /// The picker, 320 wide: "Group in books.home.example", Most chapters naming the group it
    /// reads from now, a rule, each group with its count and language, a rule and the fallback
    /// note. Rows are radio choices: the arrows move, Return or Space picks, Esc closes.
    /// </summary>
    FrameworkElement GroupFlyout(BookEditionRow edition, Flyout flyout)
    {
        var column = new StackPanel { Width = 312 };
        var head = Ui.Text(T.WindowsGroupPickerIn(edition.SourceName), "CaptionTextBlockStyle");
        head.Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush");
        head.Margin = new Thickness(12, 8, 12, 4);
        column.Children.Add(head);
        var picks = GroupPicker.Rows(edition);
        var buttons = new List<ChoiceButton>();
        var list = new StackPanel();
        AutomationProperties.SetName(list, T.SeriesGroup);
        for (var i = 0; i < picks.Count; i++)
        {
            var button = Choice(picks[i]);
            buttons.Add(button);
            list.Children.Add(button);
            if (i == 0 && picks.Count > 1)
                list.Children.Add(Rule());
        }
        column.Children.Add(list);
        column.Children.Add(Rule());
        var foot = Ui.Text(T.GroupPickerFallback, "CaptionTextBlockStyle");
        foot.Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush");
        foot.Margin = new Thickness(12, 8, 12, 8);
        column.Children.Add(foot);
        ChoiceGroup.Join(buttons, at =>
        {
            flyout.Hide();
            var pick = picks[at];
            if (pick.InUse)
                return;
            groupPicked.Add("side");
            groupPicked.Add("chapters");
            refocusGroup = true;
            ctx.Known.Reading.ChooseGroup(work, pick.Group);
        }, pickOnArrow: false);
        // The keys start on the choice in use.
        column.Loaded += (_, _) =>
        {
            var at = picks.FindIndex(p => p.InUse);
            buttons[Math.Max(0, at)].Focus(FocusState.Keyboard);
        };
        return column;
    }

    static Rectangle Rule() => new()
    {
        Height = 1,
        Margin = new Thickness(0, 4, 0, 4),
        Fill = Ui.Res<Brush>("DividerStrokeColorDefaultBrush"),
    };

    /// <summary>A row: the 6 px dot for the one in use, its name, and its line under it.</summary>
    static ChoiceButton Choice(GroupPick pick)
    {
        var dot = new Ellipse
        {
            Width = 6,
            Height = 6,
            Fill = Ui.Res<Brush>("AccentFillColorDefaultBrush"),
            Opacity = pick.InUse ? 1 : 0,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };
        var words = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        words.Children.Add(new TextBlock { Text = pick.Name, FontSize = 14, LineHeight = 20, TextWrapping = TextWrapping.WrapWholeWords });
        words.Children.Add(new TextBlock
        {
            Text = pick.Line,
            FontSize = 12,
            LineHeight = 16,
            TextWrapping = TextWrapping.WrapWholeWords,
            Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush"),
        });
        var content = new Grid();
        content.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(16) });
        content.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        content.Children.Add(dot);
        Grid.SetColumn(words, 1);
        words.Margin = new Thickness(8, 0, 0, 0);
        content.Children.Add(words);
        var button = new ChoiceButton
        {
            Content = content,
            IsChecked = pick.InUse,
            MinHeight = 48,
            Padding = new Thickness(12, 4, 12, 4),
            CornerRadius = new CornerRadius(4),
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            BorderThickness = new Thickness(0),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
        };
        // Checked looks like the rest: the dot says which is in use.
        var resources = button.Resources;
        resources["ToggleButtonBackgroundChecked"] = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        resources["ToggleButtonBackgroundCheckedPointerOver"] = Ui.Res<Brush>("SubtleFillColorSecondaryBrush");
        resources["ToggleButtonBackgroundCheckedPressed"] = Ui.Res<Brush>("SubtleFillColorTertiaryBrush");
        resources["ToggleButtonBackground"] = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        resources["ToggleButtonBackgroundPointerOver"] = Ui.Res<Brush>("SubtleFillColorSecondaryBrush");
        resources["ToggleButtonBackgroundPressed"] = Ui.Res<Brush>("SubtleFillColorTertiaryBrush");
        resources["ToggleButtonForegroundChecked"] = Ui.Res<Brush>("TextFillColorPrimaryBrush");
        resources["ToggleButtonForegroundCheckedPointerOver"] = Ui.Res<Brush>("TextFillColorPrimaryBrush");
        resources["ToggleButtonForegroundCheckedPressed"] = Ui.Res<Brush>("TextFillColorPrimaryBrush");
        resources["ToggleButtonBorderBrushChecked"] = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        AutomationProperties.SetName(button, pick.Name);
        AutomationProperties.SetFullDescription(button, pick.Line);
        return button;
    }
}
