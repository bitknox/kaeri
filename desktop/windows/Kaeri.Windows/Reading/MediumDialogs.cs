using Medium = Kaeri.Engine.Core.Medium;
using Microsoft.UI.Xaml.Automation.Peers;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// What Kaeri is for: the three rows first run, the dialogs and Settings share; the dialog the
/// engine asks for after the update that brought reading (Watch chosen) or when this PC joins
/// devices that answer otherwise (theirs chosen); and the confirmation Settings shows before a
/// medium is turned off.
/// </summary>
static class MediumDialogs
{
    static readonly KaeriIsFor[] Answers = [KaeriIsFor.Watch, KaeriIsFor.Read, KaeriIsFor.Both];

    public static string Name(KaeriIsFor answer) => answer switch
    {
        KaeriIsFor.Watch => T.MediumWatch,
        KaeriIsFor.Read => T.MediumRead,
        _ => T.MediumBoth,
    };

    public static string Line(KaeriIsFor answer) => answer switch
    {
        KaeriIsFor.Watch => T.MediumWatchLine,
        KaeriIsFor.Read => T.MediumReadLine,
        _ => T.MediumBothLine,
    };

    /// <summary>
    /// Watch, Read and Both as one group of radio rows, each with its line. <paramref name="chosen"/>
    /// null leaves none chosen, as first run starts.
    /// </summary>
    public static RadioButtons Rows(KaeriIsFor? chosen, Action<KaeriIsFor> picked, string name)
    {
        var rows = new RadioButtons { Name = name, MaxColumns = 1 };
        AutomationProperties.SetName(rows, T.OnboardingMediumTitle);
        for (var i = 0; i < Answers.Length; i++)
        {
            var answer = Answers[i];
            var words = new StackPanel { Spacing = 2, Padding = new Thickness(0, 4, 0, 4) };
            words.Children.Add(Ui.Text(Name(answer), "BodyStrongTextBlockStyle", wrap: false));
            words.Children.Add(Ui.Text(Line(answer), "HazeBodyStyle"));
            var radio = new RadioButton { Content = words, Tag = answer, Name = $"{name}{answer}" };
            AutomationProperties.SetName(radio, Name(answer));
            AutomationProperties.SetHelpText(radio, Line(answer));
            AutomationProperties.SetPositionInSet(radio, i + 1);
            AutomationProperties.SetSizeOfSet(radio, Answers.Length);
            rows.Items.Add(radio);
        }
        rows.SelectedIndex = chosen is { } c ? Array.IndexOf(Answers, c) : -1;
        rows.SelectionChanged += (_, _) =>
        {
            if (rows.SelectedItem is RadioButton { Tag: KaeriIsFor answer })
                picked(answer);
        };
        return rows;
    }

    /// <summary>
    /// The question outside first run: after the update with Watch chosen, or on joining with the
    /// other devices' answer chosen. Continue answers it; closing keeps the chosen row.
    /// </summary>
    public static async Task Ask(Ctx ctx, XamlRoot root, MediumAsk ask)
    {
        KaeriIsFor chosen = ask switch
        {
            MediumAsk.Joined joined => joined.Answer,
            _ => KaeriIsFor.Watch,
        };
        var body = new StackPanel { Spacing = 14, MaxWidth = 460 };
        body.Children.Add(Ui.Text(ask switch
        {
            MediumAsk.Updated => T.MediumUpdateLine(SyncWords.Kind),
            MediumAsk.Joined => T.MediumJoinLine(SyncWords.Kind),
            _ => T.OnboardingMediumLine,
        }, "HazeBodyStyle"));
        body.Children.Add(Rows(chosen, answer => chosen = answer, "MediumAskRows"));
        if (ask is not MediumAsk.FirstRun)
            body.Children.Add(Ui.Text(T.MediumChangeLater, "HazeCaptionStyle"));
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = ask switch
            {
                MediumAsk.Joined joined => JoinTitle(joined),
                _ => T.OnboardingMediumTitle,
            },
            Content = SeriesWords.Scroll(body, 460),
            PrimaryButtonText = T.OnboardingContinue,
            PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle"),
            DefaultButton = ContentDialogButton.Primary,
        };
        // No Cancel: the question is answered either way, with the row chosen when it closes.
        await Dialogs.Show(dialog);
        ctx.Known.Medium.Answered(ask, chosen);
    }

    /// <summary>"{devices} are set to {answer}", the names joined as the list words join them.</summary>
    static string JoinTitle(MediumAsk.Joined joined)
    {
        var devices = joined.Devices;
        var names = devices.Length switch
        {
            0 => "",
            1 => devices[0],
            _ => T.NamesPair(devices.Take(devices.Length - 1).Skip(1).Aggregate(devices[0], T.NamesList), devices[^1]),
        };
        var n = Math.Max(devices.Length, 1);
        return joined.Answer switch
        {
            KaeriIsFor.Watch => T.MediumJoinTitleWatch(names, n),
            KaeriIsFor.Read => T.MediumJoinTitleRead(names, n),
            _ => T.MediumJoinTitleBoth(names, n),
        };
    }

    /// <summary>
    /// Settings' choice: on at once for a medium turned on or Both, and for one turned off only
    /// after the confirmation says what leaves and what stays. False when it was cancelled.
    /// </summary>
    public static async Task<bool> Choose(Ctx ctx, XamlRoot root, KaeriIsFor answer)
    {
        var medium = ctx.Known.Medium;
        var current = medium.Answer;
        if (answer == current)
            return true;
        KaeriIsFor? leaves = answer switch
        {
            KaeriIsFor.Both => null,
            KaeriIsFor.Watch => current == KaeriIsFor.Read || current == KaeriIsFor.Both ? KaeriIsFor.Read : null,
            _ => current == KaeriIsFor.Watch || current == KaeriIsFor.Both ? KaeriIsFor.Watch : null,
        };
        if (leaves is not { } off)
        {
            medium.Choose(answer);
            return true;
        }
        var watch = off == KaeriIsFor.Watch;
        var body = new StackPanel { Spacing = 16, MaxWidth = 440 };
        body.Children.Add(Ui.Text(watch ? T.MediumOffWatchBody(SyncWords.Kind) : T.MediumOffReadBody(SyncWords.Kind), "HazeBodyStyle"));
        var bytes = watch ? ctx.Known.Queue.BytesOnDisk : ctx.Known.Reading.Downloads?.Meter.Used ?? 0;
        var delete = new ToggleSwitch { Name = "MediumOffDelete", IsOn = false, OnContent = "", OffContent = "", MinWidth = 0 };
        var deleteName = watch ? T.MediumOffDeleteWatch : T.MediumOffDeleteRead;
        AutomationProperties.SetName(delete, deleteName);
        var card = new Grid
        {
            ColumnSpacing = 12,
            Padding = new Thickness(14, 10, 10, 10),
            CornerRadius = new CornerRadius(8),
            Background = Ui.Res<Brush>("RaisedBrush"),
        };
        card.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        card.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        words.Children.Add(Ui.Text(deleteName, "BodyTextBlockStyle"));
        if (bytes > 0)
        {
            var size = T.MediumOffDeleteSize(Wording.Bytes(bytes), SyncWords.Kind);
            words.Children.Add(Ui.Text(size, "HazeCaptionStyle"));
            AutomationProperties.SetHelpText(delete, size);
        }
        card.Children.Add(words);
        Grid.SetColumn(delete, 1);
        card.Children.Add(delete);
        body.Children.Add(card);
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = watch ? T.MediumOffTitleWatch : T.MediumOffTitleRead,
            Content = body,
            PrimaryButtonText = watch ? T.MediumOffConfirmWatch : T.MediumOffConfirmRead,
            PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle"),
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Close,
        };
        if (await Dialogs.Show(dialog) != ContentDialogResult.Primary)
            return false;
        medium.Choose(answer, delete.IsOn);
        return true;
    }
}
