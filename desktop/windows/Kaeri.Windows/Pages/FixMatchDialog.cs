using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Pages;

/// <summary>
/// "Which title is this folder?": the match picker the other way round. The search field starts
/// with the parsed title; the results are AniList titles, and one click picks one. A result whose
/// episode count does not fit the files is dimmed with the reason, and can still be picked.
/// </summary>
sealed class FixMatchDialog
{
    readonly Ctx ctx;
    readonly ContentDialog dialog;
    readonly LocalGroup group;
    readonly TextBox field = new();
    readonly StackPanel results = new() { Spacing = 2 };
    readonly StackPanel body = new() { Spacing = 10, MinWidth = 480 };
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer typing;
    string? asked;

    public FixMatchDialog(Ctx ctx, XamlRoot root, LocalGroup group)
    {
        this.ctx = ctx;
        this.group = group;
        AutomationProperties.SetName(field, T.NavSearch);
        field.Text = group.Name;
        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.LibraryFixTitle,
            Content = new ScrollViewer { Content = body, MaxHeight = 520, VerticalScrollBarVisibility = ScrollBarVisibility.Auto },
            SecondaryButtonText = T.LibraryLeaveUnmatched,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Close,
        };
        dialog.SecondaryButtonClick += (_, _) => ctx.Send(new Command.Local(new LocalCommand.Keep(group.Id)));
        body.Children.Add(Ui.Haze($"{group.Name} · {T.LibraryFiles(group.Files)}"));
        body.Children.Add(field);
        body.Children.Add(results);
        typing = dialog.DispatcherQueue.CreateTimer();
        typing.Interval = TimeSpan.FromMilliseconds(350);
        typing.IsRepeating = false;
        typing.Tick += (_, _) => Ask();
        field.TextChanged += (_, _) => { typing.Stop(); typing.Start(); };
    }

    public async Task ShowAsync()
    {
        ctx.Batch += Heard;
        Ask();
        try
        {
            await Dialogs.Show(dialog);
        }
        finally
        {
            ctx.Batch -= Heard;
            typing.Stop();
        }
    }

    /// <summary>Fix this match: the engine searches AniList for the group, sharing the matching limit.</summary>
    void Ask()
    {
        var text = field.Text.Trim();
        if (text.Length == 0)
            return;
        asked = text;
        ctx.Send(new Command.Local(new LocalCommand.FindMatch(group.Id, text)));
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        // An answer to an older search carries its own query, and is ignored.
        if (batch.Any(e => e is Event.Local { V1: LocalEvent.Matches m } && m.Group == group.Id && m.Query == asked))
            Show();
    }

    void Show()
    {
        results.Children.Clear();
        if (ctx.Known.LocalMatches is not { } found || found.Group != group.Id || found.Query != asked)
            return;
        if (found.Offline)
        {
            results.Children.Add(Ui.Haze(T.ErrorServiceUnreachable("AniList")));
            return;
        }
        foreach (var choice in found.Choices.Take(8))
        {
            var parts = new List<string>();
            if (choice.Format is { Length: > 0 } format)
                parts.Add(Wording.Format(format));
            if (choice.Year is { } year)
                parts.Add(year.ToString(System.Globalization.CultureInfo.InvariantCulture));
            if (choice.Episodes is { } episodes)
                parts.Add(T.EpisodesCount(episodes));
            var detail = T.MatchChoiceLine(string.Join(" · ", parts), choice.Confidence);
            var line = new StackPanel { Spacing = 0, Opacity = choice.Fits ? 1 : 0.6, HorizontalAlignment = HorizontalAlignment.Left };
            line.Children.Add(Ui.Text(choice.Title, "BodyStrongTextBlockStyle", wrap: false));
            line.Children.Add(Ui.Text(detail, "HazeCaptionStyle", wrap: false));
            if (!choice.Fits)
                line.Children.Add(Ui.Text(T.LibraryFixCountMismatch(choice.Episodes ?? 0, group.Files), "HazeCaptionStyle", wrap: false));
            var row = new Button
            {
                Content = line,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Left,
                Style = Ui.Res<Style>("SubtleButtonStyle"),
                MinHeight = 52,
            };
            AutomationProperties.SetName(row, $"{choice.Title}, {detail}");
            row.Click += (_, _) =>
            {
                ctx.Send(new Command.Local(new LocalCommand.Choose(group.Id, choice.Anilist)));
                dialog.Hide();
            };
            results.Children.Add(row);
        }
    }
}
