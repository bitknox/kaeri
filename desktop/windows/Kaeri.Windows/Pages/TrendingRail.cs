using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Search's Trending rail: a heading that says its period, a DropDownButton to change it, See all
/// into the sort, and the first dozen covers. A period switch crossfades the covers with both sides
/// drawn; a period that fails says so with Try again, and the others keep working.
/// </summary>
sealed class TrendingRail : StackPanel
{
    const int Shown = 12;
    static DateTimeOffset visited = DateTimeOffset.MinValue;

    readonly Ctx ctx = Ctx.Current;
    readonly Action<TrendPeriod> seeAll;
    readonly TextBlock heading = new() { Style = (Style)Application.Current.Resources["SectionTitleStyle"] };
    readonly DropDownButton choice = new() { MinHeight = 28 };
    readonly MenuFlyout menu = new() { Placement = Microsoft.UI.Xaml.Controls.Primitives.FlyoutPlacementMode.BottomEdgeAlignedRight };
    readonly StateHost state = new();
    readonly CoverGrid covers = new() { Rail = true };
    TrendPeriod period;

    public TrendingRail(Action<TrendPeriod> seeAll)
    {
        this.seeAll = seeAll;
        Spacing = 8;
        period = ctx.Known.TrendingRemembered ?? TrendPeriod.Today;
        var head = new Grid { ColumnSpacing = 8 };
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        head.Children.Add(heading);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetHeadingLevel(heading, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level2);
        foreach (var p in Browsing.Periods)
        {
            var item = new RadioMenuFlyoutItem { Text = Browsing.Choice(p), GroupName = "trend", Tag = p };
            item.Click += (_, _) => Switch(p);
            menu.Items.Add(item);
        }
        choice.Flyout = menu;
        Grid.SetColumn(choice, 1);
        head.Children.Add(choice);
        var all = new HyperlinkButton { Content = T.SearchSeeAll };
        all.Click += (_, _) => this.seeAll(period);
        Grid.SetColumn(all, 2);
        head.Children.Add(all);
        Children.Add(head);
        state.Placeholders = () => States.Covers();
        state.ActivityText = T.StateLoading;
        state.Children.Add(covers);
        Children.Add(state);
        Say();
        Fill(first: true);
    }

    /// <summary>The rail asks again at most once a minute, whenever Search is opened.</summary>
    public void Visit()
    {
        if (DateTimeOffset.UtcNow - visited < TimeSpan.FromMinutes(1))
            return;
        visited = DateTimeOffset.UtcNow;
        ctx.Send(period == TrendPeriod.Today ? new Command.LoadBrowse() : new Command.LoadTrending(period));
    }

    public void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
            if (e is Event.Browse || (e is Event.Trending t && t.Period == period))
                Fill(first: false);
    }

    void Switch(TrendPeriod to)
    {
        if (to == period)
            return;
        period = to;
        Say();
        // Both sides stay drawn: the covers as they were fade out over the new ones.
        Crossfade.Swap(state, () => Fill(first: true));
        if (!ctx.Known.TrendingKept.ContainsKey(to))
            ctx.Send(new Command.LoadTrending(to));
    }

    void Say()
    {
        heading.Text = Browsing.Heading(period);
        choice.Content = Browsing.Choice(period);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(choice, $"{Browsing.Heading(period)}: {Browsing.Choice(period)}");
        foreach (var item in menu.Items.OfType<RadioMenuFlyoutItem>())
            item.IsChecked = (TrendPeriod)item.Tag == period;
    }

    void Fill(bool first)
    {
        if (ctx.Known.TrendingKept.TryGetValue(period, out var entries) && entries.Length > 0)
        {
            covers.Covers = [.. entries.Take(Shown).Select(e => new CoverItem(e).Load(ctx.Art))];
            state.Done();
        }
        else if (ctx.Known.TrendingFailed.Contains(period))
            state.Failed(T.TrendingFailed(Browsing.Choice(period).ToLowerInvariant()), () => ctx.Send(new Command.LoadTrending(period)));
        else
            state.Wait(hasContent: false);
    }
}
