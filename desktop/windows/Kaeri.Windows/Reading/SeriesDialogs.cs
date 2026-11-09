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
/// The series page's dialogs: Read from (every source with what it holds, and a source searched
/// by hand), Tracking (what each tracker holds, a tracker behind, the ones that can't track it
/// last, and MangaUpdates' sign-in), Remove from lists and Read again. Each listens to the store
/// while it is up and draws again in place. The twin of the Apple app's SeriesSheets.
/// </summary>
static class SeriesDialogs
{
    static ContentDialog Dialog(XamlRoot root, string title, UIElement body, string close) => new()
    {
        XamlRoot = root,
        Title = title,
        Content = body,
        CloseButtonText = close,
        DefaultButton = ContentDialogButton.Close,
    };

    // Read from

    /// <summary>
    /// Read from: every source that has the series, the one in use ticked, each with what it
    /// holds. Choosing one keeps the read marks. Sources that did not find it follow, each with
    /// Search there for a series under other words.
    /// </summary>
    public static async Task ReadFrom(Ctx ctx, XamlRoot root, string work)
    {
        var books = ctx.Known.Books;
        var panel = new StackPanel { Spacing = 12 };
        var editions = new StackPanel { Spacing = 0 };
        var misses = new StackPanel { Spacing = 0 };
        panel.Children.Add(editions);
        panel.Children.Add(Ui.Text(T.SeriesReadFromNote, "HazeCaptionStyle"));
        panel.Children.Add(misses);
        var dialog = Dialog(root, T.SeriesReadFromTitle, SeriesWords.Scroll(panel), T.ActionDone);
        // The search box for each source, kept across redraws so what is typed stays.
        var boxes = new Dictionary<string, TextBox>();
        string? open = null, drawnEditions = null, drawnMisses = null;

        void Draw()
        {
            var forWork = books.ForWork.GetValueOrDefault(work);
            var list = forWork?.Editions ?? [];
            var search = books.SourceSeries.GetValueOrDefault(work);
            var printEditions = Drawn.Fingerprint((list, forWork?.Asking, books.View?.Servers.Select(s => (s.Id, s.Kind)).ToList()));
            if (printEditions != drawnEditions)
            {
                drawnEditions = printEditions;
                var focus = Ui.MarkFocus(editions);
                editions.Children.Clear();
                if (list.Length == 0 && forWork is not { Asking: false })
                {
                    var asking = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10, Margin = new Thickness(0, 8, 0, 8) };
                    asking.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
                    asking.Children.Add(Ui.Haze(T.SeriesReadFromAsking));
                    editions.Children.Add(asking);
                }
                else if (list.Length == 0)
                {
                    editions.Children.Add(Ui.Text(T.SeriesReadFromNoneTitle, "BodyStrongTextBlockStyle"));
                    editions.Children.Add(Ui.Haze(T.SeriesReadFromNoneLine));
                }
                else
                {
                    var view = new ListView { SelectionMode = ListViewSelectionMode.Single, IsItemClickEnabled = true };
                    AutomationProperties.SetName(view, T.SeriesReadFromTitle);
                    foreach (var edition in list)
                    {
                        var line = EditionLine(ctx, edition);
                        var words = SeriesWords.Pair(edition.SourceName, line);
                        words.Tag = edition;
                        var item = new ListViewItem { Content = words, Tag = edition, Padding = new Thickness(12, 8, 12, 8) };
                        AutomationProperties.SetName(item, $"{edition.SourceName}, {line}");
                        view.Items.Add(item);
                        if (edition.InUse)
                            view.SelectedItem = item;
                    }
                    view.ItemClick += (_, e) =>
                    {
                        // The clicked item is the row's content, which carries its edition too.
                        if (e.ClickedItem is FrameworkElement { Tag: BookEditionRow { InUse: false } chosen })
                        {
                            ctx.Known.Reading.ChooseEdition(work, chosen);
                            dialog.Hide();
                        }
                    };
                    editions.Children.Add(view);
                    if (forWork is { Asking: true })
                    {
                        var still = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10, Margin = new Thickness(12, 4, 0, 0) };
                        still.Children.Add(new ProgressRing { IsActive = true, Width = 14, Height = 14 });
                        still.Children.Add(Ui.Text(T.SeriesReadFromAsking, "HazeCaptionStyle"));
                        editions.Children.Add(still);
                    }
                }
                Ui.RestoreFocus(editions, focus);
            }

            var missed = forWork?.Missing ?? [];
            var printMisses = Drawn.Fingerprint((missed, search, open));
            if (printMisses == drawnMisses)
                return;
            drawnMisses = printMisses;
            var keep = Ui.MarkFocus(misses);
            foreach (var box in boxes.Values)
                (box.Parent as Panel)?.Children.Remove(box);
            misses.Children.Clear();
            if (missed.Length == 0)
                return;
            var heading = Ui.Heading(T.ReadFromNotFound);
            AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
            misses.Children.Add(heading);
            foreach (var miss in missed)
            {
                misses.Children.Add(SeriesWords.Rule());
                var row = new Grid { ColumnSpacing = 12, Padding = new Thickness(0, 8, 0, 8) };
                row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
                row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
                row.Children.Add(SeriesWords.Pair(miss.SourceName, miss.Reason.Text()));
                if (open != miss.SourceId)
                {
                    var there = Ui.Label(T.ReadFromSearchThere, (_, _) =>
                    {
                        open = miss.SourceId;
                        Draw();
                        if (boxes.TryGetValue(miss.SourceId, out var box))
                            box.Focus(FocusState.Programmatic);
                    });
                    AutomationProperties.SetName(there, $"{T.ReadFromSearchThere}, {miss.SourceName}");
                    there.VerticalAlignment = VerticalAlignment.Center;
                    Grid.SetColumn(there, 1);
                    row.Children.Add(there);
                }
                misses.Children.Add(row);
                if (open != miss.SourceId)
                    continue;
                if (!boxes.TryGetValue(miss.SourceId, out var field))
                {
                    field = new TextBox { PlaceholderText = T.ReadFromSearchPlaceholder(miss.SourceName) };
                    AutomationProperties.SetName(field, T.ReadFromSearchPlaceholder(miss.SourceName));
                    var id = miss.SourceId;
                    field.KeyDown += (_, e) =>
                    {
                        if (e.Key == global::Windows.System.VirtualKey.Enter && field.Text.Trim().Length > 0)
                        {
                            e.Handled = true;
                            books.SearchSource(work, id, field.Text.Trim());
                        }
                    };
                    boxes[miss.SourceId] = field;
                }
                var query = new Grid { ColumnSpacing = 8, Margin = new Thickness(0, 0, 0, 8) };
                query.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
                query.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
                query.Children.Add(field);
                var go = Ui.Label(T.ReadFromSearch, (_, _) =>
                {
                    if (field.Text.Trim().Length > 0)
                        books.SearchSource(work, miss.SourceId, field.Text.Trim());
                });
                Grid.SetColumn(go, 1);
                query.Children.Add(go);
                misses.Children.Add(query);
                if (search is { } found && found.SourceId == miss.SourceId)
                    misses.Children.Add(Choices(ctx, work, found));
            }
            Ui.RestoreFocus(misses, keep);
        }

        void Changed() => Draw();
        books.Changed += Changed;
        books.LoadForWork(work);
        if (books.View is null)
            books.Load();
        Draw();
        await Dialogs.Show(dialog);
        books.Changed -= Changed;
        books.ClearSourceSearch(work);
    }

    /// <summary>What a search in one source found: each series to pick, or that nothing matched.</summary>
    static UIElement Choices(Ctx ctx, string work, SourceSeriesSearch found)
    {
        var list = new StackPanel { Spacing = 0, Margin = new Thickness(0, 0, 0, 8) };
        if (found.Searching)
        {
            var wait = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10 };
            wait.Children.Add(new ProgressRing { IsActive = true, Width = 14, Height = 14 });
            AutomationProperties.SetName(wait.Children[0], T.StateLoading);
            list.Children.Add(wait);
            return list;
        }
        if (found.Failure is { } failure)
        {
            list.Children.Add(Ui.Haze(failure.Words()));
            return list;
        }
        if (found.Choices.Length == 0)
        {
            list.Children.Add(Ui.Haze(T.ReadFromSearchNone));
            return list;
        }
        foreach (var choice in found.Choices)
        {
            var parts = new List<string>();
            if (choice.Year is { } year)
                parts.Add(year.ToString(System.Globalization.CultureInfo.CurrentCulture));
            if (choice.Language is { Length: > 0 } language)
                parts.Add(Wording.LanguageName(language));
            if (choice.Confidence > 0)
                parts.Add(T.LibraryMatchPercent(choice.Confidence));
            var line = string.Join(" · ", parts);
            var button = new Button
            {
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Stretch,
                Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
                BorderThickness = new Thickness(0),
                Padding = new Thickness(12, 6, 12, 6),
                Content = SeriesWords.Pair(choice.Title, line),
            };
            AutomationProperties.SetName(button, line.Length > 0 ? $"{choice.Title}, {line}" : choice.Title);
            button.Click += (_, _) => ctx.Known.Books.ChooseSourceSeries(work, found.SourceId, choice.SeriesRef);
            list.Children.Add(button);
        }
        return list;
    }

    /// <summary>"Kavita · Group · English · chapters 1 to 88", and "on this PC" for a folder.</summary>
    static string EditionLine(Ctx ctx, BookEditionRow edition)
    {
        var view = ctx.Known.Books.View;
        var lead = new List<string>();
        var folder = false;
        if (view?.Servers.FirstOrDefault(s => s.Id == edition.SourceId) is { } server)
            lead.Add(server.Kind switch { BookServerKind.Komga => "Komga", BookServerKind.Kavita => "Kavita", _ => T.BookSourcesOpds });
        else if (view?.Plugins.FirstOrDefault(p => p.Id == edition.SourceId) is { Store: { Length: > 0 } from })
            lead.Add(T.BookSourcesFromStore(from));
        else if (view?.Folders.Any(f => f.Id == edition.SourceId) == true)
            folder = true;
        if (edition.VersionLabel is { Length: > 0 } label)
            lead.Add(label);
        if (edition.Language is { Length: > 0 } language)
            lead.Add(Wording.LanguageName(language));
        var latest = edition.Latest ?? edition.Chapters.ToString(System.Globalization.CultureInfo.CurrentCulture);
        var parts = new List<string>(lead)
        {
            lead.Count == 0 ? T.SeriesReadFromChapters(latest) : T.SeriesReadFromChaptersLater(latest),
        };
        if (folder)
            parts.Add(T.SeriesReadFromOnDevice(SyncWords.Kind));
        return string.Join(" · ", parts);
    }

    // Tracking

    /// <summary>
    /// Tracking: each tracker that follows the series with what it holds and its score; a tracker
    /// behind says when it catches up, and one that disagrees offers whose place to keep. A
    /// tracker unsure of the series lists its guesses. Trackers that can't track it come last;
    /// MangaUpdates signs in right here, with its credit.
    /// </summary>
    public static async Task Tracking(Ctx ctx, XamlRoot root, string work)
    {
        var reading = ctx.Known.Reading;
        var panel = new StackPanel { Spacing = 0 };
        var dialog = Dialog(root, T.SeriesTracking, SeriesWords.Scroll(panel), T.ActionDone);
        var user = new TextBox { Header = T.AccountsUsername };
        var password = new PasswordBox { Header = T.AccountsPassword };
        var mangaUpdates = SeriesWords.Tracker("mangaupdates");
        AutomationProperties.SetName(user, T.AccountsUsername);
        AutomationProperties.SetName(password, T.AccountsPassword);
        var signingIn = false;
        string? drawn = null;

        void Draw()
        {
            if (reading.Series.GetValueOrDefault(work) is not { } series)
                return;
            var rows = reading.Tracking.GetValueOrDefault(work) ?? series.Tracking;
            var print = Drawn.Fingerprint((rows, signingIn));
            if (print == drawn)
                return;
            drawn = print;
            var focus = Ui.MarkFocus(panel);
            (user.Parent as Panel)?.Children.Remove(user);
            (password.Parent as Panel)?.Children.Remove(password);
            panel.Children.Clear();
            if (!string.IsNullOrEmpty(series.Titles.Display))
                panel.Children.Add(Ui.Text(series.Titles.Display, "HazeBodyStyle"));
            var tracked = rows.Where(r => r.Mapping is not TrackerMapping.Unmapped).ToList();
            var away = SeriesWords.ReadingTrackers.Where(id => rows.All(r => r.Tracker != id)).ToList();
            var unmapped = rows.Where(r => r.Mapping is TrackerMapping.Unmapped).ToList();
            var newest = tracked.Where(r => !r.Divergent).Select(r => r.Chapters).DefaultIfEmpty(tracked.Select(r => r.Chapters).DefaultIfEmpty(0u).Max()).Max();
            foreach (var row in tracked)
            {
                panel.Children.Add(SeriesWords.Rule());
                panel.Children.Add(TrackerRow(ctx, work, row, series, newest));
            }
            if (tracked.Select(r => r.SplitNote).FirstOrDefault(n => n is not null) is { } note)
            {
                var said = Ui.Text(note.Text(), "HazeCaptionStyle");
                said.Margin = new Thickness(0, 8, 0, 0);
                panel.Children.Add(said);
            }
            if (away.Count > 0 || unmapped.Count > 0)
            {
                var heading = Ui.Heading(T.SeriesNotTracked);
                AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
                panel.Children.Add(heading);
                foreach (var row in unmapped)
                {
                    panel.Children.Add(SeriesWords.Rule());
                    var plain = SeriesWords.Pair(SeriesWords.Tracker(row.Tracker), T.SeriesTrackerNotThere);
                    plain.Padding = new Thickness(0, 8, 0, 8);
                    plain.Opacity = 0.7;
                    panel.Children.Add(plain);
                }
                foreach (var tracker in away)
                {
                    panel.Children.Add(SeriesWords.Rule());
                    var row = new Grid { ColumnSpacing = 12, Padding = new Thickness(0, 8, 0, 8) };
                    row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
                    row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
                    var words = SeriesWords.Pair(SeriesWords.Tracker(tracker), T.SeriesTrackerConnect);
                    words.Opacity = 0.7;
                    row.Children.Add(words);
                    if (tracker == "mangaupdates")
                    {
                        // MangaUpdates takes a user name and password, so it signs in here.
                        if (!signingIn)
                        {
                            var connect = Ui.Label(T.AccountsSignInTo(mangaUpdates), (_, _) =>
                            {
                                signingIn = true;
                                Draw();
                                user.Focus(FocusState.Programmatic);
                            });
                            connect.VerticalAlignment = VerticalAlignment.Center;
                            Grid.SetColumn(connect, 1);
                            row.Children.Add(connect);
                        }
                        panel.Children.Add(row);
                        if (signingIn)
                            panel.Children.Add(SignInForm(user, password, mangaUpdates, Submit, () =>
                            {
                                password.Password = "";
                                signingIn = false;
                                Draw();
                            }));
                        continue;
                    }
                    var settings = new HyperlinkButton { Content = T.SettingsTitle, Style = Ui.Res<Style>("SilkLinkStyle"), VerticalAlignment = VerticalAlignment.Center };
                    AutomationProperties.SetName(settings, $"{T.SettingsTitle}, {SeriesWords.Tracker(tracker)}");
                    settings.Click += (_, _) =>
                    {
                        dialog.Hide();
                        ctx.Window.OpenSettings("accounts");
                    };
                    Grid.SetColumn(settings, 1);
                    row.Children.Add(settings);
                    panel.Children.Add(row);
                }
            }
            Ui.RestoreFocus(panel, focus);
        }

        // The password goes to the engine once and is cleared from the box; it is never logged.
        void Submit()
        {
            if (user.Text.Trim().Length == 0 || password.Password.Length == 0)
                return;
            reading.ConnectMangaUpdates(user.Text.Trim(), password.Password);
            password.Password = "";
            signingIn = false;
            reading.LoadTracking(work);
            Draw();
        }
        password.KeyDown += (_, e) =>
        {
            if (e.Key == global::Windows.System.VirtualKey.Enter)
            {
                e.Handled = true;
                Submit();
            }
        };

        void Changed(ReadingPart part)
        {
            if (part is ReadingPart.Tracking or ReadingPart.Series)
                Draw();
        }
        reading.Changed += Changed;
        reading.LoadTracking(work);
        Draw();
        await Dialogs.Show(dialog);
        reading.Changed -= Changed;
        password.Password = "";
    }

    /// <summary>MangaUpdates' sign-in: user name, password, where the password goes, and its credit.</summary>
    static StackPanel SignInForm(TextBox user, PasswordBox password, string name, Action submit, Action cancel)
    {
        var form = new StackPanel { Spacing = 8, Margin = new Thickness(0, 0, 0, 12) };
        form.Children.Add(user);
        form.Children.Add(password);
        form.Children.Add(Ui.Text(T.AccountsFormDetail(name), "HazeCaptionStyle"));
        form.Children.Add(Ui.Text(T.AccountsMangaupdatesCredit, "HazeCaptionStyle"));
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        buttons.Children.Add(Ui.Label(T.AccountsSignInTo(name), (_, _) => submit(), accent: true));
        buttons.Children.Add(Ui.Label(T.ActionCancel, (_, _) => cancel()));
        form.Children.Add(buttons);
        return form;
    }

    /// <summary>One tracker: what it holds, its score, and when a tracker behind catches up; its guesses when unsure.</summary>
    static StackPanel TrackerRow(Ctx ctx, string work, TrackerReadRow row, SeriesView series, uint newest)
    {
        var reading = ctx.Known.Reading;
        var block = new StackPanel { Spacing = 4, Padding = new Thickness(0, 8, 0, 8) };
        var top = new Grid { ColumnSpacing = 12 };
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        string line;
        if (row.Mapping is TrackerMapping.Looking)
            line = T.SeriesTrackerLooking;
        else if (row.Mapping is TrackerMapping.Ask)
            line = T.SeriesTrackerAsk;
        else if (row.Divergent)
            line = row.Chapters < newest ? T.SeriesTrackerBehind(row.Chapters, newest) : T.SeriesTrackerAhead(row.Chapters);
        else
        {
            var parts = new List<string>();
            if (row.Status is { } status)
                parts.Add(SeriesWords.List(status));
            parts.Add(SeriesWords.Progress(row, series));
            line = string.Join(" · ", parts);
        }
        var name = SeriesWords.Tracker(row.Tracker);
        top.Children.Add(SeriesWords.Pair(name, line));
        if (row.Divergent)
        {
            var menu = new MenuFlyout();
            menu.Items.Add(SeriesWords.Item(T.SeriesTrackerKeepKaeri, () => reading.ResolveConflict(work, row.Tracker, true)));
            menu.Items.Add(SeriesWords.Item(T.SeriesTrackerTakeTheirs(name), () => reading.ResolveConflict(work, row.Tracker, false)));
            var more = new DropDownButton { Content = new FontIcon { Glyph = "", FontSize = 14 }, Flyout = menu, VerticalAlignment = VerticalAlignment.Center };
            AutomationProperties.SetName(more, $"{T.ActionMore}, {name}");
            Grid.SetColumn(more, 1);
            top.Children.Add(more);
        }
        else if (row.Score is float score && score > 0)
        {
            var said = Ui.Text(T.SeriesTrackerScore(score.ToString("0.#", System.Globalization.CultureInfo.CurrentCulture)), "HazeBodyStyle", wrap: false);
            said.VerticalAlignment = VerticalAlignment.Center;
            Grid.SetColumn(said, 1);
            top.Children.Add(said);
        }
        block.Children.Add(top);
        if (row.Mapping is TrackerMapping.Ask ask)
        {
            foreach (var candidate in ask.Candidates)
            {
                var parts = new List<string>();
                if (candidate.Year is { } year)
                    parts.Add(year.ToString(System.Globalization.CultureInfo.InvariantCulture));
                parts.Add(T.LibraryMatchPercent(candidate.Confidence));
                var pick = new Button
                {
                    HorizontalAlignment = HorizontalAlignment.Stretch,
                    HorizontalContentAlignment = HorizontalAlignment.Stretch,
                    Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
                    BorderThickness = new Thickness(0),
                    Padding = new Thickness(12, 6, 12, 6),
                    Content = SeriesWords.Pair(candidate.Title, string.Join(" · ", parts)),
                };
                AutomationProperties.SetName(pick, $"{candidate.Title}, {string.Join(", ", parts)}");
                var id = candidate.TrackerId;
                pick.Click += (_, _) => reading.ChooseMapping(work, row.Tracker, id);
                block.Children.Add(pick);
            }
            var none = new HyperlinkButton { Content = T.SeriesTrackerNoneOfThese, Style = Ui.Res<Style>("SilkLinkStyle") };
            none.Click += (_, _) => reading.ChooseMapping(work, row.Tracker, null);
            block.Children.Add(none);
        }
        return block;
    }

    // Remove from lists

    /// <summary>Asked first, as a destructive confirmation.</summary>
    public static Task<bool> AskRemove(XamlRoot root, SeriesView series) =>
        Ui.Confirm(root, T.SeriesRemoveFromListsTitle(series.Titles.Display), T.SeriesRemoveFromListsLine, T.ActionRemove);

    /// <summary>What each tracker made of Remove from lists, a line each.</summary>
    public static async Task Removed(XamlRoot root, ListRemoval[] results)
    {
        var lines = string.Join("\n", results.Select(r => r.Line.Joined()).Where(l => l.Length > 0));
        var dialog = Dialog(root, T.SeriesRemovedTitle, Ui.Text(lines, "BodyTextBlockStyle"), T.ActionDone);
        await Dialogs.Show(dialog);
    }

    // Read again

    /// <summary>Read again from the start: asked first, then said with Undo.</summary>
    public static async Task ReadAgain(Ctx ctx, XamlRoot root, SeriesView series)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.ReadAgainTitle(series.Titles.Display),
            Content = Ui.Text(T.ReadAgainLine, "BodyTextBlockStyle"),
            PrimaryButtonText = T.ReadAgainConfirm,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
        };
        if (await Dialogs.Show(dialog) != ContentDialogResult.Primary)
            return;
        var work = series.Work;
        ctx.Known.Reading.ReadAgain(work);
        ctx.Window.Say(T.SeriesReadAgainDoneWindows(series.Titles.Display), T.ActionUndo, () => ctx.Known.Reading.UndoReadAgain(work));
    }
}
