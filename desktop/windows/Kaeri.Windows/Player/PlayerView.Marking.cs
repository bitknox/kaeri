using System;
using System.Collections.Generic;
using System.Linq;
using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Player;

/// <summary>
/// Marking skip times (Alt+K), the prompts about them and the Skip times panel. The engine holds
/// the steps and the rules; this draws the bar it describes and sends the presses back.
/// </summary>
public sealed partial class PlayerView
{
    SkipMarkView? marking;
    Border? markBar;
    SkipPrompt? skipPrompt;
    Border? promptCard;
    DispatcherQueueTimer? promptTimer;
    bool sharing;
    /// <summary>A screenshot run's prompt, kept up whatever the engine says.</summary>
    bool promptHeld;

    static string KindName(SkipKind kind) => kind switch
    {
        SkipKind.Ending => T.PlayerMarkKindEnding,
        SkipKind.Recap => T.PlayerMarkKindRecap,
        _ => T.PlayerMarkKindOpening,
    };

    static string KindInline(SkipKind kind) => kind switch
    {
        SkipKind.Ending => T.PlayerMarkKindEndingInline,
        SkipKind.Recap => T.PlayerMarkKindRecapInline,
        _ => T.PlayerMarkKindOpeningInline,
    };

    static string KindPlural(SkipKind kind) => kind switch
    {
        SkipKind.Ending => T.PlayerMarkKindEndingPlural,
        SkipKind.Recap => T.PlayerMarkKindRecapPlural,
        _ => T.PlayerMarkKindOpeningPlural,
    };

    /// <summary>"1 min 30 s": the review's length.</summary>
    static string Length(double seconds)
    {
        var s = (long)Math.Round(seconds);
        var minutes = s / 60;
        var rest = s % 60;
        if (minutes == 0)
            return T.DurationSeconds(rest);
        return rest > 0 ? T.DurationMinutesSeconds(minutes, rest.ToString("00")) : T.DurationMinutes(minutes);
    }

    static string Most(double seconds) => T.TimeMinutes((long)Math.Max(1, Math.Round(seconds / 60)));

    bool Marking => marking is not null;

    static TextBlock Tabular(TextBlock text)
    {
        Microsoft.UI.Xaml.Documents.Typography.SetNumeralAlignment(text, FontNumeralAlignment.Tabular);
        return text;
    }

    /// <summary>What the engine says about the marking bar, the prompts and the notices.</summary>
    void HeardSkip(Event e)
    {
        switch (e)
        {
            case Event.PlayerSkipMark m:
                marking = m.View;
                DrawMarking();
                DrawTimeline();
                if (marking is null)
                    Focus(FocusState.Programmatic);
                else
                    ShowControls();
                break;
            case Event.PlayerSkipPrompt p:
                if (!promptHeld || p.Prompt is not null)
                    ShowPrompt(p.Prompt);
                break;
            case Event.PlayerSkipNotice n:
                Say(NoticeText(n.Notice));
                break;
        }
    }

    static string NoticeText(SkipNotice notice) => notice switch
    {
        SkipNotice.Saved s => T.PlayerMarkSaved(KindInline(s.Kind)),
        SkipNotice.SavedHere h => T.PlayerMarkTooLong(KindPlural(h.Kind), Most(h.Most)),
        SkipNotice.Confirmed => T.PlayerMarkConfirmed,
        SkipNotice.Reported => T.PlayerSkipReported,
        _ => T.PlayerSkipHiddenHere,
    };

    // Alt+K, and the keys while the bar is up.

    void OnKeyMark(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () =>
    {
        if (Marking)
            Send(new PlayerCommand.CancelSkipMark());
        else
            BeginMarking(null);
    });

    void BeginMarking(SkipKind? kind)
    {
        if (ctx?.Known.Party is not null)
        {
            Say(T.PlayerMarkUnavailable);
            return;
        }
        Send(new PlayerCommand.BeginSkipMark(kind));
    }

    /// <summary>Left and Right while the bar is on its first two steps: the playhead by a second, five with Shift.</summary>
    bool MarkingNudge(int seconds)
    {
        if (marking is not { Step: not SkipMarkStep.Review })
            return false;
        Send(new PlayerCommand.NudgeSkipEdge(marking.Step == SkipMarkStep.Start ? SkipEdge.Start : SkipEdge.End, seconds * 1000L));
        return true;
    }

    /// <summary>Enter on the bar: mark the edge, or save the review. False when the bar is not up.</summary>
    bool MarkingEnter()
    {
        if (marking is null)
            return false;
        if (marking.Step == SkipMarkStep.Review)
        {
            if (marking.CanSave)
                Send(new PlayerCommand.SaveSkipMark());
        }
        else
            Send(new PlayerCommand.MarkSkipEdge());
        return true;
    }

    // The bar: at the top of the video, raised, radius 8, buttons 32 high with radius 4.

    void DrawMarking()
    {
        if (marking is not { } view)
        {
            if (markBar is { Visibility: Visibility.Visible })
                Motion.Hide(markBar);
            return;
        }
        if (markBar is null)
        {
            markBar = new Border
            {
                HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Top,
                Margin = new Thickness(16, 64, 16, 0), MaxWidth = 720, MinWidth = 420,
                Background = (Brush)Application.Current.Resources["RaisedBrush"], CornerRadius = new CornerRadius(8),
                Padding = new Thickness(16, 10, 12, 12), Visibility = Visibility.Collapsed,
            };
            AutomationProperties.SetName(markBar, T.PlayerMarkSkipTimes);
            VideoPanel.Children.Add(markBar);
        }
        var body = new StackPanel { Spacing = 10 };
        var head = new Grid { ColumnSpacing = 8 };
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        if (view.Step == SkipMarkStep.Review)
        {
            var title = new TextBlock
            {
                Text = T.PlayerMarkReviewTitle(KindName(view.Kind), Length(view.Length)), VerticalAlignment = VerticalAlignment.Center,
                Style = (Style)Application.Current.Resources["BodyStrongTextBlockStyle"],
            };
            AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level2);
            head.Children.Add(title);
        }
        else
            head.Children.Add(KindChoice(view));
        var cancel = new Button
        {
            Style = (Style)Application.Current.Resources["PlayerButtonStyle"], Width = 32, Height = 32,
            Content = new FontIcon { Glyph = "", FontSize = 12 },
        };
        AutomationProperties.SetName(cancel, T.ActionCancel);
        cancel.Click += (_, _) => Send(new PlayerCommand.CancelSkipMark());
        Grid.SetColumn(cancel, 1);
        head.Children.Add(cancel);
        body.Children.Add(head);
        if (view.Step == SkipMarkStep.Review)
            Review(view, body);
        else
            Steps(view, body);
        markBar.Child = body;
        Motion.Show(markBar);
        Announce(body);
    }

    UIElement KindChoice(SkipMarkView view)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 4 };
        foreach (var kind in new[] { SkipKind.Opening, SkipKind.Ending, SkipKind.Recap })
        {
            var choice = new ToggleButton
            {
                Content = KindName(kind), IsChecked = kind == view.Kind, MinHeight = 32, Padding = new Thickness(12, 0, 12, 0),
                CornerRadius = new CornerRadius(4),
            };
            var picked = kind;
            choice.Click += (_, _) =>
            {
                if (picked != view.Kind)
                    Send(new PlayerCommand.SetSkipMarkKind(picked));
                else
                    choice.IsChecked = true;
            };
            row.Children.Add(choice);
        }
        AutomationProperties.SetName(row, T.PlayerMarkSkipTimes);
        return row;
    }

    /// <summary>The first two steps: what to do, and the one button that does it.</summary>
    void Steps(SkipMarkView view, StackPanel body)
    {
        var start = view.Step == SkipMarkStep.Start;
        var row = new Grid { ColumnSpacing = 12 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var line = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        line.Children.Add(new TextBlock
        {
            Text = start ? T.PlayerMarkStepStart(KindInline(view.Kind)) : T.PlayerMarkStepEnd(KindInline(view.Kind)),
            TextWrapping = TextWrapping.WrapWholeWords,
        });
        if (view.Start is { } at)
            line.Children.Add(Tabular(new TextBlock
            {
                Text = $"{T.PlayerMarkStarts} {Wording.Clock(at)}", Style = (Style)Application.Current.Resources["HazeCaptionStyle"],
            }));
        row.Children.Add(line);
        var mark = new Button
        {
            Content = start ? T.PlayerMarkStart : T.PlayerMarkEnd, Style = (Style)Application.Current.Resources["AccentButtonStyle"],
            MinHeight = 32, MinWidth = 112, CornerRadius = new CornerRadius(4),
        };
        mark.Click += (_, _) => Send(new PlayerCommand.MarkSkipEdge());
        Grid.SetColumn(mark, 1);
        row.Children.Add(mark);
        body.Children.Add(row);
    }

    /// <summary>The review: the edges with their nudges, what saving does, Watch the edges and Save.</summary>
    void Review(SkipMarkView view, StackPanel body)
    {
        body.Children.Add(Edge(T.PlayerMarkStarts, T.PlayerMarkEdgeStart, SkipEdge.Start, view.Start ?? 0));
        body.Children.Add(Edge(T.PlayerMarkEnds, T.PlayerMarkEdgeEnd, SkipEdge.End, view.End ?? 0));
        var haze = (Style)Application.Current.Resources["HazeCaptionStyle"];
        // The notes sit to the left of the buttons, at the bottom of the row, as the board draws them.
        var notes = new StackPanel { VerticalAlignment = VerticalAlignment.Bottom, HorizontalAlignment = HorizontalAlignment.Left };
        void Note(string text) => notes.Children.Add(new TextBlock { Text = text, Style = haze, TextWrapping = TextWrapping.WrapWholeWords });
        if (view.Check is SkipMarkCheck.TooShort)
            Note(T.PlayerMarkTooShort);
        else if (view.Check is SkipMarkCheck.TooLong tooLong)
            Note(T.PlayerMarkTooLong(KindPlural(view.Kind), Most(tooLong.Most)));
        foreach (var note in view.Notes ?? [])
            switch (note)
            {
                case SkipMarkNote.Found:
                    Note(T.PlayerMarkFound);
                    break;
                case SkipMarkNote.Shares:
                    Note(T.PlayerMarkShares);
                    break;
                case SkipMarkNote.Replaces r:
                    Note(T.PlayerMarkReplaces(Wording.Clock(r.Start), Wording.Clock(r.End)));
                    break;
                case SkipMarkNote.NoEntry:
                    Note(T.PlayerMarkNoMal);
                    break;
            }
        var actions = new Grid { ColumnSpacing = 8, Margin = new Thickness(0, 4, 0, 0) };
        actions.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        actions.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        actions.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var watch = new Button { Content = T.PlayerMarkWatchEdges, MinHeight = 32, CornerRadius = new CornerRadius(4), VerticalAlignment = VerticalAlignment.Bottom, IsEnabled = !view.Previewing };
        watch.Click += (_, _) => Send(new PlayerCommand.PreviewSkipEdges());
        var save = new Button
        {
            Content = T.PlayerMarkSave, Style = (Style)Application.Current.Resources["AccentButtonStyle"], IsEnabled = view.CanSave,
            MinHeight = 32, MinWidth = 96, CornerRadius = new CornerRadius(4), VerticalAlignment = VerticalAlignment.Bottom,
        };
        save.Click += (_, _) => Send(new PlayerCommand.SaveSkipMark());
        Grid.SetColumn(watch, 1);
        Grid.SetColumn(save, 2);
        actions.Children.Add(notes);
        actions.Children.Add(watch);
        actions.Children.Add(save);
        body.Children.Add(actions);
    }

    UIElement Edge(string label, string name, SkipEdge edge, double at)
    {
        var row = new Grid { ColumnSpacing = 8 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(72) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.Children.Add(new TextBlock { Text = label, VerticalAlignment = VerticalAlignment.Center, Style = (Style)Application.Current.Resources["HazeBodyStyle"] });
        var time = Tabular(new TextBlock { Text = Wording.Clock(at), VerticalAlignment = VerticalAlignment.Center });
        Grid.SetColumn(time, 1);
        row.Children.Add(time);
        Button Nudge(string text, string spoken, long ms, int column)
        {
            var b = new Button { Content = text, MinHeight = 32, MinWidth = 56, CornerRadius = new CornerRadius(4) };
            AutomationProperties.SetName(b, spoken);
            b.Click += (_, _) => Send(new PlayerCommand.NudgeSkipEdge(edge, ms));
            Grid.SetColumn(b, column);
            row.Children.Add(b);
            return b;
        }
        Nudge(T.PlayerMarkBack1, T.PlayerMarkBack1Label(name), -1000, 2);
        Nudge(T.PlayerMarkForward1, T.PlayerMarkForward1Label(name), 1000, 3);
        return row;
    }

    /// <summary>The segment being marked, drawn on the seek line at half water: from the start to the playhead, then to the end.</summary>
    (double From, double To)? ProvisionalMark()
    {
        if (marking is not { Start: { } start } view)
            return null;
        return view.Step switch
        {
            SkipMarkStep.End => (start, Math.Max(start, position)),
            SkipMarkStep.Review when view.End is { } end => (Math.Min(start, end), Math.Max(start, end)),
            _ => null,
        };
    }

    // The prompts: Was that the opening, Wrong times, the found times' card, and the one-time sharing question.

    void ShowPrompt(SkipPrompt? prompt)
    {
        promptTimer?.Stop();
        var before = skipPrompt;
        skipPrompt = prompt;
        if (prompt is SkipPrompt.Share)
        {
            skipPrompt = null;
            AskToShare();
            return;
        }
        if (prompt is null)
        {
            HidePrompt();
            return;
        }
        // The found card's answer takes the question's place in the same card.
        var answer = before is SkipPrompt.FoundEdges && prompt is SkipPrompt.FoundSaved or SkipPrompt.FoundNever
            && cornerCard == promptCard && promptCard is { Visibility: Visibility.Visible };
        BuildPrompt(prompt, answer);
        UpdateCorner();
        if (promptHeld)
            return;
        // The found card waits as long as it takes while a screen reader is on.
        if (prompt is SkipPrompt.FoundEdges && DeviceWatch.ScreenReaderRunning())
            return;
        // Wrong times is up for 8 s, the found times' answers for 4 s, and Was that the opening
        // and the found times' question for 12 s (the engine's skip_prompt_hold_secs), as long as the corner's other notes.
        var seconds = prompt switch
        {
            SkipPrompt.WrongTimes => 8,
            SkipPrompt.FoundSaved or SkipPrompt.FoundNever => 4,
            _ => 12,
        };
        promptTimer ??= DispatcherQueue.GetForCurrentThread().CreateTimer();
        promptTimer.IsRepeating = false;
        promptTimer.Interval = TimeSpan.FromSeconds(seconds);
        promptTimer.Tick -= PromptTimedOut;
        promptTimer.Tick += PromptTimedOut;
        promptTimer.Start();
    }

    void PromptTimedOut(DispatcherQueueTimer s, object e)
    {
        s.Stop();
        if (skipPrompt is null)
            return;
        skipPrompt = null;
        HidePrompt();
        SendQuietly(new PlayerCommand.DismissSkipPrompt());
    }

    void HidePrompt()
    {
        promptTimer?.Stop();
        UpdateCorner();
    }

    void BuildPrompt(SkipPrompt prompt, bool answer = false)
    {
        if (promptCard is null)
        {
            promptCard = new Border
            {
                Visibility = Visibility.Collapsed, HorizontalAlignment = HorizontalAlignment.Right, VerticalAlignment = VerticalAlignment.Bottom,
                Background = (Brush)Application.Current.Resources["RaisedBrush"], CornerRadius = new CornerRadius(8), Padding = new Thickness(16, 8, 8, 8),
            };
            AutomationProperties.SetLiveSetting(promptCard, AutomationLiveSetting.Polite);
            Corner.Children.Add(promptCard);
        }
        string line;
        string? detail = null;
        var buttons = new List<(string Text, Action Act, PromptButton Look)>();
        var found = false;
        switch (prompt)
        {
            case SkipPrompt.WasOpening:
                line = T.PlayerWasOpening;
                buttons.Add((T.PlayerDontAskAgain, () => Send(new PlayerCommand.NeverAskWasOpening()), PromptButton.Plain));
                buttons.Add((T.PlayerMarkIt, () => BeginMarking(SkipKind.Opening), PromptButton.Accent));
                break;
            case SkipPrompt.WrongTimes wrong:
                line = T.PlayerSkipWrong(KindInline(wrong.Kind));
                var kind = wrong.Kind;
                buttons.Add((T.PlayerSkipReport, () => Send(new PlayerCommand.ReportSkip(kind)), PromptButton.Plain));
                buttons.Add((T.PlayerSkipMark, () => BeginMarking(kind), PromptButton.Accent));
                break;
            case SkipPrompt.FoundEdges edges:
                found = true;
                line = edges.Kind == SkipKind.Ending ? T.PlayerFoundAskEnding : T.PlayerFoundAskOpening;
                detail = T.PlayerFoundAskShare(edges.Episode);
                var foundKind = edges.Kind;
                buttons.Add((T.PlayerDontAskAgain, () => Send(new PlayerCommand.NeverAskWasOpening()), PromptButton.Quiet));
                buttons.Add((T.PlayerFoundAdjust, () => Send(new PlayerCommand.AdjustFoundSkip(foundKind)), PromptButton.Plain));
                buttons.Add((T.PlayerFoundYes, () => Send(new PlayerCommand.ConfirmFoundSkip(foundKind)), PromptButton.Accent));
                break;
            case SkipPrompt.FoundSaved saved:
                found = true;
                line = T.PlayerFoundSaved(saved.Episode);
                break;
            case SkipPrompt.FoundNever:
                found = true;
                line = T.PlayerFoundNever;
                detail = T.PlayerFoundNeverHow;
                break;
            default:
                return;
        }
        // The found times' card is two rows, the words and then the buttons, since three buttons and the question do not fit one.
        var layer = new StackPanel { Spacing = found ? 8 : 0 };
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, HorizontalAlignment = found ? HorizontalAlignment.Right : HorizontalAlignment.Stretch };
        var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center, Padding = new Thickness(0, found ? 4 : 0, 8, 0) };
        words.Children.Add(new TextBlock
        {
            Text = line, TextWrapping = TextWrapping.WrapWholeWords, VerticalAlignment = VerticalAlignment.Center,
            FontWeight = prompt is SkipPrompt.FoundEdges ? Microsoft.UI.Text.FontWeights.SemiBold : Microsoft.UI.Text.FontWeights.Normal,
        });
        if (detail is not null)
            words.Children.Add(new TextBlock { Text = detail, Style = (Style)Application.Current.Resources["HazeCaptionStyle"], TextWrapping = TextWrapping.WrapWholeWords, TextTrimming = TextTrimming.None });
        if (found)
        {
            layer.Width = 356;
            layer.Children.Add(words);
        }
        else
            row.Children.Add(words);
        // The found times' card is answered as it stands: the engine reads which card it was.
        var answered = prompt is SkipPrompt.FoundEdges;
        foreach (var (text, act, look) in buttons)
        {
            var b = new Button { Content = text, MinHeight = 32 };
            if (look == PromptButton.Accent)
                b.Style = (Style)Application.Current.Resources["AccentButtonStyle"];
            else if (look == PromptButton.Quiet)
            {
                b.Style = (Style)Application.Current.Resources["SubtleButtonStyle"];
                b.Foreground = (Brush)Application.Current.Resources["HazeBrush"];
            }
            if (found && look == PromptButton.Accent)
                b.MinWidth = 64;
            b.Click += (_, _) =>
            {
                if (answered)
                {
                    // Yes and Don't ask again are answered by the card's next words; Adjust closes it for the review.
                    foreach (var other in row.Children.OfType<Button>())
                        other.IsEnabled = false;
                    if (text == T.PlayerFoundAdjust)
                    {
                        skipPrompt = null;
                        HidePrompt();
                    }
                    act();
                    return;
                }
                skipPrompt = null;
                HidePrompt();
                Send(new PlayerCommand.DismissSkipPrompt());
                act();
            };
            row.Children.Add(b);
        }
        if (found)
        {
            if (row.Children.Count > 0)
                layer.Children.Add(row);
        }
        else
            layer = row;
        UIElement shown = layer;
        if (answer && promptCard.Child is UIElement old)
        {
            // The question and its answer overlap while the card follows their height: never an empty card.
            var from = old.RenderSize.Height;
            promptCard.Child = null;
            layer.Measure(new global::Windows.Foundation.Size(380, double.PositiveInfinity));
            var to = layer.DesiredSize.Height;
            var both = new Grid();
            both.Children.Add(old);
            both.Children.Add(layer);
            layer.VerticalAlignment = VerticalAlignment.Top;
            if (old is FrameworkElement oldFrame)
                oldFrame.VerticalAlignment = VerticalAlignment.Top;
            promptCard.Child = both;
            Motion.Fade(old, 0);
            Motion.FadeIn(layer, Motion.Standard);
            Motion.Height(both, from, to);
            var swap = DispatcherQueue.GetForCurrentThread().CreateTimer();
            swap.Interval = Motion.Standard + TimeSpan.FromMilliseconds(80);
            swap.IsRepeating = false;
            swap.Tick += (t, _) =>
            {
                t.Stop();
                if (promptCard.Child == both)
                {
                    both.Children.Clear();
                    both.Height = double.NaN;
                    layer.VerticalAlignment = VerticalAlignment.Stretch;
                    promptCard.Child = layer;
                }
            };
            swap.Start();
            return;
        }
        promptCard.Child = shown;
    }

    enum PromptButton { Plain, Accent, Quiet }

    /// <summary>The first Save asks once: share the times with AniSkip, or keep them on the viewer's devices.</summary>
    async void AskToShare()
    {
        if (sharing || XamlRoot is not { } root)
            return;
        sharing = true;
        try
        {
            var text = new StackPanel { Spacing = 10, MaxWidth = 420 };
            text.Children.Add(new TextBlock { Text = T.PlayerShareBody, TextWrapping = TextWrapping.WrapWholeWords });
            text.Children.Add(new TextBlock
            {
                Text = T.PlayerShareSent, TextWrapping = TextWrapping.WrapWholeWords, Style = (Style)Application.Current.Resources["HazeCaptionStyle"],
            });
            var dialog = new ContentDialog
            {
                XamlRoot = root, RequestedTheme = ElementTheme.Dark, Title = T.PlayerShareTitle, Content = text,
                PrimaryButtonText = T.PlayerShareYes, CloseButtonText = T.PlayerShareNo, DefaultButton = ContentDialogButton.Primary,
            };
            var answer = await dialog.ShowAsync();
            Send(new PlayerCommand.AnswerShareSkipTimes(answer == ContentDialogResult.Primary));
        }
        finally
        {
            sharing = false;
        }
    }

    // The Skip times panel: where each time came from, and what can be done to it.

    MenuFlyoutItem? skipTimesItem;

    /// <summary>The player menu's two skip entries: Mark skip times, and the panel of this episode's times.</summary>
    void AddSkipMenu(MenuFlyout menu)
    {
        foreach (var old in menu.Items.Where(i => i.Tag as string == "skips").ToList())
            menu.Items.Remove(old);
        var inParty = ctx?.Known.Party is not null;
        var mark = new MenuFlyoutItem
        {
            Text = T.PlayerMarkSkipTimes, Tag = "skips", KeyboardAcceleratorTextOverride = "Alt+K",
            IsEnabled = !inParty && duration > 0,
        };
        mark.Click += (_, _) => BeginMarking(null);
        var times = new MenuFlyoutItem { Text = T.PlayerSkipTimes, Tag = "skips" };
        times.Click += (_, _) => ShowSkipTimes();
        menu.Items.Insert(0, mark);
        menu.Items.Insert(1, times);
        menu.Items.Insert(2, new MenuFlyoutSeparator { Tag = "skips" });
        if (inParty)
            ToolTipService.SetToolTip(mark, T.PlayerMarkUnavailable);
    }

    async void ShowSkipTimes()
    {
        if (XamlRoot is not { } root)
            return;
        var list = new StackPanel { Spacing = 10, MinWidth = 360 };
        ContentDialog? dialog = null;
        foreach (var kind in new[] { SkipKind.Opening, SkipKind.Ending, SkipKind.Recap })
        {
            var mark = timelineMarks.FirstOrDefault(m => m.Kind == kind);
            var found = timelineMarks.Any(m => m.Kind == kind);
            var row = new Grid { ColumnSpacing = 8 };
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var words = new StackPanel { Spacing = 0 };
            words.Children.Add(new TextBlock { Text = KindName(kind), Style = (Style)Application.Current.Resources["BodyStrongTextBlockStyle"] });
            var origin = !found ? T.PlayerSkipNone : mark.Origin switch
            {
                SkipOrigin.Yours => T.PlayerSkipYours,
                SkipOrigin.AniSkip => T.PlayerSkipFromAniskip,
                SkipOrigin.Found => T.PlayerSkipFound,
                _ => T.PlayerSkipFromFile,
            };
            // Why nothing was found, when finding is paused: openings and endings only.
            if (!found && kind != SkipKind.Recap && ctx?.Known.SkipFindPaused is { } pause
                && Pages.SettingsPage.FindSkipsPause(pause) is { } why && pause != SkipFindPause.Unavailable)
                origin = $"{origin} · {why}";
            words.Children.Add(Tabular(new TextBlock
            {
                Text = found ? $"{Wording.Clock(mark.Start)}\u2013{Wording.Clock(mark.End)} · {origin}" : origin,
                Style = (Style)Application.Current.Resources["HazeCaptionStyle"],
            }));
            row.Children.Add(words);
            var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 4 };
            var picked = kind;
            Button Action(string text, Action act)
            {
                var b = new Button { Content = text, MinHeight = 32 };
                b.Click += (_, _) =>
                {
                    dialog?.Hide();
                    act();
                };
                actions.Children.Add(b);
                return b;
            }
            if (found && mark.Origin == SkipOrigin.Found)
                Action(T.PlayerFoundAdjust, () => Send(new PlayerCommand.AdjustFoundSkip(picked)));
            else
                Action(T.PlayerSkipMark, () => BeginMarking(picked));
            if (found && mark.Origin == SkipOrigin.AniSkip)
                Action(T.PlayerSkipReport, () => Send(new PlayerCommand.ReportSkip(picked)));
            if (found && mark.Origin == SkipOrigin.Yours)
                Action(T.PlayerSkipClear, () => Send(new PlayerCommand.ClearMySkip(picked)));
            Grid.SetColumn(actions, 1);
            row.Children.Add(actions);
            list.Children.Add(row);
        }
        dialog = new ContentDialog
        {
            XamlRoot = root, RequestedTheme = ElementTheme.Dark, Title = T.PlayerSkipTimes, Content = list,
            CloseButtonText = T.ActionClose,
        };
        await dialog.ShowAsync();
    }
}
