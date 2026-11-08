using Medium = Kaeri.Engine.Core.Medium;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Automation.Provider;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Pages;

/// <summary>
/// One choice of a group where one is picked: drawn as a segment or a row, read by Narrator as
/// a radio button with its place in the group ("2 of 4").
/// </summary>
sealed class ChoiceButton : ToggleButton
{
    public Action? Pick { get; set; }

    protected override AutomationPeer OnCreateAutomationPeer() => new ChoicePeer(this);

    sealed class ChoicePeer(ChoiceButton owner) : ToggleButtonAutomationPeer(owner), ISelectionItemProvider
    {
        protected override AutomationControlType GetAutomationControlTypeCore() => AutomationControlType.RadioButton;

        protected override string GetClassNameCore() => nameof(RadioButton);

        protected override object? GetPatternCore(PatternInterface pattern) => pattern switch
        {
            PatternInterface.SelectionItem => this,
            PatternInterface.Toggle => null,
            _ => base.GetPatternCore(pattern),
        };

        public bool IsSelected => owner.IsChecked == true;

        public IRawElementProviderSimple? SelectionContainer => null;

        public void AddToSelection() => Select();

        public void RemoveFromSelection()
        {
        }

        public void Select() => owner.Pick?.Invoke();
    }
}

/// <summary>
/// A group where one is picked: one Tab stop on the chosen item, the arrow keys move along
/// the group (and, with <c>pickOnArrow</c>, pick as they go), Home and End reach its ends.
/// </summary>
static class ChoiceGroup
{
    public static void Join(IReadOnlyList<ChoiceButton> buttons, Action<int> pick, bool pickOnArrow)
    {
        for (var i = 0; i < buttons.Count; i++)
        {
            var at = i;
            var button = buttons[i];
            AutomationProperties.SetPositionInSet(button, i + 1);
            AutomationProperties.SetSizeOfSet(button, buttons.Count);
            button.Pick = () => Choose(at);
            button.Click += (_, _) => Choose(at);
            button.KeyDown += (_, e) =>
            {
                var to = e.Key switch
                {
                    global::Windows.System.VirtualKey.Left or global::Windows.System.VirtualKey.Up => at - 1,
                    global::Windows.System.VirtualKey.Right or global::Windows.System.VirtualKey.Down => at + 1,
                    global::Windows.System.VirtualKey.Home => 0,
                    global::Windows.System.VirtualKey.End => buttons.Count - 1,
                    _ => -1,
                };
                if (e.Key is global::Windows.System.VirtualKey.Left or global::Windows.System.VirtualKey.Up && at == 0)
                    to = buttons.Count - 1;
                if (e.Key is global::Windows.System.VirtualKey.Right or global::Windows.System.VirtualKey.Down && at == buttons.Count - 1)
                    to = 0;
                if (to < 0)
                    return;
                e.Handled = true;
                buttons[to].Focus(FocusState.Keyboard);
                if (pickOnArrow)
                    Choose(to);
            };
        }
        Stops();

        void Choose(int at)
        {
            for (var i = 0; i < buttons.Count; i++)
            {
                var was = buttons[i].IsChecked == true;
                buttons[i].IsChecked = i == at;
                if (was != (i == at) && FrameworkElementAutomationPeer.FromElement(buttons[i]) is { } peer)
                    peer.RaisePropertyChangedEvent(SelectionItemPatternIdentifiers.IsSelectedProperty, was, i == at);
            }
            Stops();
            pick(at);
        }

        // One Tab stop: the chosen item, or the first when none is.
        void Stops()
        {
            var chosen = Math.Max(0, buttons.ToList().FindIndex(b => b.IsChecked == true));
            for (var i = 0; i < buttons.Count; i++)
                buttons[i].IsTabStop = i == chosen;
        }
    }
}

/// <summary>A row of choices where one is picked: the reader's segmented control. A swatch option shows its colour, named for Narrator.</summary>
sealed class Segments : Grid
{
    readonly List<ChoiceButton> buttons = [];

    public Segments(IReadOnlyList<(string Name, string Label)> options, string? chosen, ReaderPalette palette, Action<string> pick)
        : this(options.Select(o => (o.Name, o.Label, (global::Windows.UI.Color?)null)).ToList(), chosen, palette, pick)
    {
    }

    public Segments(IReadOnlyList<(string Name, string Label, global::Windows.UI.Color? Swatch)> options, string? chosen, ReaderPalette palette, Action<string> pick)
    {
        ColumnSpacing = 4;
        for (var i = 0; i < options.Count; i++)
        {
            var (name, label, swatch) = options[i];
            ColumnDefinitions.Add(new ColumnDefinition { Width = swatch is null ? new GridLength(1, GridUnitType.Star) : new GridLength(48) });
            var button = new ChoiceButton
            {
                // Text wraps rather than clipping at large text sizes.
                Content = swatch is { } colour
                    ? new Microsoft.UI.Xaml.Shapes.Ellipse
                    {
                        Width = 22,
                        Height = 22,
                        Fill = new SolidColorBrush(colour),
                        Stroke = new SolidColorBrush(palette.Rule),
                        StrokeThickness = 1,
                    }
                    : new TextBlock { Text = label, TextWrapping = TextWrapping.WrapWholeWords, TextAlignment = TextAlignment.Center },
                IsChecked = name == chosen,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                VerticalAlignment = VerticalAlignment.Stretch,
                MinHeight = 32,
                Padding = new Thickness(6, 4, 6, 4),
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
            res["ToggleButtonBorderBrushChecked"] = new SolidColorBrush(swatch is null ? Microsoft.UI.Colors.Transparent : palette.Silk);
            AutomationProperties.SetName(button, label);
            if (swatch is not null)
                ToolTipService.SetToolTip(button, label);
            SetColumn(button, i);
            Children.Add(button);
            buttons.Add(button);
        }
        ChoiceGroup.Join(buttons, at => pick(options[at].Name), pickOnArrow: true);
    }
}

/// <summary>
/// The Aa flyout: what belongs to this series (the reading mode) and what stays on this PC
/// for every book of its kind, then the way to Settings' Reading pane. A novel's has the
/// font, size, margins, line spacing, justification, look and Paper mode.
/// </summary>
static class SettingsFlyout
{
    /// <summary>A mode was just picked: the next build focuses the mode row.</summary>
    internal static bool ModePicked;

    public static FrameworkElement Build(ReaderOverlay owner, Flyout flyout)
    {
        var reader = owner.Reader;
        var palette = owner.Palette;
        var (width, height) = Size(owner, 340, 560);
        var stack = new StackPanel { Spacing = 6, Width = width, Padding = new Thickness(4) };
        var medium = reader.Medium;
        var strip = reader.View?.Mode is ReaderMode.Strip or ReaderMode.Vertical;
        var main = new ScrollViewer { Content = stack, MaxHeight = height, VerticalScrollBarVisibility = ScrollBarVisibility.Auto, HorizontalScrollMode = ScrollMode.Disabled };
        // The flyout holds two pages: the settings, and the reading mode picker pushed over them.
        var host = new Grid();
        host.Children.Add(main);
        Button? modeRow = null;

        if (medium != Medium.Novel)
        {
            stack.Children.Add(Heading(T.WindowsReaderThisSeries, palette, first: true));
            var value = reader.ModeOverridden ? ModeName(reader.ChosenMode) : T.WindowsReaderModeAuto(Guessed(reader));
            modeRow = PushRow(T.WindowsReaderReadingMode, value, palette);
            modeRow.Click += (_, _) => PushModes(owner, host, main, modeRow, width, height);
            stack.Children.Add(modeRow);
            if (ModePicked)
            {
                var focusRow = modeRow;
                focusRow.Loaded += (_, _) =>
                {
                    ModePicked = false;
                    focusRow.Focus(FocusState.Programmatic);
                };
            }
            if (!reader.ModeOverridden)
            {
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
            // System, then the four looks as swatches, each named for Narrator.
            stack.Children.Add(new Segments(
                [
                    ("system", T.ReaderLookSystem, null),
                    ("paper", T.ReaderLookPaper, ReaderPalette.NovelGround("paper", false)),
                    ("light", T.ReaderLookLight, ReaderPalette.NovelGround("light", false)),
                    ("sepia", T.ReaderLookSepia, ReaderPalette.NovelGround("sepia", false)),
                    ("dark", T.ReaderLookDark, ReaderPalette.NovelGround("dark", false)),
                ],
                reader.Choice(ReaderSettingKey.Look) ?? "system", palette,
                name => reader.Set(ReaderSettingKey.Look, new ReaderValue.Choice(name))));
        }

        // Paper mode: only where the engine offers its row (a touch screen it counts as handheld).
        if (owner.Reader.Row(ReaderSettingKey.PaperMode) is not null)
        {
            var paper = new ToggleSwitch { Header = null, IsOn = owner.PaperMode, OnContent = "", OffContent = "", MinWidth = 0 };
            AutomationProperties.SetName(paper, T.WindowsReaderPaperMode);
            paper.Toggled += (_, _) => owner.SetPaperMode(paper.IsOn);
            stack.Children.Add(SwitchRow(T.WindowsReaderPaperMode, paper, palette));
            stack.Children.Add(Note(T.WindowsReaderPaperModeNote, palette));
        }

        var more = new HyperlinkButton { Content = T.WindowsReaderMoreSettings, Margin = new Thickness(0, 10, 0, 0), Padding = new Thickness(0, 4, 0, 4) };
        more.Click += (_, _) =>
        {
            flyout.Hide();
            owner.MoreSettings();
        };
        stack.Children.Add(more);
        return host;
    }

    /// <summary>
    /// A flyout's size: its base width grows with the text size, up to what the window leaves,
    /// and its height stays inside the window.
    /// </summary>
    internal static (double Width, double Height) Size(ReaderOverlay owner, double width, double height)
    {
        var text = Math.Max(1, new global::Windows.UI.ViewManagement.UISettings().TextScaleFactor);
        var roomW = owner.ActualWidth > 0 ? owner.ActualWidth - 48 : width * text;
        var roomH = owner.ActualHeight > 0 ? owner.ActualHeight - 180 : height;
        return (Math.Max(Math.Min(width * Math.Min(text, 1.6), roomW), Math.Min(width, roomW)), Math.Max(240, Math.Min(height * Math.Min(text, 1.4), roomH)));
    }

    static string Guessed(ReaderSession reader) => reader.View?.Mode switch
    {
        ReaderMode.Strip => T.WindowsReaderModeStripLower,
        ReaderMode.Vertical => T.WindowsReaderModeVerticalLower,
        _ => reader.View?.Direction == ReadDirection.LeftToRight ? T.WindowsReaderModeLtrLower : T.WindowsReaderModeRtlLower,
    };

    static string GuessedWay(ReaderSession reader) => reader.View?.Mode switch
    {
        ReaderMode.Strip => T.ReaderModeStrip,
        ReaderMode.Vertical => T.WindowsReaderModeVertical,
        _ => reader.View?.Direction == ReadDirection.LeftToRight ? T.ReaderModeLtr : T.ReaderModeRtl,
    };

    static string ModeName(ReaderSession.ModeChoice mode) => mode switch
    {
        ReaderSession.ModeChoice.RightToLeft => T.ReaderModeRtl,
        ReaderSession.ModeChoice.LeftToRight => T.ReaderModeLtr,
        ReaderSession.ModeChoice.Vertical => T.WindowsReaderModeVertical,
        ReaderSession.ModeChoice.Strip => T.ReaderModeStrip,
        _ => T.WindowsReaderModeAutoOption,
    };

    /// <summary>A row that names a setting and its value, with a chevron: it pushes a picker.</summary>
    static Button PushRow(string label, string value, ReaderPalette palette)
    {
        var grid = new Grid { ColumnSpacing = 8 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.Children.Add(new TextBlock { Text = label, FontSize = 14, Foreground = new SolidColorBrush(palette.Silk), VerticalAlignment = VerticalAlignment.Center, TextWrapping = TextWrapping.WrapWholeWords });
        var shown = new TextBlock { Text = value, FontSize = 14, Foreground = new SolidColorBrush(palette.Haze), VerticalAlignment = VerticalAlignment.Center, TextWrapping = TextWrapping.WrapWholeWords, TextAlignment = TextAlignment.Right };
        Grid.SetColumn(shown, 1);
        grid.Children.Add(shown);
        var chevron = new FontIcon { Glyph = "\uE76C", FontSize = 12, Foreground = new SolidColorBrush(palette.Haze) };
        Grid.SetColumn(chevron, 2);
        grid.Children.Add(chevron);
        var row = new Button
        {
            Content = grid,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            MinHeight = 40,
            Padding = new Thickness(8, 6, 8, 6),
            CornerRadius = new CornerRadius(4),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            Margin = new Thickness(-8, 2, -8, 0),
        };
        AutomationProperties.SetName(row, label);
        AutomationProperties.SetHelpText(row, value);
        return row;
    }

    /// <summary>
    /// The reading mode picker, pushed inside the flyout: Back, the series, the modes with Auto
    /// naming what it found, and where the choice is kept. A pick applies at once and pops back;
    /// focus goes to the chosen mode on push and back to the row on pop.
    /// </summary>
    static void PushModes(ReaderOverlay owner, Grid host, FrameworkElement main, Button row, double width, double height)
    {
        var reader = owner.Reader;
        var palette = owner.Palette;
        var page = new StackPanel { Spacing = 2, Width = width, Padding = new Thickness(4) };
        var scroll = new ScrollViewer { Content = page, MaxHeight = height, VerticalScrollBarVisibility = ScrollBarVisibility.Auto, HorizontalScrollMode = ScrollMode.Disabled };

        void Pop()
        {
            host.Children.Remove(scroll);
            main.Visibility = Visibility.Visible;
            if (!owner.PaperMode)
                Motion.FadeIn(main);
            row.Focus(FocusState.Programmatic);
        }

        var top = new Grid { ColumnSpacing = 4, Margin = new Thickness(-4, 0, 0, 4) };
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var back = new Button
        {
            Content = new FontIcon { Glyph = "\uE72B", FontSize = 14 },
            Width = 36,
            Height = 36,
            Padding = new Thickness(0),
            CornerRadius = new CornerRadius(4),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            Foreground = new SolidColorBrush(palette.Silk),
        };
        AutomationProperties.SetName(back, T.WindowsReaderBack);
        ToolTipService.SetToolTip(back, T.WindowsReaderBack);
        back.Click += (_, _) => Pop();
        top.Children.Add(back);
        var title = new TextBlock { Text = T.WindowsReaderReadingMode, FontSize = 17, FontWeight = FontWeights.SemiBold, Foreground = new SolidColorBrush(palette.Silk), VerticalAlignment = VerticalAlignment.Center };
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level2);
        Grid.SetColumn(title, 1);
        top.Children.Add(title);
        page.Children.Add(top);
        if (reader.View?.Title is { Length: > 0 } series)
            page.Children.Add(new TextBlock { Text = series, FontSize = 12, Foreground = new SolidColorBrush(palette.Haze), Margin = new Thickness(0, 0, 0, 6), TextTrimming = TextTrimming.CharacterEllipsis });

        var chosen = reader.ModeOverridden ? reader.ChosenMode : ReaderSession.ModeChoice.Auto;
        (ReaderSession.ModeChoice Mode, string Name, string? Hint)[] modes =
        [
            (ReaderSession.ModeChoice.Auto, T.WindowsReaderModeAutoOption, T.WindowsReaderModeAutoLine(GuessedWay(reader))),
            (ReaderSession.ModeChoice.RightToLeft, T.ReaderModeRtl, null),
            (ReaderSession.ModeChoice.LeftToRight, T.ReaderModeLtr, null),
            (ReaderSession.ModeChoice.Vertical, T.WindowsReaderModeVertical, T.WindowsReaderModeVerticalLine),
            (ReaderSession.ModeChoice.Strip, T.ReaderModeStrip, T.WindowsReaderModeStripLine),
        ];
        var buttons = new List<ChoiceButton>();
        foreach (var (mode, name, hint) in modes)
        {
            var words = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
            words.Children.Add(new TextBlock { Text = name, FontSize = 14, Foreground = new SolidColorBrush(palette.Silk), TextWrapping = TextWrapping.WrapWholeWords });
            if (hint is not null)
                words.Children.Add(new TextBlock { Text = hint, FontSize = 12, Foreground = new SolidColorBrush(palette.Haze), TextWrapping = TextWrapping.WrapWholeWords });
            var check = new FontIcon { Glyph = "\uE73E", FontSize = 14, Foreground = new SolidColorBrush(palette.Silk), Opacity = mode == chosen ? 1 : 0 };
            var grid = new Grid { ColumnSpacing = 8 };
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            grid.Children.Add(words);
            Grid.SetColumn(check, 1);
            grid.Children.Add(check);
            var button = new ChoiceButton
            {
                Content = grid,
                IsChecked = mode == chosen,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Stretch,
                MinHeight = 44,
                Padding = new Thickness(8, 6, 8, 6),
                Margin = new Thickness(-8, 0, -8, 0),
                CornerRadius = new CornerRadius(4),
                Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
                BorderThickness = new Thickness(0),
            };
            // The check shows the choice; the row keeps the list's own ground.
            var res = button.Resources;
            res["ToggleButtonBackgroundChecked"] = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
            res["ToggleButtonBackgroundCheckedPointerOver"] = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Silk, 0.06));
            res["ToggleButtonBackgroundCheckedPressed"] = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Silk, 0.1));
            res["ToggleButtonBorderBrushChecked"] = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
            AutomationProperties.SetName(button, name);
            if (hint is not null)
                AutomationProperties.SetHelpText(button, hint);
            buttons.Add(button);
            page.Children.Add(button);
        }
        // Arrows move between the modes; Space, Enter or a click picks, since a pick lays the book out again.
        ChoiceGroup.Join(buttons, at =>
        {
            ModePicked = true;
            reader.ChooseMode(modes[at].Mode);
            Pop();
        }, pickOnArrow: false);
        page.Children.Add(Note(T.WindowsReaderModeChoiceNote, palette));
        page.KeyDown += (_, e) =>
        {
            if (e.Key is global::Windows.System.VirtualKey.Escape or global::Windows.System.VirtualKey.Back
                || (e.Key == global::Windows.System.VirtualKey.Left && Microsoft.UI.Input.InputKeyboardSource.GetKeyStateForCurrentThread(global::Windows.System.VirtualKey.Menu).HasFlag(global::Windows.UI.Core.CoreVirtualKeyStates.Down)))
            {
                e.Handled = true;
                Pop();
            }
        };

        main.Visibility = Visibility.Collapsed;
        host.Children.Add(scroll);
        if (!owner.PaperMode)
            Motion.FadeIn(scroll);
        var focus = buttons[Math.Max(0, Array.FindIndex(modes, m => m.Mode == chosen))];
        scroll.Loaded += (_, _) => focus.Focus(FocusState.Programmatic);
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
        var (width, height) = SettingsFlyout.Size(owner, 380, 520);
        var root = new Grid { Width = width, Height = height, RowSpacing = 8 };
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
            // A quick list never flashes a loading line: it shows after 600 ms, fading in.
            var waiting = new TextBlock
            {
                Text = T.ReaderChaptersLoading,
                Foreground = new SolidColorBrush(palette.Haze),
                FontSize = 13,
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
                Opacity = 0,
            };
            AutomationProperties.SetLiveSetting(waiting, AutomationLiveSetting.Polite);
            var timer = waiting.DispatcherQueue.CreateTimer();
            timer.Interval = TimeSpan.FromMilliseconds(600);
            timer.IsRepeating = false;
            timer.Tick += (_, _) =>
            {
                if (owner.PaperMode)
                    waiting.Opacity = 1;
                else
                    Motion.FadeIn(waiting);
            };
            waiting.Loaded += (_, _) => timer.Start();
            waiting.Unloaded += (_, _) => timer.Stop();
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
