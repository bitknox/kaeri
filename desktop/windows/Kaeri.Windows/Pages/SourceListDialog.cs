using Kaeri.App;
using Kaeri.Engine.Core;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// What each source lists for an episode, to pick one and play it here: a cast that couldn't
/// start on a TV, or a start that found nothing to play, opens it.
/// </summary>
sealed class SourceListDialog
{
    readonly Ctx ctx;
    readonly ContentDialog dialog;
    readonly uint id;
    readonly string episode;
    readonly StackPanel body = new() { Spacing = 12, Width = 440 };
    readonly ListView list = new() { SelectionMode = ListViewSelectionMode.Single, MaxHeight = 260 };
    readonly ProgressRing ring = new() { Width = 16, Height = 16, IsActive = true };
    readonly FontIcon mark = new() { Glyph = "", FontSize = 16, Opacity = 0 };
    readonly TextBlock line = Ui.Text("", "HazeBodyStyle");
    /// <summary>What each source said when none listed a release, one line each.</summary>
    readonly StackPanel reasons = new() { Spacing = 4 };
    SourceCandidate[]? candidates;
    bool choosing, failed, openProviders;

    public SourceListDialog(Ctx ctx, XamlRoot root, uint id, string episode)
    {
        this.ctx = ctx;
        this.id = id;
        this.episode = episode;
        dialog = new ContentDialog
        {
            XamlRoot = root,
            Content = body,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
            PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle"),
        };
        dialog.PrimaryButtonClick += OnPrimary;
        list.SelectionChanged += (_, _) => dialog.IsPrimaryButtonEnabled = list.SelectedIndex >= 0;
        AutomationProperties.SetName(list, T.SourcesLabel);
        AutomationProperties.SetLiveSetting(line, AutomationLiveSetting.Polite);
        Choose();
    }

    public async Task ShowAsync()
    {
        ctx.Batch += Heard;
        var answer = ContentDialogResult.None;
        try
        {
            answer = await Dialogs.Show(dialog);
        }
        finally
        {
            ctx.Batch -= Heard;
        }
        // Only Open sources goes there: Cancel and Escape stay where they are.
        if (openProviders && answer == ContentDialogResult.Primary)
            ctx.Go("providers");
    }

    void OnPrimary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        if (openProviders)
            return;
        if (failed)
        {
            args.Cancel = true;
            Choose();
            return;
        }
        if (list.SelectedIndex < 0 || candidates is null)
        {
            args.Cancel = true;
            return;
        }
        var pick = candidates[list.SelectedIndex];
        ctx.Send(new Command.PlaySource(id, episode, pick.ProviderId, pick.Id));
    }

    /// <summary>Step two: what each source lists for the episode, loading in place.</summary>
    void Choose()
    {
        dialog.Title = T.SourcesChooseFor(episode);
        dialog.PrimaryButtonText = T.ActionPlay;
        dialog.IsPrimaryButtonEnabled = false;
        if (!choosing)
        {
            body.Children.Clear();
            // The ring while looking, the error mark in its place if that failed: the line stays put.
            var waiting = new Grid { ColumnSpacing = 8, MinHeight = 20 };
            waiting.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            waiting.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            waiting.Children.Add(ring);
            waiting.Children.Add(mark);
            Grid.SetColumn(line, 1);
            waiting.Children.Add(line);
            body.Children.Add(waiting);
            body.Children.Add(reasons);
            body.Children.Add(list);
        }
        choosing = true;
        failed = false;
        candidates = null;
        reasons.Children.Clear();
        list.Items.Clear();
        list.Opacity = 0;
        ring.IsActive = true;
        ring.Opacity = 1;
        mark.Opacity = 0;
        line.Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush");
        line.Text = T.SourcesLooking;
        ctx.Send(new Command.LoadSources(id, episode));
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.Sources s when choosing && s.Id == id && s.Episode == episode:
                    Listed(s.Candidates, s.Missed);
                    break;
                // The engine says why it found nothing as an alert, not as an empty list.
                case Event.Toast t when choosing && candidates is null && !failed && t.V1.Kind == NoticeKind.Alert:
                    Failed(t.V1.Msg is { } said ? said.Text() : T.SourcesLoadFailed);
                    break;
            }
        }
    }

    void Failed(string why)
    {
        failed = true;
        ring.IsActive = false;
        ring.Opacity = 0;
        mark.Glyph = "\uEA39";
        mark.Foreground = Ui.Res<Brush>("SystemFillColorCriticalBrush");
        mark.Opacity = 1;
        line.Foreground = Ui.Res<Brush>("SystemFillColorCriticalBrush");
        line.Text = why;
        dialog.PrimaryButtonText = T.ActionTryAgain;
        dialog.IsPrimaryButtonEnabled = true;
    }

    void Listed(SourceCandidate[] found, MissedListing[] missed)
    {
        candidates = found;
        ring.IsActive = false;
        ring.Opacity = 0;
        list.Items.Clear();
        // An answer, not a failure: the mark in the ring's place says so in the line's colour.
        mark.Glyph = "\uE946";
        mark.Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush");
        mark.Opacity = 1;
        reasons.Children.Clear();
        if (found.Length == 0 && missed.Length > 0)
        {
            // What each source said, by the name it goes by: one that failed or was held back
            // is not one without the episode, and Try again asks it anew. The line over them
            // claims only what the sources that answered said.
            line.Text = ReleasePicker.NoneLists(missed, episode);
            foreach (var m in missed)
                reasons.Children.Add(Ui.Text(ReleasePicker.Missed(m, episode), "BodyTextBlockStyle"));
            if (missed.Any(m => ReleasePicker.Unreached(m.Miss)))
            {
                failed = true;
                dialog.PrimaryButtonText = T.ActionTryAgain;
            }
            else
            {
                dialog.PrimaryButtonText = T.SourcesOpen;
                openProviders = true;
            }
            dialog.IsPrimaryButtonEnabled = true;
            return;
        }
        if (found.Length == 0)
        {
            line.Text = T.SourcesNoneList(episode);
            dialog.PrimaryButtonText = T.SourcesOpen;
            dialog.IsPrimaryButtonEnabled = true;
            openProviders = true;
            return;
        }
        // In a room the pick plays for everyone, and the line says so.
        line.Text = ctx.Known.Room is not null ? T.RoomSourcesFoundLine(found.Length) : T.PlayerSourcesFoundLine(found.Length);
        foreach (var c in found)
        {
            var row = new StackPanel { Padding = new Thickness(0, 4, 0, 4) };
            row.Children.Add(Ui.Text(c.Title, wrap: false));
            row.Children.Add(Ui.Text(ReleasePicker.Detail(c), "HazeCaptionStyle", wrap: false));
            list.Items.Add(new ListViewItem { Content = row });
        }
        // The only release is the one to play: picked already, so Play works at once.
        list.SelectedIndex = ReleasePicker.Preselected(found);
        dialog.IsPrimaryButtonEnabled = list.SelectedIndex >= 0;
        Motion.Show(list);
    }
}
