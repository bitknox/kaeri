using Kaeri.App;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// The pieces both Searches draw, the shows' and reading's, so the two look and move alike: the
/// filter row's drop-downs, which say their name until set and then what they hold in amber; the
/// chips that take a filter off; the Filters toggle with its badge; and See all at the end of a
/// rail's heading.
/// </summary>
static class SearchParts
{
    /// <summary>The cover cell of Search's grid and rails: covers 150 by 225, 16 apart, 26 between lines.</summary>
    public const double CoverWidth = 150, CoverHeight = 225, CoverGap = 16, LineGap = 26;

    /// <summary>A filter's drop-down as the row draws it: 32 high, its name until something is set.</summary>
    public static DropDownButton Menu(string name, FlyoutBase flyout, double minWidth = 96)
    {
        var button = new DropDownButton { Content = name, MinHeight = 32, MinWidth = minWidth, Flyout = flyout };
        AutomationProperties.SetName(button, name);
        return button;
    }

    /// <summary>A drop-down's list of boxes, as Genre and Format hold them; a long list scrolls.</summary>
    public static (Flyout Flyout, StackPanel List) Boxes(double minWidth, bool scrolls = false)
    {
        var list = new StackPanel { Spacing = 2, MinWidth = minWidth };
        var flyout = new Flyout
        {
            Placement = FlyoutPlacementMode.BottomEdgeAlignedLeft,
            Content = scrolls ? new ScrollViewer { MaxHeight = 360, VerticalScrollBarVisibility = ScrollBarVisibility.Auto, Content = list } : list,
        };
        return (flyout, list);
    }

    /// <summary>One choice in a list of boxes: <paramref name="click"/> hears whether it is now ticked.</summary>
    public static CheckBox Box(string text, bool on, Action<bool> click)
    {
        var box = new CheckBox { Content = text, IsChecked = on };
        box.Click += (_, _) => click(box.IsChecked == true);
        return box;
    }

    /// <summary>One choice of a drop-down that holds one at a time, as Season, Year and Sort do.</summary>
    public static RadioMenuFlyoutItem Radio(string text, string group, bool on, Action click)
    {
        var item = new RadioMenuFlyoutItem { Text = text, GroupName = group, IsChecked = on };
        item.Click += (_, _) => click();
        return item;
    }

    /// <summary>A filter's words, set only when they change, and its spoken name: the filter, then what it has. Set, in amber.</summary>
    public static void Say(DropDownButton button, string name, string text, bool set)
    {
        if (button.Content as string != text)
            button.Content = text;
        AutomationProperties.SetName(button, set ? $"{name}, {text}" : name);
        if (set)
            button.Foreground = (Brush)Application.Current.Resources["AccentTextFillColorTertiaryBrush"];
        else
            button.ClearValue(Control.ForegroundProperty);
    }

    /// <summary>A filter that is set, as a chip under the row that takes it off.</summary>
    public static Button Chip(string label, Action remove)
    {
        var chip = new Button
        {
            Content = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 6,
                Children =
                {
                    new TextBlock { Text = label, FontSize = 12, MaxLines = 1, TextTrimming = TextTrimming.CharacterEllipsis, MaxWidth = 280 },
                    new FontIcon { Glyph = "", FontSize = 9 },
                },
            },
            CornerRadius = new CornerRadius(12),
            Padding = new Thickness(10, 3, 10, 4),
            MinHeight = 26,
            Background = Ui.Res<Brush>("RaisedBrush"),
            BorderBrush = Ui.Res<Brush>("EdgeBrush"),
            BorderThickness = new Thickness(1),
        };
        AutomationProperties.SetName(chip, T.FilterRemove(label));
        chip.Click += (_, _) => remove();
        return chip;
    }

    /// <summary>Clear filters at the row's end, shown while anything is set.</summary>
    public static HyperlinkButton Clear(Action clear)
    {
        var link = new HyperlinkButton { Content = T.SearchClearFilters, Style = Ui.Res<Style>("SilkLinkStyle"), VerticalAlignment = VerticalAlignment.Center, Visibility = Visibility.Collapsed };
        link.Click += (_, _) => clear();
        return link;
    }

    /// <summary>Filters: the panel's toggle, with how many of the panel's own filters are set.</summary>
    public static (ToggleButton Button, InfoBadge Badge) FiltersToggle()
    {
        var badge = new InfoBadge { Visibility = Visibility.Collapsed, VerticalAlignment = VerticalAlignment.Center };
        var button = new ToggleButton
        {
            MinHeight = 32,
            Content = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 8,
                Children = { new FontIcon { Glyph = "", FontSize = 14 }, new TextBlock { Text = T.FiltersTitle }, badge },
            },
        };
        AutomationProperties.SetName(button, T.FiltersTitle);
        ToolTipService.SetToolTip(button, T.TooltipWithKey(T.FiltersTitle, "Ctrl+Shift+F"));
        return (button, badge);
    }

    /// <summary>The badge on Filters: how many of the panel's own filters are set, and the button's name with it.</summary>
    public static void Badge(ToggleButton button, InfoBadge badge, uint count)
    {
        badge.Value = (int)count;
        badge.Visibility = count > 0 ? Visibility.Visible : Visibility.Collapsed;
        AutomationProperties.SetName(button, count > 0 ? T.SearchFiltersButtonSet(count) : T.FiltersTitle);
    }

    /// <summary>See all with its chevron, at the end of a rail's heading.</summary>
    public static HyperlinkButton SeeAll(Action act)
    {
        var all = new HyperlinkButton
        {
            Style = Ui.Res<Style>("SilkLinkStyle"),
            VerticalAlignment = VerticalAlignment.Center,
            Content = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 4,
                Children = { new TextBlock { Text = T.SearchSeeAll }, new FontIcon { Glyph = "", FontSize = 10 } },
            },
        };
        AutomationProperties.SetName(all, T.SearchSeeAll);
        all.Click += (_, _) => act();
        return all;
    }

    /// <summary>A rail's heading: its words in the section style at the start, See all at the end.</summary>
    public static Grid RailHead(UIElement heading, HyperlinkButton? seeAll)
    {
        var head = new Grid { ColumnSpacing = 8 };
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        head.Children.Add(heading);
        if (seeAll is not null)
        {
            Grid.SetColumn(seeAll, 1);
            head.Children.Add(seeAll);
        }
        return head;
    }
}
