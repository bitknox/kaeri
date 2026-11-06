using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Search's Trending rail. The heading is the period menu: a DropDownButton styled as the section
/// heading ("Trending today ⌄") with a MenuFlyout of three radio items. See all browses the period
/// shown, and the rail holds its first dozen covers. A period switch crossfades the label on the
/// quick token; the old covers stay 600 ms, then crossfade to placeholders if the new ones are not
/// in, and the placeholders (held at least 0.3 s) crossfade to them, so the shelf is never blank.
/// A period that fails says so with Try again, and the others keep working.
/// </summary>
sealed class TrendingRail : StackPanel
{
    const int Shown = 12;
    static readonly TimeSpan OldStays = TimeSpan.FromMilliseconds(600), PlatesLeast = TimeSpan.FromMilliseconds(300);
    static DateTimeOffset visited = DateTimeOffset.MinValue;

    readonly Ctx ctx = Ctx.Current;
    readonly Action<TrendPeriod> seeAll;
    readonly TextBlock labelA = Label(), labelB = Label();
    readonly DropDownButton heading = new()
    {
        Padding = new Thickness(0, 2, 6, 2),
        Background = new Microsoft.UI.Xaml.Media.SolidColorBrush(Microsoft.UI.Colors.Transparent),
        BorderThickness = new Thickness(0),
    };
    readonly MenuFlyout menu = new() { Placement = Microsoft.UI.Xaml.Controls.Primitives.FlyoutPlacementMode.BottomEdgeAlignedLeft };
    readonly StateHost state = new();
    readonly CoverGrid covers = new() { Rail = true };
    readonly DispatcherTimer oldStays = new() { Interval = OldStays };
    TrendPeriod period;
    bool switching, onA;

    static TextBlock Label() => new() { Style = (Style)Application.Current.Resources["SectionTitleStyle"] };

    public TrendingRail(Action<TrendPeriod> seeAll)
    {
        this.seeAll = seeAll;
        Spacing = 8;
        period = ctx.Known.TrendingRemembered ?? TrendPeriod.Today;
        var head = new Grid { ColumnSpacing = 8 };
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        // Two labels on one spot: the old one fades away as the new one comes in.
        var face = new Grid();
        labelA.Visibility = labelB.Visibility = Visibility.Collapsed;
        face.Children.Add(labelA);
        face.Children.Add(labelB);
        heading.Content = face;
        heading.HorizontalAlignment = HorizontalAlignment.Left;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetHeadingLevel(heading, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level2);
        foreach (var p in Browsing.Periods)
        {
            var item = new RadioMenuFlyoutItem { Text = Browsing.Choice(p), GroupName = "trend", Tag = p };
            item.Click += (_, _) => Switch(p);
            menu.Items.Add(item);
        }
        heading.Flyout = menu;
        head.Children.Add(heading);
        var all = new HyperlinkButton { Content = T.SearchSeeAll, VerticalAlignment = VerticalAlignment.Center };
        all.Click += (_, _) => this.seeAll(period);
        Grid.SetColumn(all, 1);
        head.Children.Add(all);
        Children.Add(head);
        state.Placeholders = () => States.Covers();
        state.ActivityText = T.StateLoading;
        state.Children.Add(covers);
        Children.Add(state);
        oldStays.Tick += (_, _) =>
        {
            oldStays.Stop();
            // Still not in: the old covers crossfade to placeholders.
            if (switching)
                Crossfade.Swap(state, () => state.WaitNow());
        };
        Say(animate: false);
        Fill();
    }

    /// <summary>The rail asks again at most once a minute, whenever Search is opened.</summary>
    public void Visit()
    {
        if (DateTimeOffset.UtcNow - visited < TimeSpan.FromMinutes(1))
            return;
        visited = DateTimeOffset.UtcNow;
        ctx.Send(period == TrendPeriod.Today ? new Command.LoadBrowse() : new Command.LoadTrending(period));
    }

#if KAERI_TESTING
    /// <summary>For a shot: the heading's menu open.</summary>
    public void OpenMenu() => heading.Flyout?.ShowAt(heading);

    /// <summary>For a shot: a period chosen as the menu would.</summary>
    public void Pick(TrendPeriod to)
    {
        menu.Hide();
        Switch(to);
    }
#endif

    public void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
            if (e is Event.Browse || (e is Event.Trending t && t.Period == period))
                Arrived();
    }

    void Switch(TrendPeriod to)
    {
        if (to == period)
            return;
        period = to;
        Say(animate: true);
        if (Has(to) || ctx.Known.TrendingFailed.Contains(to))
        {
            // Kept: the covers crossfade at once.
            switching = false;
            oldStays.Stop();
            Crossfade.Swap(state, Fill);
            return;
        }
        // Not kept: the old covers stay while the period is asked for.
        switching = true;
        oldStays.Stop();
        oldStays.Start();
        ctx.Send(new Command.LoadTrending(to));
    }

    bool Has(TrendPeriod p) => ctx.Known.TrendingKept.TryGetValue(p, out var entries) && entries.Length > 0;

    async void Arrived()
    {
        if (!switching)
        {
            Fill();
            return;
        }
        if (!Has(period) && !ctx.Known.TrendingFailed.Contains(period))
            return;
        switching = false;
        oldStays.Stop();
        // Placeholders that came stand their least time, then crossfade to the new covers.
        if (state.PlaceholdersSince is { } since)
        {
            var left = PlatesLeast - (DateTimeOffset.UtcNow - since);
            if (left > TimeSpan.Zero)
                await Task.Delay(left);
        }
        Crossfade.Swap(state, Fill);
    }

    void Say(bool animate)
    {
        var (from, to) = onA ? (labelA, labelB) : (labelB, labelA);
        onA = !onA;
        to.Text = Browsing.Heading(period);
        if (animate && Motion.Enabled)
        {
            Motion.HideQuick(from);
            Motion.FadeIn(to, Motion.Quick);
        }
        else
        {
            Motion.Set(from, false);
            Motion.Set(to, true);
        }
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(heading, Browsing.Heading(period));
        foreach (var item in menu.Items.OfType<RadioMenuFlyoutItem>())
            item.IsChecked = (TrendPeriod)item.Tag == period;
    }

    void Fill()
    {
        if (ctx.Known.TrendingKept.TryGetValue(period, out var entries) && entries.Length > 0)
        {
            covers.Show([.. entries.Take(Shown)]);
            if (state.PlaceholdersSince is not null)
                state.DoneNow();
            else
                state.Done();
        }
        else if (ctx.Known.TrendingFailed.Contains(period))
            state.Failed(T.TrendingFailed(Browsing.Choice(period).ToLowerInvariant()), () => ctx.Send(new Command.LoadTrending(period)));
        else
            state.Wait(hasContent: false);
    }
}
