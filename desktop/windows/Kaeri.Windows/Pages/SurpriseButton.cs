using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Surprise me: asks the engine for a random title from the pool the page names and opens it.
/// A pick that takes over 0.6 s shows a ring where the dice glyph was; a second press while one
/// is out does nothing. The engine words the empty, offline and failed notices itself.
/// </summary>
sealed class SurpriseButton : Button
{
    readonly Ctx ctx = Ctx.Current;
    readonly Func<SurprisePool> pool;
    readonly IconElement glyph = Dice.Icon();
    readonly ProgressRing ring = new() { Width = 14, Height = 14, IsActive = true, Visibility = Visibility.Collapsed };
    readonly DispatcherTimer slow = new() { Interval = TimeSpan.FromMilliseconds(600) };
    bool pending;

    public SurpriseButton(Func<SurprisePool> pool, string hint)
    {
        this.pool = pool;
        MinHeight = 32;
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        row.Children.Add(glyph);
        row.Children.Add(ring);
        row.Children.Add(new TextBlock { Text = T.SurpriseMe });
        Content = row;
        ToolTipService.SetToolTip(this, hint);
        AutomationProperties.SetName(this, T.SurpriseMe);
        AutomationProperties.SetHelpText(this, hint);
        slow.Tick += (_, _) =>
        {
            slow.Stop();
            if (pending)
            {
                glyph.Visibility = Visibility.Collapsed;
                ring.Visibility = Visibility.Visible;
            }
        };
        Click += (_, _) => Press();
    }

    /// <summary>Asks for a pick; ignored while one is out.</summary>
    public void Press()
    {
        if (pending)
            return;
        pending = true;
        slow.Start();
        ctx.Send(new Command.Surprise(pool()));
    }

    /// <summary>The page forwards what the engine says: the answer opens the pick.</summary>
    public void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            if (e is not Event.Surprise answer || !pending)
                continue;
            pending = false;
            slow.Stop();
            ring.Visibility = Visibility.Collapsed;
            glyph.Visibility = Visibility.Visible;
            if (answer.Outcome is SurpriseOutcome.Picked picked)
            {
                ctx.Pick = new Pick(picked.Id, answer.Pool, picked.More);
                ctx.OpenTitle(picked.Id);
            }
        }
    }
}

/// <summary>
/// The dice glyph of Surprise me and Another, which Segoe Fluent Icons has none of: a rounded
/// square with five pips, 16 px, in the text colour.
/// </summary>
static class Dice
{
    const string Data =
        "F0 M4,1.5 H12 A2.5,2.5 0 0 1 14.5,4 V12 A2.5,2.5 0 0 1 12,14.5 H4 A2.5,2.5 0 0 1 1.5,12 V4 A2.5,2.5 0 0 1 4,1.5 Z "
        + "M4,2.8 A1.2,1.2 0 0 0 2.8,4 V12 A1.2,1.2 0 0 0 4,13.2 H12 A1.2,1.2 0 0 0 13.2,12 V4 A1.2,1.2 0 0 0 12,2.8 Z "
        + "M4.3,5.3 A1,1 0 1 1 6.3,5.3 A1,1 0 1 1 4.3,5.3 Z M9.7,5.3 A1,1 0 1 1 11.7,5.3 A1,1 0 1 1 9.7,5.3 Z "
        + "M7,8 A1,1 0 1 1 9,8 A1,1 0 1 1 7,8 Z "
        + "M4.3,10.7 A1,1 0 1 1 6.3,10.7 A1,1 0 1 1 4.3,10.7 Z M9.7,10.7 A1,1 0 1 1 11.7,10.7 A1,1 0 1 1 9.7,10.7 Z";

    public static IconElement Icon() => new PathIcon
    {
        Data = (Microsoft.UI.Xaml.Media.Geometry)Microsoft.UI.Xaml.Markup.XamlBindingHelper.ConvertValue(typeof(Microsoft.UI.Xaml.Media.Geometry), Data),
        Width = 16,
        Height = 16,
        VerticalAlignment = VerticalAlignment.Center,
    };
}
