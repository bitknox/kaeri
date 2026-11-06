using Kaeri.App;
using Kaeri.Windows.Pages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;

namespace Kaeri.Windows;

/// <summary>
/// Messages: every notice this run, newest first, the last 200, as the Mac's Messages and the
/// TUI's recent messages keep them. An alert has a vermilion dot and Report. The card keeps one
/// size; a notice said while it is open joins the top of the list, and nothing else moves.
/// </summary>
static class Messages
{
    public static async void Show(Ctx ctx, XamlRoot root)
    {
        if (InfoCard.IsOpen)
            return;
        var list = new StackPanel { Spacing = 0 };
        var none = Ui.Haze(T.MessagesNone);
        none.Margin = new Thickness(0, 8, 0, 0);
        list.Children.Add(none);
        none.Visibility = ctx.Known.Messages.All.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        foreach (var said in ctx.Known.Messages.All)
            list.Children.Add(Row(ctx, root, said, list.Children.Count > 1));
        void Added(Said said)
        {
            none.Visibility = Visibility.Collapsed;
            // Under the empty line, which stays first and hidden; the row before the newest gets its rule.
            if (list.Children.Count > 1 && list.Children[1] is FrameworkElement previous)
                previous.BorderThicknessTop();
            list.Children.Insert(1, Row(ctx, root, said, false));
            while (list.Children.Count > NoticeLog.Kept + 1)
                list.Children.RemoveAt(list.Children.Count - 1);
        }
        ctx.Known.Messages.Added += Added;
        var scroll = new ScrollViewer
        {
            Content = list,
            Height = 360,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            Padding = new Thickness(0, 0, 16, 0),
        };
        AutomationProperties.SetName(scroll, T.MessagesTitle);
        var title = Ui.Text(T.MessagesTitle, "SubtitleTextBlockStyle");
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level1);
        try
        {
            await InfoCard.Show(root, title, scroll, 560, 520);
        }
        finally
        {
            ctx.Known.Messages.Added -= Added;
        }
    }

    /// <summary>A rule above every row but the first.</summary>
    static void BorderThicknessTop(this FrameworkElement row)
    {
        if (row is Border border)
            border.BorderThickness = new Thickness(0, 1, 0, 0);
    }

    static Border Row(Ctx ctx, XamlRoot root, Said said, bool ruled)
    {
        var grid = new Grid { ColumnSpacing = 8, Padding = new Thickness(0, 10, 0, 10) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var dot = new Ellipse
        {
            Width = 6,
            Height = 6,
            Margin = new Thickness(0, 7, 0, 0),
            VerticalAlignment = VerticalAlignment.Top,
            Fill = said.Alert ? Ui.Res<Brush>("SystemFillColorCriticalBrush") : new SolidColorBrush(Microsoft.UI.Colors.Transparent),
        };
        AutomationProperties.SetAccessibilityView(dot, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        grid.Children.Add(dot);
        var text = new TextBlock { Text = said.Text.FirstUpper(), TextWrapping = TextWrapping.WrapWholeWords, IsTextSelectionEnabled = true };
        Grid.SetColumn(text, 1);
        grid.Children.Add(text);
        if (said.Alert)
        {
            var report = new Button { Content = T.ActionReport, VerticalAlignment = VerticalAlignment.Top, Padding = new Thickness(10, 2, 10, 3) };
            AutomationProperties.SetName(report, $"{T.ActionReport}, {said.Text}");
            report.Click += (_, _) =>
            {
                ctx.Known.LastError = new Kaeri.Engine.Model.BugError("message", said.Text);
                InfoCard.HideOpen();
                ReportDialog.Show(ctx, root);
            };
            Grid.SetColumn(report, 2);
            grid.Children.Add(report);
        }
        var time = new TextBlock
        {
            Text = said.At.ToLocalTime().ToString("t", System.Globalization.CultureInfo.CurrentCulture),
            Foreground = Ui.Res<Brush>("HazeBrush"),
            VerticalAlignment = VerticalAlignment.Top,
        };
        Grid.SetColumn(time, 3);
        grid.Children.Add(time);
        var row = new Border
        {
            Child = grid,
            BorderBrush = Ui.Res<Brush>("FaintRuleBrush"),
            BorderThickness = new Thickness(0, ruled ? 1 : 0, 0, 0),
        };
        AutomationProperties.SetName(row, said.Alert ? $"{T.MessagesAlert}, {said.Text}, {time.Text}" : $"{said.Text}, {time.Text}");
        return row;
    }
}
