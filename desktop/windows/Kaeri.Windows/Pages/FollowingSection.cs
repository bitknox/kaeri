using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// People you follow, at the end of the title page's details column: the heading, "From
/// AniList", five people (picture, name, line, score at the end), then Show all 10, which grows
/// the list in place. While AniList is slow to answer, two placeholder rows from 600 ms, held at
/// least 0.3 s, then one crossfade to the people. Nobody followed has the show: no section.
/// The page keeps one and puts its <see cref="Root"/> back each time it lays the column.
/// </summary>
sealed class FollowingSection
{
    const double Avatar = 32, RowHeight = 46, ButtonArea = 40;

    readonly Ctx ctx;
    readonly DispatcherQueueTimer wait;

    /// <summary>The section's place in the column: the placeholders and the people in one cell, for the crossfade.</summary>
    public Grid Root { get; } = new() { Margin = new Thickness(0, 4, 0, 0), Visibility = Visibility.Collapsed };

    readonly StackPanel placeholders = new() { Visibility = Visibility.Collapsed };
    readonly StackPanel section = new() { Visibility = Visibility.Collapsed };

    enum Phase { None, Waiting, Placeholders, Section }

    Phase phase;
    FollowedEntry[] shown = [];
    bool expanded;
    /// <summary>Counts each reset and each change, so a held step that was overtaken does nothing.</summary>
    int turn;
    DateTime placeholdersAt;

    public FollowingSection(Ctx ctx, DispatcherQueue queue)
    {
        this.ctx = ctx;
        Root.Children.Add(placeholders);
        Root.Children.Add(section);
        wait = queue.CreateTimer();
        wait.IsRepeating = false;
        wait.Interval = TimeSpan.FromMilliseconds(600);
        wait.Tick += (_, _) => ShowPlaceholders();
    }

    /// <summary>Another title: whatever stood goes at once, and the list is folded again.</summary>
    public void Reset()
    {
        turn++;
        wait.Stop();
        phase = Phase.None;
        shown = [];
        expanded = false;
        Motion.Set(placeholders, false);
        Motion.Set(section, false);
        placeholders.Children.Clear();
        section.Children.Clear();
        Root.Visibility = Visibility.Collapsed;
    }

    /// <summary>
    /// What the page now has: the people when AniList answered with some, the placeholders while
    /// it has not answered and the page asks, and nothing otherwise.
    /// </summary>
    public void Update(Entry? entry)
    {
        var people = entry?.Detail?.Following ?? [];
        var answered = entry?.Detail is { Partial: false };
        if (people.Length > 0)
            Present(people);
        else if (!answered && Following.Live(ctx.Known))
            Wait();
        else
            Leave();
    }

    void Wait()
    {
        if (phase != Phase.None)
            return;
        phase = Phase.Waiting;
        wait.Start();
    }

    void ShowPlaceholders()
    {
        if (phase != Phase.Waiting)
            return;
        phase = Phase.Placeholders;
        placeholdersAt = DateTime.UtcNow;
        DrawPlaceholders();
        Root.Visibility = Visibility.Visible;
        Motion.FadeIn(placeholders);
    }

    void Present(FollowedEntry[] people)
    {
        wait.Stop();
        switch (phase)
        {
            case Phase.Section when Following.Same(people, shown):
                return;
            case Phase.Section:
                // Someone's progress changed while the page was open: the list says it in place.
                Crossfade.Swap(Root, () => Fill(people));
                return;
            case Phase.Placeholders:
            {
                phase = Phase.Section;
                var mine = ++turn;
                var held = TimeSpan.FromMilliseconds(300) - (DateTime.UtcNow - placeholdersAt);
                void Swap()
                {
                    if (mine != turn)
                        return;
                    Fill(shown);
                    section.Opacity = 0;
                    section.Visibility = Visibility.Visible;
                    Motion.Crossfade(placeholders, section);
                    After(Motion.Standard, () =>
                    {
                        if (mine == turn)
                            Motion.Set(placeholders, false);
                    });
                }
                shown = people;
                if (held > TimeSpan.Zero)
                    After(held, Swap);
                else
                    Swap();
                return;
            }
            default:
                phase = Phase.Section;
                turn++;
                Fill(people);
                Root.Visibility = Visibility.Visible;
                Motion.FadeIn(section);
                return;
        }
    }

    /// <summary>Nobody followed has the show, or the switch went off: what stands fades away.</summary>
    void Leave()
    {
        wait.Stop();
        var mine = ++turn;
        var leaving = phase switch
        {
            Phase.Placeholders => placeholders,
            Phase.Section => section,
            _ => null,
        };
        phase = Phase.None;
        shown = [];
        if (leaving is null)
            return;
        Motion.FadeOutThen(leaving, () =>
        {
            if (mine != turn)
                return;
            Motion.Set(leaving, false);
            leaving.Children.Clear();
            Root.Visibility = Visibility.Collapsed;
        });
    }

    void After(TimeSpan delay, Action act)
    {
        var timer = Root.DispatcherQueue.CreateTimer();
        timer.IsRepeating = false;
        timer.Interval = delay;
        timer.Tick += (_, _) => act();
        timer.Start();
    }

    /// <summary>The section's own layout as raised plates: the heading, the note and two people.</summary>
    void DrawPlaceholders()
    {
        placeholders.Children.Clear();
        var heading = States.Plate(120, 11, 4);
        heading.Margin = new Thickness(0, 4, 0, 9);
        var note = States.Plate(72, 8, 4);
        note.Margin = new Thickness(0, 0, 0, 8);
        placeholders.Children.Add(heading);
        placeholders.Children.Add(note);
        for (var i = 0; i < 2; i++)
        {
            var row = new Grid { Height = RowHeight, ColumnSpacing = 12 };
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            var circle = States.Plate(Avatar, Avatar, Avatar / 2);
            circle.VerticalAlignment = VerticalAlignment.Center;
            row.Children.Add(circle);
            var bars = new StackPanel { Spacing = 6, VerticalAlignment = VerticalAlignment.Center };
            bars.Children.Add(States.Plate(96, 9, 4));
            bars.Children.Add(States.Plate(140, 8, 4));
            Grid.SetColumn(bars, 1);
            row.Children.Add(bars);
            placeholders.Children.Add(row);
        }
        AutomationProperties.SetAccessibilityView(placeholders, AccessibilityView.Raw);
    }

    /// <summary>The heading, the note, the first five people, and the rest behind Show all.</summary>
    void Fill(FollowedEntry[] people)
    {
        shown = people;
        pressAll = null;
        section.Children.Clear();
        var heading = new TextBlock { Text = T.MoreFollowing, FontSize = 14, LineHeight = 20, FontWeight = FontWeights.SemiBold };
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
        section.Children.Add(heading);
        section.Children.Add(new TextBlock { Text = T.MoreFollowingNote, Style = Ui.Res<Style>("HazeCaptionStyle"), LineHeight = 16, Margin = new Thickness(0, 0, 0, 6) });
        foreach (var person in people.Take(Following.Shown))
            section.Children.Add(Row(person));
        if (people.Length <= Following.Shown)
            return;

        var rest = new StackPanel();
        foreach (var person in people.Skip(Following.Shown))
            rest.Children.Add(Row(person));
        Clip(rest);
        section.Children.Add(rest);
        if (expanded)
            return;
        rest.Visibility = Visibility.Collapsed;
        var all = new Button
        {
            Content = T.FollowingShowAll(people.Length),
            MinHeight = 32,
            Height = 32,
            Margin = new Thickness(0, 8, 0, 0),
            VerticalAlignment = VerticalAlignment.Top,
            Background = Ui.Res<Brush>("RaisedBrush"),
        };
        var area = new Grid { Height = ButtonArea };
        area.Children.Add(all);
        Clip(area);
        section.Children.Add(area);
        void Press() => ShowAll(rest, area, all, people.Length - Following.Shown);
        pressAll = Press;
        all.Click += (_, _) => Press();
    }

    Action? pressAll;

#if KAERI_TESTING
    /// <summary>--show=following-all: Show all, as if clicked.</summary>
    internal void PressShowAll() => pressAll?.Invoke();
#endif

    /// <summary>
    /// Show all: the rest grow in place on the standard token as the button's room closes, the
    /// rows fading in and the button out. Nothing above moves. Focus on the button goes back to
    /// what comes before it.
    /// </summary>
    void ShowAll(StackPanel rest, Grid area, Button all, int more)
    {
        expanded = true;
        all.IsEnabled = false;
        if (all.FocusState != FocusState.Unfocused && all.XamlRoot is { } root)
            _ = Microsoft.UI.Xaml.Input.FocusManager.TryMoveFocus(Microsoft.UI.Xaml.Input.FocusNavigationDirection.Previous,
                new Microsoft.UI.Xaml.Input.FindNextElementOptions { SearchRoot = root.Content });
        var height = more * RowHeight;
        rest.Height = 0;
        rest.Visibility = Visibility.Visible;
        Motion.FadeIn(rest);
        Motion.FadeOutThen(all, () => { });
        Motion.Ease(rest, 0, 1, at =>
        {
            rest.Height = height * at;
            area.Height = ButtonArea * (1 - at);
            if (at < 1)
                return;
            rest.Height = double.NaN;
            area.Visibility = Visibility.Collapsed;
        });
    }

    /// <summary>A person: the picture in a circle, the name over the line, the score at the end. One element for a screen reader.</summary>
    PersonRow Row(FollowedEntry person)
    {
        var row = new PersonRow { Height = RowHeight, ColumnSpacing = 12 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var face = Face(ctx, person.Name, person.AvatarUrl, Avatar);
        face.VerticalAlignment = VerticalAlignment.Center;
        row.Children.Add(face);
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(new TextBlock { Text = person.Name, FontSize = 14, LineHeight = 20, TextTrimming = TextTrimming.CharacterEllipsis, TextWrapping = TextWrapping.NoWrap });
        if (Following.Line(person) is { Length: > 0 } line)
            text.Children.Add(new TextBlock { Text = line, Style = Ui.Res<Style>("HazeCaptionStyle"), LineHeight = 16, TextWrapping = TextWrapping.NoWrap });
        Grid.SetColumn(text, 1);
        row.Children.Add(text);
        if (Following.ScoreText(person) is { Length: > 0 } score)
        {
            var end = new TextBlock { Text = score, Style = Ui.Res<Style>("HazeCaptionStyle"), LineHeight = 16, VerticalAlignment = VerticalAlignment.Center };
            Grid.SetColumn(end, 2);
            row.Children.Add(end);
        }
        AutomationProperties.SetName(row, Following.Spoken(person));
        return row;
    }

    /// <summary>Keeps what grows or closes inside its own height while it eases.</summary>
    static void Clip(FrameworkElement element) =>
        element.SizeChanged += (_, e) => element.Clip = new RectangleGeometry { Rect = new global::Windows.Foundation.Rect(0, 0, e.NewSize.Width, e.NewSize.Height) };

    /// <summary>
    /// A person's picture in a circle, from the art cache. Without one, the first letter of the
    /// name on the selected raised fill. With <paramref name="ring"/>, a 2 px ring of it around
    /// the circle, so pictures in a stack stand apart.
    /// </summary>
    internal static FrameworkElement Face(Ctx ctx, string name, string? url, double size, Brush? ring = null)
    {
        var face = new Grid { Width = size, Height = size, CornerRadius = new CornerRadius(size / 2), Background = Ui.Res<Brush>("SelectedFillBrush") };
        if (url is null)
            face.Children.Add(new TextBlock
            {
                Text = Following.Initial(name),
                FontSize = Math.Round(size * 0.46),
                FontWeight = FontWeights.SemiBold,
                Foreground = Ui.Res<Brush>("SilkBrush"),
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            });
        else
        {
            var image = new Image { Stretch = Stretch.UniformToFill };
            ArtFade.SetOn(image, true);
            face.Children.Add(image);
            ctx.Art.Bind(url, s => image.Source = s);
        }
        AutomationProperties.SetAccessibilityView(face, AccessibilityView.Raw);
        if (ring is null)
            return face;
        var outer = new Border { Width = size + 4, Height = size + 4, CornerRadius = new CornerRadius(size / 2 + 2), Background = ring, Padding = new Thickness(2), Child = face };
        AutomationProperties.SetAccessibilityView(outer, AccessibilityView.Raw);
        return outer;
    }
}

/// <summary>A person's row: one text element for a screen reader, named by the page, its parts silent.</summary>
sealed partial class PersonRow : Grid
{
    protected override AutomationPeer OnCreateAutomationPeer() => new Peer(this);

    sealed class Peer(PersonRow owner) : FrameworkElementAutomationPeer(owner)
    {
        protected override AutomationControlType GetAutomationControlTypeCore() => AutomationControlType.Text;

        protected override IList<AutomationPeer> GetChildrenCore() => [];

        protected override bool IsControlElementCore() => true;

        protected override bool IsContentElementCore() => true;
    }
}

/// <summary>The marks a row shows: whose they are (the row's item), who, and whether the row keeps their slot.</summary>
public sealed record MarkSet(object Owner, FollowedMark[] Marks, bool Room);

/// <summary>
/// An episode row's marks: up to three pictures overlapping, each ringed in the ground, then
/// "+N" on the selected raised fill. Its tooltip says every name, never a score. Marks coming to
/// a row it already showed fade in (0.2 s) and going fade out (0.3 s); a row drawn anew, or a
/// recycled container taking another row, just shows its own. The slot is 80 wide while the
/// table has any marks, and nothing otherwise.
/// </summary>
public sealed partial class FollowMarks : Grid
{
    public const double Slot = 80;
    const double Size = 24;

    public static readonly DependencyProperty SetProperty = DependencyProperty.Register(
        nameof(Set), typeof(object), typeof(FollowMarks), new PropertyMetadata(null, (d, e) => ((FollowMarks)d).Take(e.OldValue as MarkSet, e.NewValue as MarkSet)));

    public object? Set
    {
        get => GetValue(SetProperty);
        set => SetValue(SetProperty, value);
    }

    readonly StackPanel stack = new() { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Right, VerticalAlignment = VerticalAlignment.Center };

    public FollowMarks()
    {
        Room(false);
        Children.Add(stack);
        // Not a stop of its own: the row's name says the sentence.
        AutomationProperties.SetAccessibilityView(this, AccessibilityView.Raw);
    }

    void Take(MarkSet? was, MarkSet? now)
    {
        var marks = now?.Marks ?? [];
        var same = was is not null && now is not null && ReferenceEquals(was.Owner, now.Owner);
        if (same && Following.Same(was!.Marks, marks) && was.Room == now!.Room)
            return;
        ToolTipService.SetToolTip(this, marks.Length > 0 ? Following.MarksSpoken(marks) : null);
        if (same && marks.Length == 0 && stack.Children.Count > 0)
        {
            Motion.FadeOutThen(stack, () =>
            {
                if (Set is MarkSet { Marks.Length: 0 } still)
                {
                    stack.Children.Clear();
                    stack.Opacity = 1;
                    Room(still.Room);
                }
            });
            return;
        }
        Room(now?.Room == true);
        var arriving = same && was!.Marks.Length == 0 && marks.Length > 0;
        Draw(marks);
        if (arriving)
            Motion.FadeIn(stack);
        else
            Motion.Set(stack, true);
    }

    /// <summary>The slot's width, or no slot at all, so the row's cells stand as they did before marks.</summary>
    void Room(bool kept)
    {
        Width = kept ? Slot : 0;
        Visibility = kept ? Visibility.Visible : Visibility.Collapsed;
    }

    void Draw(FollowedMark[] marks)
    {
        stack.Children.Clear();
        if (marks.Length == 0 || Ctx.Current is not { } ctx)
            return;
        var ring = Ui.Res<Brush>("GroundBrush");
        var (pictures, more) = Following.Stack(marks);
        foreach (var mark in pictures)
        {
            var face = FollowingSection.Face(ctx, mark.Name, mark.AvatarUrl, Size, ring);
            // The board's -6 between 24 px circles, with the 2 px rings outside them.
            if (stack.Children.Count > 0)
                face.Margin = new Thickness(-10, 0, 0, 0);
            stack.Children.Add(face);
        }
        if (more <= 0)
            return;
        var count = new Border
        {
            Height = Size,
            MinWidth = Size,
            CornerRadius = new CornerRadius(Size / 2),
            Background = Ui.Res<Brush>("SelectedFillBrush"),
            Padding = new Thickness(9, 0, 4, 0),
            Child = new TextBlock
            {
                Text = Following.More(more),
                FontSize = 11,
                FontWeight = FontWeights.SemiBold,
                Foreground = Ui.Res<Brush>("SilkBrush"),
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            },
        };
        // The count tucks under the last picture as the pictures do, its text clear of the overlap.
        var chip = new Border { Height = Size + 4, CornerRadius = new CornerRadius(Size / 2 + 2), Background = ring, Padding = new Thickness(2), Child = count, Margin = new Thickness(-10, 0, 0, 0) };
        stack.Children.Add(chip);
    }
}
