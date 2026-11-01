using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Pages;

/// <summary>
/// "Which episode is this file?": a Season and an Episode stepper for a file whose name has no
/// number. Season 0 holds specials. The file waits in Not matched until this is applied.
/// </summary>
sealed class SetEpisodeDialog
{
    readonly ContentDialog dialog;

    public SetEpisodeDialog(Ctx ctx, XamlRoot root, UnnumberedFile file)
    {
        var season = Stepper(T.LibrarySeason, 1, 0);
        var episode = Stepper(T.LibraryEpisode, 1, 1);
        var body = new StackPanel { Spacing = 12, MinWidth = 360 };
        body.Children.Add(Ui.Haze(file.Name));
        body.Children.Add(season);
        body.Children.Add(episode);
        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.LibraryEpisodeTitle,
            Content = body,
            PrimaryButtonText = T.LibraryApply,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
        };
        dialog.PrimaryButtonClick += (_, _) =>
            ctx.Send(new Command.Local(new LocalCommand.SetEpisode(file.Group, file.Path, (uint)((NumberBox)season.Children[1]).Value, (uint)((NumberBox)episode.Children[1]).Value)));
    }

    public Task ShowAsync() => Dialogs.Show(dialog);

    /// <summary>A label above a number box with plus and minus, as a stepper.</summary>
    static StackPanel Stepper(string label, double value, double minimum)
    {
        var panel = new StackPanel { Spacing = 4 };
        var box = new NumberBox
        {
            Value = value,
            Minimum = minimum,
            Maximum = 9999,
            SmallChange = 1,
            SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Inline,
            ValidationMode = NumberBoxValidationMode.InvalidInputOverwritten,
        };
        AutomationProperties.SetName(box, label);
        panel.Children.Add(Ui.Text(label, "BodyTextBlockStyle"));
        panel.Children.Add(box);
        return panel;
    }
}
