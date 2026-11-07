using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Kaeri.Windows.Pages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Sync;

/// <summary>
/// What a library pull found different from AniList, as the Mac's conflicts sheet lists it: each
/// title, the field with both sides, and which side to keep. A side kept settles the row at once:
/// it fades out and only it goes, and the dialog closes by itself once none are left. The dialog
/// keeps one size while rows go, and a pull that brings a new list while it is open adds or takes
/// away only the rows that changed.
/// </summary>
static class ConflictsDialog
{
    public static async Task Show(Ctx ctx, XamlRoot root)
    {
        if (ctx.Known.Conflicts.Count == 0)
            return;
        var tracker = KaeriFfiMethods.TrackerName("anilist");
        var list = new StackPanel();
        var rows = new Dictionary<uint, Border>();
        ContentDialog? dialog = null;

        void Fill()
        {
            var desired = new List<UIElement>();
            foreach (var conflict in ctx.Known.Conflicts.DistinctBy(c => c.AnilistId))
            {
                if (!rows.TryGetValue(conflict.AnilistId, out var row))
                    rows[conflict.AnilistId] = row = Row(conflict);
                row.BorderThickness = new Thickness(0, desired.Count == 0 ? 0 : 1, 0, 0);
                desired.Add(row);
            }
            foreach (var gone in rows.Keys.Where(id => !ctx.Known.Conflicts.Any(c => c.AnilistId == id)).ToList())
                rows.Remove(gone);
            Drawn.Arrange(list.Children, desired, Ui.HoldingFocus(list, list.Children));
            if (desired.Count == 0)
                dialog?.Hide();
        }

        Border Row(ConflictRow conflict)
        {
            var grid = new Grid { ColumnSpacing = 8, Padding = new Thickness(0, 10, 0, 10) };
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
            words.Children.Add(new TextBlock { Text = conflict.Title, Style = Ui.Res<Style>("ShowTitleStyle"), TextWrapping = TextWrapping.WrapWholeWords });
            var line = T.AccountsClashRow(conflict.Field.FirstUpper(), conflict.Local, conflict.Remote, tracker);
            words.Children.Add(new TextBlock { Text = line, Foreground = Ui.Res<Brush>("HazeBrush"), TextWrapping = TextWrapping.WrapWholeWords });
            grid.Children.Add(words);
            var row = new Border { Child = grid, BorderBrush = Ui.Res<Brush>("FaintRuleBrush") };
            void Keep(bool local)
            {
                ctx.Send(ctx.Known.Resolve(conflict.AnilistId, local));
                // The next row's first button takes the focus before this one fades away.
                var at = list.Children.IndexOf(row);
                var next = list.Children.Skip(at + 1).Concat(list.Children.Take(Math.Max(0, at)).Reverse()).OfType<Border>().FirstOrDefault();
                ((next?.Child as Grid)?.Children.OfType<Button>().FirstOrDefault())?.Focus(FocusState.Keyboard);
                Motion.Hide(row);
                row.DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, async () =>
                {
                    await Task.Delay(Motion.FadeOutTime + TimeSpan.FromMilliseconds(50));
                    Fill();
                });
            }
            var mine = new Button { Content = T.AccountsKeepMine, VerticalAlignment = VerticalAlignment.Center };
            AutomationProperties.SetName(mine, $"{T.AccountsKeepMine}, {conflict.Title}");
            mine.Click += (_, _) => Keep(true);
            var theirs = new Button { Content = T.AccountsKeepTheirs(tracker), VerticalAlignment = VerticalAlignment.Center };
            AutomationProperties.SetName(theirs, $"{T.AccountsKeepTheirs(tracker)}, {conflict.Title}");
            theirs.Click += (_, _) => Keep(false);
            Grid.SetColumn(mine, 1);
            Grid.SetColumn(theirs, 2);
            grid.Children.Add(mine);
            grid.Children.Add(theirs);
            AutomationProperties.SetName(row, $"{conflict.Title}, {line}");
            return row;
        }

        Fill();
        var body = new StackPanel { Spacing = 12, Width = 560 };
        body.Children.Add(new TextBlock { Text = T.AccountsClashDetail(tracker), Foreground = Ui.Res<Brush>("HazeBrush"), TextWrapping = TextWrapping.WrapWholeWords });
        body.Children.Add(new ScrollViewer { Content = list, Height = 320, VerticalScrollBarVisibility = ScrollBarVisibility.Auto, Padding = new Thickness(0, 0, 16, 0) });
        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.AccountsClashTitle(tracker),
            Content = body,
            CloseButtonText = T.ActionDone,
            DefaultButton = ContentDialogButton.Close,
        };
        // Wider than a ContentDialog's usual 548, for the two buttons beside each title.
        dialog.Resources["ContentDialogMaxWidth"] = 640.0;
        void Heard(IReadOnlyList<Event> batch)
        {
            if (batch.Any(e => e is Event.Conflicts))
                Fill();
        }
        ctx.Batch += Heard;
        try
        {
            await Dialogs.Show(dialog);
        }
        finally
        {
            ctx.Batch -= Heard;
        }
    }
}
