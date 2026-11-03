using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Player;

/// <summary>
/// The player's Episodes panel: a flyout from the Episodes button with compact rows, and under
/// them a footer with the title and synopsis of the row under the pointer or the keyboard focus,
/// else the one playing. The footer is reserved only when the table has a synopsis as the panel
/// opens, so text arriving later moves nothing. A row spoiler protection holds back shows its
/// title and facts.
/// </summary>
sealed class EpisodesPanel
{
    const double Width = 300, ListMax = 360, FooterHeight = 92;

    public Flyout Flyout { get; } = new() { Placement = Microsoft.UI.Xaml.Controls.Primitives.FlyoutPlacementMode.BottomEdgeAlignedRight };

    readonly Func<(MenuEpisode[] Episodes, EpisodeRow[] Table, bool Protect)> source;
    readonly Action<string> choose;

    public EpisodesPanel(Func<(MenuEpisode[] Episodes, EpisodeRow[] Table, bool Protect)> source, Action<string> choose)
    {
        this.source = source;
        this.choose = choose;
        Flyout.FlyoutPresenterStyle = Presenter();
        Flyout.Opening += (_, _) => Build();
    }

    static Style Presenter()
    {
        var style = new Style(typeof(FlyoutPresenter));
        style.Setters.Add(new Setter(Control.PaddingProperty, new Thickness(0)));
        style.Setters.Add(new Setter(FrameworkElement.MinWidthProperty, Width));
        style.Setters.Add(new Setter(FrameworkElement.MaxWidthProperty, Width));
        return style;
    }

    /// <summary>The footer's words for a row: the synopsis, or the facts where protection holds it back or there is none.</summary>
    static (string Heading, string Body) Words(MenuEpisode episode, EpisodeRow? row, bool protect)
    {
        var heading = Lines.Episode(episode.Label, episode.Title, null);
        if (row is null)
            return (heading, "");
        var now = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        var held = Synopsis.Held(row, protect, false);
        return (heading, Synopsis.TextOf(row) is { } text && !held ? text : Synopsis.Facts(row, now, at => Synopsis.Date(at)));
    }

    void Build()
    {
        var (episodes, table, protect) = source();
        var reserved = KaeriFfiMethods.EpisodeText(table) == EpisodeText.Present;
        var playing = episodes.FirstOrDefault(e => e.Current);
        EpisodeRow? RowOf(MenuEpisode e) => table.FirstOrDefault(r => r.Number == e.Number);

        var heading = new TextBlock { Style = (Style)Application.Current.Resources["HazeCaptionStyle"], TextTrimming = TextTrimming.CharacterEllipsis };
        var body = new TextBlock { TextWrapping = TextWrapping.Wrap, MaxLines = 3, TextTrimming = TextTrimming.CharacterEllipsis, FontSize = 12, LineHeight = 17 };
        body.Foreground = (Brush)Application.Current.Resources["HazeBrush"];
        void Show(MenuEpisode? e)
        {
            if (e is null)
                return;
            var (h, b) = Words(e, RowOf(e), protect);
            if (heading.Text == h && body.Text == b)
                return;
            heading.Text = h;
            body.Text = b;
        }
        Show(playing);

        var list = new StackPanel { Padding = new Thickness(4, 6, 4, 6) };
        Button? current = null;
        foreach (var episode in episodes.Take(200))
        {
            var line = new Grid { ColumnSpacing = 10 };
            line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(24) });
            line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var number = new TextBlock { Text = episode.Number, HorizontalAlignment = HorizontalAlignment.Right, VerticalAlignment = VerticalAlignment.Center };
            if (episode.Current)
                number.Foreground = (Brush)Application.Current.Resources["AmberBrush"];
            var title = new TextBlock { Text = episode.Title ?? episode.Label, TextTrimming = TextTrimming.CharacterEllipsis, VerticalAlignment = VerticalAlignment.Center };
            if (episode.Watched)
                title.Foreground = (Brush)Application.Current.Resources["HazeBrush"];
            Grid.SetColumn(title, 1);
            line.Children.Add(number);
            line.Children.Add(title);
            if (episode.Watched)
            {
                var check = new FontIcon { Glyph = "", FontSize = 12, Foreground = (Brush)Application.Current.Resources["HazeBrush"] };
                Grid.SetColumn(check, 2);
                line.Children.Add(check);
            }
            var row = new Button
            {
                Content = line,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Stretch,
                Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
                BorderThickness = new Thickness(0),
                Padding = new Thickness(8, 6, 8, 6),
                CornerRadius = new CornerRadius(6),
            };
            AutomationProperties.SetName(row, Lines.Episode(episode.Label, episode.Title, null));
            var picked = episode;
            row.PointerEntered += (_, _) => Show(picked);
            row.GotFocus += (_, _) => Show(picked);
            row.Click += (_, _) =>
            {
                Flyout.Hide();
                choose(picked.Number);
            };
            if (episode.Current)
                current = row;
            list.Children.Add(row);
        }
        // Left again, the footer goes back to the episode playing.
        list.PointerExited += (_, _) => Show(playing);

        var scroll = new ScrollViewer { MaxHeight = ListMax, Content = list, VerticalScrollBarVisibility = ScrollBarVisibility.Auto };
        var footer = new StackPanel
        {
            Spacing = 3,
            Padding = new Thickness(12, 8, 12, 10),
            Height = FooterHeight,
            BorderThickness = new Thickness(0, 1, 0, 0),
            BorderBrush = (Brush)Application.Current.Resources["FaintRuleBrush"],
            Visibility = reserved ? Visibility.Visible : Visibility.Collapsed,
        };
        footer.Children.Add(heading);
        footer.Children.Add(body);
        var panel = new StackPanel { Width = Width };
        panel.Children.Add(scroll);
        panel.Children.Add(footer);
        Flyout.Content = panel;
        // Opened, the keyboard starts on the episode playing.
        Flyout.Opened -= OpenedAt;
        focusAt = current;
        Flyout.Opened += OpenedAt;
    }

    Button? focusAt;

    void OpenedAt(object? sender, object e) => focusAt?.Focus(FocusState.Programmatic);
}
