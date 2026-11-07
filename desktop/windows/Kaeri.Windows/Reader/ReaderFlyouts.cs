using Medium = Kaeri.Engine.Core.Medium;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>A row of choices where one is picked: the reader's segmented control.</summary>
sealed class Segments : Grid
{
    readonly List<ToggleButton> buttons = [];

    public Segments(IReadOnlyList<(string Name, string Label)> options, string? chosen, ReaderPalette palette, Action<string> pick)
    {
        ColumnSpacing = 4;
        for (var i = 0; i < options.Count; i++)
        {
            var (name, label) = options[i];
            ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            var button = new ToggleButton
            {
                Content = label,
                IsChecked = name == chosen,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                MinHeight = 32,
                Padding = new Thickness(6, 0, 6, 0),
                CornerRadius = new CornerRadius(4),
                FontSize = 13,
            };
            var res = button.Resources;
            res["ToggleButtonBackgroundChecked"] = new SolidColorBrush(palette.Selected);
            res["ToggleButtonBackgroundCheckedPointerOver"] = new SolidColorBrush(palette.Selected);
            res["ToggleButtonBackgroundCheckedPressed"] = new SolidColorBrush(palette.Selected);
            res["ToggleButtonForegroundChecked"] = new SolidColorBrush(palette.Silk);
            res["ToggleButtonForegroundCheckedPointerOver"] = new SolidColorBrush(palette.Silk);
            res["ToggleButtonForegroundCheckedPressed"] = new SolidColorBrush(palette.Silk);
            res["ToggleButtonBorderBrushChecked"] = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
            AutomationProperties.SetName(button, label);
            button.Click += (_, _) =>
            {
                foreach (var other in buttons)
                    other.IsChecked = other == button;
                pick(name);
            };
            SetColumn(button, i);
            Children.Add(button);
            buttons.Add(button);
        }
    }
}

/// <summary>
/// The Aa flyout: what belongs to this series (the reading mode) and what stays on this PC
/// for every book of its kind, then the way to Settings' Reading pane. A novel's has the
/// font, size, margins, line spacing, justification, look and Paper mode.
/// </summary>
static class SettingsFlyout
{
    public static FrameworkElement Build(ReaderOverlay owner, Flyout flyout)
    {
        var reader = owner.Reader;
        var palette = owner.Palette;
        var stack = new StackPanel { Spacing = 6, Width = 340, Padding = new Thickness(4) };
        var medium = reader.Medium;
        var strip = reader.View?.Mode is ReaderMode.Strip or ReaderMode.Vertical;

        if (medium != Medium.Novel)
        {
            stack.Children.Add(Heading(T.WindowsReaderThisSeries, palette, first: true));
            stack.Children.Add(Label(T.WindowsReaderReadingMode, palette));
            var chosen = reader.ModeOverridden ? reader.ChosenMode : ReaderSession.ModeChoice.Auto;
            stack.Children.Add(new Segments(
                [
                    (nameof(ReaderSession.ModeChoice.Auto), T.WindowsReaderModeAutoOption),
                    (nameof(ReaderSession.ModeChoice.RightToLeft), T.ReaderModeRtl),
                    (nameof(ReaderSession.ModeChoice.LeftToRight), T.ReaderModeLtr),
                    (nameof(ReaderSession.ModeChoice.Strip), T.ReaderModeStrip),
                ],
                chosen.ToString(), palette,
                name => reader.ChooseMode(Enum.Parse<ReaderSession.ModeChoice>(name))));
            if (!reader.ModeOverridden)
            {
                var guessed = strip ? T.WindowsReaderModeStripLower
                    : reader.View?.Direction == ReadDirection.LeftToRight ? T.WindowsReaderModeLtrLower : T.WindowsReaderModeRtlLower;
                stack.Children.Add(Note(T.WindowsReaderModeAuto(guessed), palette));
                if (reader.Row(ReaderSettingKey.Direction)?.Detected == true || reader.Row(ReaderSettingKey.Mode)?.Detected == true)
                    stack.Children.Add(Note(T.WindowsReaderModeDetectedNote, palette));
            }
            else
            {
                stack.Children.Add(Note(T.WindowsReaderModeChoiceNote, palette));
                var useDefault = new HyperlinkButton { Content = T.WindowsReaderUseDefault, Padding = new Thickness(0, 2, 0, 2) };
                useDefault.Click += (_, _) => reader.ChooseMode(ReaderSession.ModeChoice.Auto);
                stack.Children.Add(useDefault);
            }

            stack.Children.Add(Heading(strip ? T.WindowsReaderAllStripsPc : T.WindowsReaderAllMangaPc, palette));
            if (!strip)
            {
                stack.Children.Add(Label(T.WindowsReaderPageLayout, palette));
                stack.Children.Add(Choice(owner, ReaderSettingKey.PageLayout, "auto",
                    [("auto", T.ReaderLayoutAuto), ("single", T.ReaderLayoutSingle), ("double", T.ReaderLayoutDouble)]));
                stack.Children.Add(Label(T.WindowsReaderFit, palette));
                stack.Children.Add(Choice(owner, ReaderSettingKey.Fit, "screen",
                    [("screen", T.ReaderFitScreen), ("width", T.ReaderFitWidth), ("height", T.WindowsReaderFitHeight)]));
                stack.Children.Add(Toggle(owner, ReaderSettingKey.CropBorders, T.WindowsReaderCropBorders, null));
            }
            else
            {
                stack.Children.Add(Label(T.WindowsReaderGap, palette));
                stack.Children.Add(Choice(owner, ReaderSettingKey.StripGap, "none",
                    [("none", T.ReaderGapNone), ("small", T.ReaderGapSmall)]));
            }
            stack.Children.Add(Label(T.WindowsReaderBackground, palette));
            stack.Children.Add(Choice(owner, ReaderSettingKey.Background, "theme",
                [("theme", T.ReaderBackgroundTheme), ("black", T.ReaderBackgroundBlack), ("white", T.ReaderBackgroundWhite)]));
        }
        else
        {
            stack.Children.Add(Heading(T.WindowsReaderAllNovels, palette, first: true));
            stack.Children.Add(Label(T.WindowsReaderReadingMode, palette));
            stack.Children.Add(Choice(owner, ReaderSettingKey.Mode, "paged",
                [("paged", T.WindowsReaderModePaged), ("scrolled", T.WindowsReaderModeScrolled)]));
            stack.Children.Add(Label(T.WindowsReaderFont, palette));
            stack.Children.Add(Choice(owner, ReaderSettingKey.Font, "serif",
                [("serif", T.ReaderFontSerif), ("sans", T.ReaderFontSans)]));
            stack.Children.Add(Label(T.WindowsReaderTextSize, palette));
            stack.Children.Add(TextSize(owner));
            stack.Children.Add(Label(T.WindowsReaderLineSpacing, palette));
            stack.Children.Add(Amounts(owner, ReaderSettingKey.LineSpacing, 1.55,
                [(1.2, T.WindowsReaderLineTight), (1.55, T.WindowsReaderLineNormal), (1.8, T.WindowsReaderLineLoose)]));
            stack.Children.Add(Label(T.WindowsReaderMargins, palette));
            stack.Children.Add(Amounts(owner, ReaderSettingKey.Margins, 1.0,
                [(0.6, T.ReaderMarginsNarrow), (1.0, T.WindowsReaderLineNormal), (1.6, T.ReaderMarginsWide)]));
            stack.Children.Add(Toggle(owner, ReaderSettingKey.Justify, T.WindowsReaderJustify, null));
            stack.Children.Add(Label(T.WindowsReaderLook, palette));
            stack.Children.Add(Choice(owner, ReaderSettingKey.Look, "system",
                [("system", T.ReaderLookSystem), ("paper", T.ReaderLookPaper), ("light", T.ReaderLookLight), ("sepia", T.ReaderLookSepia), ("dark", T.ReaderLookDark)]));
        }

        // Paper mode: the engine's row where it offers one, otherwise this PC's own choice.
        var paper = new ToggleSwitch { Header = null, IsOn = owner.PaperMode, OnContent = "", OffContent = "", MinWidth = 0 };
        AutomationProperties.SetName(paper, T.WindowsReaderPaperMode);
        paper.Toggled += (_, _) => owner.SetPaperMode(paper.IsOn);
        stack.Children.Add(SwitchRow(T.WindowsReaderPaperMode, paper, palette));
        stack.Children.Add(Note(T.WindowsReaderPaperModeNote, palette));

        var more = new HyperlinkButton { Content = T.WindowsReaderMoreSettings, Margin = new Thickness(0, 10, 0, 0), Padding = new Thickness(0, 4, 0, 4) };
        more.Click += (_, _) =>
        {
            flyout.Hide();
            owner.MoreSettings();
        };
        stack.Children.Add(more);
        return new ScrollViewer { Content = stack, MaxHeight = 560, VerticalScrollBarVisibility = ScrollBarVisibility.Auto, HorizontalScrollMode = ScrollMode.Disabled };
    }

    static TextBlock Heading(string text, ReaderPalette palette, bool first = false)
    {
        var heading = new TextBlock
        {
            Text = text,
            FontSize = 13,
            FontWeight = FontWeights.SemiBold,
            Foreground = new SolidColorBrush(palette.Silk),
            Margin = new Thickness(0, first ? 0 : 14, 0, 2),
        };
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
        return heading;
    }

    static TextBlock Label(string text, ReaderPalette palette) => new()
    {
        Text = text,
        FontSize = 12,
        Foreground = new SolidColorBrush(palette.Haze),
        Margin = new Thickness(0, 6, 0, 0),
    };

    static TextBlock Note(string text, ReaderPalette palette) => new()
    {
        Text = text,
        FontSize = 12,
        Foreground = new SolidColorBrush(palette.Haze),
        TextWrapping = TextWrapping.WrapWholeWords,
    };

    static Segments Choice(ReaderOverlay owner, ReaderSettingKey key, string fallback, IReadOnlyList<(string, string)> options) =>
        new(options, owner.Reader.Choice(key) ?? fallback, owner.Palette, name => owner.Reader.Set(key, new ReaderValue.Choice(name)));

    static Segments Amounts(ReaderOverlay owner, ReaderSettingKey key, double fallback, IReadOnlyList<(double Value, string Label)> options)
    {
        var now = owner.Reader.Amount(key) ?? fallback;
        var nearest = options.MinBy(o => Math.Abs(o.Value - now)).Value;
        var named = options.Select(o => (o.Value.ToString(System.Globalization.CultureInfo.InvariantCulture), o.Label)).ToList();
        return new Segments(named, nearest.ToString(System.Globalization.CultureInfo.InvariantCulture), owner.Palette,
            name => owner.Reader.Set(key, new ReaderValue.Amount((float)double.Parse(name, System.Globalization.CultureInfo.InvariantCulture))));
    }

    static Grid SwitchRow(string text, ToggleSwitch toggle, ReaderPalette palette)
    {
        var row = new Grid { Margin = new Thickness(0, 6, 0, 0) };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.Children.Add(new TextBlock { Text = text, FontSize = 14, Foreground = new SolidColorBrush(palette.Silk), VerticalAlignment = VerticalAlignment.Center });
        Grid.SetColumn(toggle, 1);
        row.Children.Add(toggle);
        return row;
    }

    static Grid Toggle(ReaderOverlay owner, ReaderSettingKey key, string text, string? note)
    {
        var toggle = new ToggleSwitch { IsOn = owner.Reader.IsOn(key), OnContent = "", OffContent = "", MinWidth = 0 };
        AutomationProperties.SetName(toggle, text);
        toggle.Toggled += (_, _) => owner.Reader.Set(key, new ReaderValue.Toggle(toggle.IsOn));
        return SwitchRow(text, toggle, owner.Palette);
    }

    /// <summary>Text size: smaller, the size in percent, larger.</summary>
    static Grid TextSize(ReaderOverlay owner)
    {
        var palette = owner.Palette;
        var now = owner.Reader.Amount(ReaderSettingKey.TextSize) ?? 1.0;
        var value = new TextBlock
        {
            Text = T.WindowsReaderPercent((int)Math.Round(now * 100)),
            FontSize = 14,
            Foreground = new SolidColorBrush(palette.Silk),
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetLiveSetting(value, AutomationLiveSetting.Polite);
        void Step(double by)
        {
            now = Math.Clamp(Math.Round((now + by) * 20) / 20, 0.7, 2.0);
            value.Text = T.WindowsReaderPercent((int)Math.Round(now * 100));
            owner.Reader.Set(ReaderSettingKey.TextSize, new ReaderValue.Amount((float)now));
        }
        var smaller = new Button { Content = new FontIcon { Glyph = "", FontSize = 12 }, Width = 40, Height = 32, CornerRadius = new CornerRadius(4) };
        AutomationProperties.SetName(smaller, T.WindowsReaderSmaller);
        ToolTipService.SetToolTip(smaller, T.WindowsReaderSmaller);
        smaller.Click += (_, _) => Step(-0.05);
        var larger = new Button { Content = new FontIcon { Glyph = "", FontSize = 12 }, Width = 40, Height = 32, CornerRadius = new CornerRadius(4) };
        AutomationProperties.SetName(larger, T.WindowsReaderLarger);
        ToolTipService.SetToolTip(larger, T.WindowsReaderLarger);
        larger.Click += (_, _) => Step(0.05);
        var row = new Grid();
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.Children.Add(smaller);
        Grid.SetColumn(value, 1);
        row.Children.Add(value);
        Grid.SetColumn(larger, 2);
        row.Children.Add(larger);
        return row;
    }
}

/// <summary>
/// The Chapters flyout: the chapter's pages as a grid, five across and laid out as the book
/// reads, and the chapter list by volume. A novel shows its contents and the list.
/// </summary>
static class ChaptersFlyout
{
    public static FrameworkElement Build(ReaderOverlay owner, Flyout flyout)
    {
        var reader = owner.Reader;
        var palette = owner.Palette;
        var root = new Grid { Width = 380, Height = 520, RowSpacing = 8 };
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        var hasPages = reader.Medium != Medium.Novel && reader.View?.Mode == ReaderMode.Paged && reader.Leaves.Length > 0;
        var hasContents = reader.Medium == Medium.Novel && owner.NovelContents.Length > 0;
        var body = new ContentControl { HorizontalContentAlignment = HorizontalAlignment.Stretch, VerticalContentAlignment = VerticalAlignment.Stretch };
        Grid.SetRow(body, 1);
        root.Children.Add(body);
        FrameworkElement List() => ChapterList(owner, flyout);
        if (hasPages || hasContents)
        {
            var bar = new SelectorBar();
            var first = new SelectorBarItem { Text = hasPages ? T.WindowsReaderPagesTab : T.ReaderChapters, IsSelected = true };
            var second = new SelectorBarItem { Text = T.WindowsReaderChaptersTab };
            bar.Items.Add(first);
            bar.Items.Add(second);
            bar.SelectionChanged += (_, _) =>
                body.Content = bar.SelectedItem == first ? (hasPages ? PageGrid(owner, flyout) : Contents(owner, flyout)) : List();
            root.Children.Add(bar);
            body.Content = hasPages ? PageGrid(owner, flyout) : Contents(owner, flyout);
        }
        else
        {
            Grid.SetRow(body, 0);
            Grid.SetRowSpan(body, 2);
            body.Content = List();
        }
        return root;
    }

    /// <summary>The pages, five across; in a right-to-left book the first is at the top right.</summary>
    static FrameworkElement PageGrid(ReaderOverlay owner, Flyout flyout)
    {
        var reader = owner.Reader;
        var palette = owner.Palette;
        var chapter = reader.View?.Chapter.Id ?? "";
        var current = owner.ShownLeaf;
        var entries = new List<(LeafPart Part, int Leaf)>();
        for (var i = 0; i < reader.Leaves.Length; i++)
        {
            var leaf = reader.Leaves[i];
            if (leaf.EndPage)
                continue;
            foreach (var part in leaf.Parts.OrderBy(p => p.Page).ThenBy(p => p.Half))
                entries.Add((part, i));
        }
        const int across = 5;
        var grid = new Grid { ColumnSpacing = 8, RowSpacing = 10, FlowDirection = owner.Rtl ? FlowDirection.RightToLeft : FlowDirection.LeftToRight };
        for (var c = 0; c < across; c++)
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var rows = (entries.Count + across - 1) / across;
        for (var r = 0; r < rows; r++)
            grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        Button? ringed = null;
        for (var k = 0; k < entries.Count; k++)
        {
            var (part, leaf) = entries[k];
            var number = (int)part.Page + 1;
            var thumb = new PageImageView { Height = 90, Dim = palette.Paper };
            thumb.FlowDirection = FlowDirection.LeftToRight;
            if (reader.FileOf(part, chapter) is { } file)
                thumb.Show(file, owner.Scale);
            else if (reader.FailureOf(part, chapter) is null)
                reader.NeedPage(part.Page);
            var frame = new Border { Height = 90, Background = new SolidColorBrush(palette.Raised), CornerRadius = new CornerRadius(4), Child = thumb };
            var label = new TextBlock { Text = number.ToString(System.Globalization.CultureInfo.CurrentCulture), FontSize = 12, Foreground = new SolidColorBrush(palette.Haze), HorizontalAlignment = HorizontalAlignment.Center };
            var cell = new StackPanel { Spacing = 4 };
            cell.Children.Add(frame);
            cell.Children.Add(label);
            var isCurrent = leaf == current;
            var button = new Button
            {
                Content = cell,
                Padding = new Thickness(2),
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Stretch,
                Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
                BorderBrush = new SolidColorBrush(isCurrent ? palette.Silk : Microsoft.UI.Colors.Transparent),
                BorderThickness = new Thickness(isCurrent ? 2 : 0),
                CornerRadius = new CornerRadius(6),
            };
            AutomationProperties.SetName(button, T.WindowsReaderGridPage(number));
            if (isCurrent)
                AutomationProperties.SetItemStatus(button, T.WindowsReaderPageLabel(number, reader.PageCount));
            var target = leaf;
            button.Click += (_, _) =>
            {
                flyout.Hide();
                owner.JumpToLeaf(target);
            };
            Grid.SetColumn(button, k % across);
            Grid.SetRow(button, k / across);
            grid.Children.Add(button);
            if (isCurrent)
                ringed ??= button;
        }
        var scroll = new ScrollViewer { Content = grid, VerticalScrollBarVisibility = ScrollBarVisibility.Auto, Padding = new Thickness(0, 0, 8, 0) };
        if (ringed is not null)
            scroll.Loaded += (_, _) => ringed.StartBringIntoView(new BringIntoViewOptions { VerticalAlignmentRatio = 0.4, AnimationDesired = false });
        return scroll;
    }

    /// <summary>A novel's contents: each entry goes to its place in the book.</summary>
    static FrameworkElement Contents(ReaderOverlay owner, Flyout flyout)
    {
        var palette = owner.Palette;
        var stack = new StackPanel { Spacing = 2 };
        foreach (var entry in owner.NovelContents)
        {
            var button = Row(entry.Label, null, false, palette);
            var url = entry.Url;
            button.Click += (_, _) =>
            {
                flyout.Hide();
                owner.NovelGoToUrl(url);
            };
            stack.Children.Add(button);
        }
        return new ScrollViewer { Content = stack, VerticalScrollBarVisibility = ScrollBarVisibility.Auto };
    }

    /// <summary>The chapters by volume, the open one on the selected fill; a click opens one.</summary>
    static FrameworkElement ChapterList(ReaderOverlay owner, Flyout flyout)
    {
        var reader = owner.Reader;
        var palette = owner.Palette;
        if (!reader.ChaptersLoaded)
        {
            var waiting = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
            waiting.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
            waiting.Children.Add(new TextBlock { Text = T.ReaderChaptersLoading, Foreground = new SolidColorBrush(palette.Haze), FontSize = 13 });
            return waiting;
        }
        var openId = reader.View?.Chapter.Id;
        var stack = new StackPanel { Spacing = 2 };
        Button? open = null;
        foreach (var group in reader.Chapters)
        {
            var heading = group.Title ?? (group.Volume is { Length: > 0 } volume ? T.WindowsReaderVolume(volume) : null);
            if (heading is not null)
            {
                var text = new TextBlock { Text = heading, FontSize = 12, FontWeight = FontWeights.SemiBold, Foreground = new SolidColorBrush(palette.Haze), Margin = new Thickness(8, 12, 8, 4) };
                AutomationProperties.SetHeadingLevel(text, AutomationHeadingLevel.Level2);
                stack.Children.Add(text);
            }
            foreach (var row in group.Chapters)
            {
                var state = row.State switch
                {
                    ChapterReadState.Read => T.ChapterStateRead,
                    ChapterReadState.InProgress { Of: { } of } progress => T.ChapterStatePageOf(progress.Page, of),
                    _ when row.NewChapter => T.ChapterStateNew,
                    _ when row.Pages is { } pages => T.WindowsReaderChapterPages((int)pages),
                    _ => null,
                };
                var isOpen = row.Id == openId;
                var button = Row(row.Title ?? row.Label, state, isOpen, palette);
                var id = row.Id;
                button.Click += (_, _) =>
                {
                    flyout.Hide();
                    if (id != openId)
                        reader.OpenChapter(id);
                };
                stack.Children.Add(button);
                if (isOpen)
                    open = button;
            }
        }
        var scroll = new ScrollViewer { Content = stack, VerticalScrollBarVisibility = ScrollBarVisibility.Auto };
        if (open is not null)
            scroll.Loaded += (_, _) => open.StartBringIntoView(new BringIntoViewOptions { VerticalAlignmentRatio = 0.4, AnimationDesired = false });
        return scroll;
    }

    static Button Row(string title, string? state, bool selected, ReaderPalette palette)
    {
        var grid = new Grid { ColumnSpacing = 12 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.Children.Add(new TextBlock { Text = title, FontSize = 14, Foreground = new SolidColorBrush(palette.Silk), TextTrimming = TextTrimming.CharacterEllipsis, VerticalAlignment = VerticalAlignment.Center });
        if (state is not null)
        {
            var stateText = new TextBlock { Text = state, FontSize = 12, Foreground = new SolidColorBrush(palette.Haze), VerticalAlignment = VerticalAlignment.Center };
            Grid.SetColumn(stateText, 1);
            grid.Children.Add(stateText);
        }
        var button = new Button
        {
            Content = grid,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            MinHeight = 40,
            Padding = new Thickness(8, 4, 8, 4),
            CornerRadius = new CornerRadius(4),
            BorderThickness = new Thickness(0),
            Background = new SolidColorBrush(selected ? palette.Selected : Microsoft.UI.Colors.Transparent),
        };
        AutomationProperties.SetName(button, state is null ? title : title + ", " + state);
        return button;
    }
}
