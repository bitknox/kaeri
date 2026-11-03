using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Surprise me: asks the engine for a random title from the pool the page names and opens it.
/// A pick that takes over 0.6 s shows a ring where the shuffle glyph was; a second press while one
/// is out does nothing. The engine words the empty, offline and failed notices itself.
/// </summary>
sealed class SurpriseButton : Button
{
    readonly Ctx ctx = Ctx.Current;
    readonly Func<SurprisePool> pool;
    readonly FontIcon glyph = new() { Glyph = "", FontSize = 14 };
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
