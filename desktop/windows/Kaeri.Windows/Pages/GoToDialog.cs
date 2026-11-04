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
            SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Inline,
            AcceptsExpression = false,
            MinWidth = 280,
        };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, KaeriFfiMethods.GoToLabel(rows));
        // A refusal takes the hint's place: the vermilion glyph, then the reason in silk.
        var hint = box.Description;
        var reason = new TextBlock { FontSize = 12, LineHeight = 16, Foreground = Ui.Res<Microsoft.UI.Xaml.Media.Brush>("SilkBrush"), TextWrapping = TextWrapping.WrapWholeWords };
        var glyph = States.ErrorGlyph(13);
        glyph.VerticalAlignment = VerticalAlignment.Top;
        glyph.Margin = new Thickness(0, 1, 0, 0);
        var error = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, Margin = new Thickness(0, 6, 0, 0), Visibility = Visibility.Collapsed };
        error.Children.Add(glyph);
        error.Children.Add(reason);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetLiveSetting(reason, Microsoft.UI.Xaml.Automation.Peers.AutomationLiveSetting.Polite);
        var body = new StackPanel();
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
            reason.Text = Browsing.GoMessage(result) ?? "";
            var refused = reason.Text.Length > 0;
            error.Visibility = refused ? Visibility.Visible : Visibility.Collapsed;
            box.Description = refused ? null : hint;
            return false;
        }
        // Go waits for a number, and typing again puts the hint back.
        void Typed(string text)
        {
            dialog.IsPrimaryButtonEnabled = text.Any(char.IsAsciiDigit);
            error.Visibility = Visibility.Collapsed;
            box.Description = hint;
        }
        dialog.IsPrimaryButtonEnabled = false;
        box.Loaded += (_, _) =>
        {
            if (Ui.Descendant<TextBox>(box) is { } field)
                field.TextChanged += (_, _) => Typed(field.Text);
        };
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
