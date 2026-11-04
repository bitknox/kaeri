using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Pages;

/// <summary>
/// Go to episode: a ContentDialog with a NumberBox headed Episode, the show's span under it ("1 to
/// 412"), and Go in the selected fill. The engine decides what a number means; one that is not a
/// row keeps the dialog open with the reason. The title page and the player's Episodes panel share it.
/// </summary>
static class GoToDialog
{
    /// <summary>The row the number named, or null when the dialog was cancelled.</summary>
    public static async Task<GoTo.Row?> Ask(XamlRoot root, EpisodeRow[] rows, string? stage = null)
    {
        var box = new NumberBox
        {
            Header = T.LibraryEpisode,
            // The show's span under the field, as the board draws it: "1 to 412".
            Description = KaeriFfiMethods.GoToHint(rows),
            Minimum = 1,
            SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Compact,
            AcceptsExpression = false,
            MinWidth = 280,
        };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, KaeriFfiMethods.GoToLabel(rows));
        var error = new TextBlock { Style = (Style)Application.Current.Resources["HazeCaptionStyle"], TextWrapping = TextWrapping.WrapWholeWords, Visibility = Visibility.Collapsed };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetLiveSetting(error, Microsoft.UI.Xaml.Automation.Peers.AutomationLiveSetting.Polite);
        var body = new StackPanel { Spacing = 8 };
        body.Children.Add(box);
        body.Children.Add(error);
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.EpisodeGoToTitle,
            Content = body,
            PrimaryButtonText = T.ActionGo,
            CloseButtonText = T.ActionCancel,
            // Go is the default in the selected fill, not amber: the dialog's own default style
            // would make it the accent, so Enter in the field goes instead (below).
            DefaultButton = ContentDialogButton.None,
            PrimaryButtonStyle = (Style)Application.Current.Resources["SelectedFillButtonStyle"],
        };
        GoTo? landed = null;
        // True when the number typed is a row; else the dialog says why and stays open.
        bool Take()
        {
            var text = new string((box.Text ?? "").Where(char.IsAsciiDigit).ToArray());
            var result = Browsing.Go(rows, text);
            if (result is GoTo.Row)
            {
                landed = result;
                return true;
            }
            error.Text = Browsing.GoMessage(result) ?? "";
            error.Visibility = error.Text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
            return false;
        }
        dialog.PrimaryButtonClick += (_, args) => args.Cancel = !Take();
        box.KeyDown += (_, a) =>
        {
            if (a.Key != global::Windows.System.VirtualKey.Enter)
                return;
            a.Handled = true;
            if (Take())
                dialog.Hide();
        };
        await Dialogs.Show(dialog, () =>
        {
            box.Focus(FocusState.Programmatic);
#if KAERI_TESTING
            // A shot's number, put in the box and taken as Go would. "go:128" types it a digit
            // at a time, then presses Go, for the Go to episode clip.
            if (stage is not null && stage.StartsWith("go:", StringComparison.Ordinal))
            {
                var digits = stage[3..];
                var at = 0;
                var timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(260) };
                timer.Tick += (_, _) =>
                {
                    if (at < digits.Length)
                    {
                        at++;
                        box.Text = digits[..at];
                        return;
                    }
                    timer.Stop();
                    if (Take())
                        dialog.Hide();
                };
                timer.Start();
            }
            else if (stage is not null)
            {
                box.Text = stage;
                Take();
            }
#endif
        });
        return landed as GoTo.Row;
    }
}
