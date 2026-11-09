using Medium = Kaeri.Engine.Core.Medium;
using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;

namespace Kaeri.Windows.Pages;

/// <summary>
/// The reader's chrome: the top row that is the title bar while the reader is up, the group at
/// the foot (the scrubber with a number at each end, then Chapters, the title and its line, Aa
/// and Full screen), the notice at the top centre and the Back to pill in the bottom leading
/// corner. The group is acrylic, 640 wide, 16 above the bottom, with 32 px buttons on 4 px corners.
/// </summary>
sealed class ReaderChrome
{
    readonly ReaderOverlay owner;

    public Grid Top { get; } = new() { Height = 48, VerticalAlignment = VerticalAlignment.Top };
    public Grid DragArea { get; } = new() { Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent) };
    public Border Group { get; } = new();
    public Border Notice { get; } = new() { Visibility = Visibility.Collapsed };
    public Button BackTo { get; } = new() { Visibility = Visibility.Collapsed };
    /// <summary>Over the whole reader: the scrub preview, which follows the thumb above the group.</summary>
    public Canvas PreviewLayer { get; } = new() { IsHitTestVisible = false };

    readonly TextBlock topTitle = new() { FontSize = 12, VerticalAlignment = VerticalAlignment.Center, TextTrimming = TextTrimming.CharacterEllipsis };
    readonly Border credit = new() { Visibility = Visibility.Collapsed, CornerRadius = new CornerRadius(14), Padding = new Thickness(12, 4, 12, 4), HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
    readonly TextBlock creditText = new() { FontSize = 12, TextTrimming = TextTrimming.CharacterEllipsis };
    readonly Button back;

    readonly Slider scrubber = new() { StepFrequency = 1, SmallChange = 1, LargeChange = 5, Minimum = 1, Maximum = 1, VerticalAlignment = VerticalAlignment.Center, IsThumbToolTipEnabled = false, Margin = new Thickness(8, 0, 8, 0) };
    readonly TextBlock leftNumber = Number();
    readonly TextBlock rightNumber = Number();
    readonly Canvas ringLayer = new() { IsHitTestVisible = false };
    readonly Ellipse ring = new() { Width = 10, Height = 10, StrokeThickness = 1.5, Visibility = Visibility.Collapsed };
    readonly Grid scrubRow = new();
    readonly TextBlock title = new() { FontSize = 13, FontWeight = FontWeights.SemiBold, TextTrimming = TextTrimming.CharacterEllipsis, TextWrapping = TextWrapping.NoWrap };
    readonly TextBlock line = new() { FontSize = 12, TextTrimming = TextTrimming.CharacterEllipsis, TextWrapping = TextWrapping.NoWrap };
    readonly TextBlock noticeText = new() { FontSize = 13, VerticalAlignment = VerticalAlignment.Center, TextWrapping = TextWrapping.WrapWholeWords, MaxWidth = 420 };
    readonly Button noticeAction = new() { Visibility = Visibility.Collapsed, MinHeight = 28, Padding = new Thickness(10, 0, 10, 0), CornerRadius = new CornerRadius(4) };
    public Button Chapters { get; }
    public Button Settings { get; }
    public Button Fullscreen { get; }
    readonly FontIcon fullscreenIcon = new() { Glyph = "", FontSize = 16 };
    bool scrubbing;
    bool setting;
    readonly StackPanel preview = new() { Spacing = 6, Width = PreviewWidth, Visibility = Visibility.Collapsed };
    readonly Border previewPage = new() { Width = PreviewWidth, Height = PreviewHeight, CornerRadius = new CornerRadius(4), BorderThickness = new Thickness(1) };
    readonly PageImageView previewImage = new();
    readonly TextBlock previewNumber = new() { FontSize = 13, FontWeight = FontWeights.SemiBold, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
    readonly Border previewPill = new() { Height = PillHeight, MinWidth = 36, CornerRadius = new CornerRadius(PillHeight / 2), Padding = new Thickness(10, 0, 10, 0), HorizontalAlignment = HorizontalAlignment.Center };
    int previewAt = -1;
    bool previewShown;

    const double PreviewWidth = 96, PreviewHeight = 136, PillHeight = 24;
    ReaderAction? noticeKind;

    static TextBlock Number() => new() { FontSize = 12, VerticalAlignment = VerticalAlignment.Center, MinWidth = 24, TextAlignment = TextAlignment.Center };

    public ReaderChrome(ReaderOverlay owner)
    {
        this.owner = owner;

        // The top row: Back, then the title that drags the window; the caption buttons sit over its end.
        back = IconButton("", T.ActionBack, "Esc", (_, _) => owner.Close());
        back.Margin = new Thickness(8, 0, 0, 0);
        Top.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        Top.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        Top.Children.Add(back);
        Grid.SetColumn(DragArea, 1);
        topTitle.Margin = new Thickness(12, 0, 140, 0);
        DragArea.Children.Add(topTitle);
        credit.Child = creditText;
        DragArea.Children.Add(credit);
        Top.Children.Add(DragArea);

        // The group.
        scrubRow.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        scrubRow.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        scrubRow.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        scrubRow.Children.Add(leftNumber);
        Grid.SetColumn(scrubber, 1);
        scrubRow.Children.Add(scrubber);
        Grid.SetColumn(ringLayer, 1);
        ringLayer.Margin = scrubber.Margin;
        ringLayer.Children.Add(ring);
        scrubRow.Children.Add(ringLayer);
        Grid.SetColumn(rightNumber, 2);
        scrubRow.Children.Add(rightNumber);
        AutomationProperties.SetName(scrubber, T.WindowsReaderScrubber);
        AutomationProperties.SetAccessibilityView(leftNumber, AccessibilityView.Raw);
        AutomationProperties.SetAccessibilityView(rightNumber, AccessibilityView.Raw);
        scrubber.AddHandler(UIElement.PointerPressedEvent, new PointerEventHandler((_, _) =>
        {
            scrubbing = true;
            ShowPreview((int)Math.Round(scrubber.Value));
        }), true);
        scrubber.AddHandler(UIElement.PointerReleasedEvent, new PointerEventHandler((_, _) => EndScrub()), true);
        scrubber.AddHandler(UIElement.PointerCaptureLostEvent, new PointerEventHandler((_, _) => EndScrub()), true);
        scrubber.ValueChanged += (_, e) =>
        {
            if (setting)
                return;
            ShowNumbers((int)e.NewValue);
            if (!scrubbing)
                owner.ScrubTo((int)Math.Round(e.NewValue));
            else
                ShowPreview((int)Math.Round(e.NewValue));
        };
        scrubber.SizeChanged += (_, _) => PlaceRing();

        // The scrub preview: the page under the thumb, 96 wide, with its number under it.
        previewImage.Width = PreviewWidth - 2;
        previewImage.Height = PreviewHeight - 2;
        previewPage.Child = previewImage;
        previewPill.Child = previewNumber;
        preview.Children.Add(previewPage);
        preview.Children.Add(previewPill);
        PreviewLayer.Children.Add(preview);
        AutomationProperties.SetAccessibilityView(preview, AccessibilityView.Raw);

        Chapters = IconButton("", T.ReaderChapters, null, null);
        Settings = IconButton("", T.WindowsReaderSettingsOpen, null, null);
        Fullscreen = IconButton("", T.PlayerFullscreen, "F11", (_, _) => owner.ToggleFullscreen());
        Fullscreen.Content = fullscreenIcon;
        var titles = new StackPanel { VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(4, 0, 4, 0) };
        titles.Children.Add(title);
        titles.Children.Add(line);
        var row = new Grid { ColumnSpacing = 8 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.Children.Add(Chapters);
        Grid.SetColumn(titles, 1);
        row.Children.Add(titles);
        Grid.SetColumn(Settings, 2);
        row.Children.Add(Settings);
        Grid.SetColumn(Fullscreen, 3);
        row.Children.Add(Fullscreen);
        var stack = new StackPanel { Spacing = 6 };
        stack.Children.Add(scrubRow);
        stack.Children.Add(row);
        Group.Child = stack;
        Group.Padding = new Thickness(12, 8, 12, 10);
        Group.CornerRadius = new CornerRadius(8);
        Group.HorizontalAlignment = HorizontalAlignment.Center;
        Group.VerticalAlignment = VerticalAlignment.Bottom;
        Group.Margin = new Thickness(16, 0, 16, 16);
        Group.BorderThickness = new Thickness(1);

        // The notice, top centre.
        var noticeRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        noticeRow.Children.Add(noticeText);
        noticeRow.Children.Add(noticeAction);
        Notice.Child = noticeRow;
        Notice.Padding = new Thickness(16, 8, 8, 8);
        Notice.CornerRadius = new CornerRadius(8);
        Notice.HorizontalAlignment = HorizontalAlignment.Center;
        Notice.VerticalAlignment = VerticalAlignment.Top;
        Notice.Margin = new Thickness(16, 56, 16, 0);
        AutomationProperties.SetLiveSetting(noticeText, AutomationLiveSetting.Polite);
        Notice.PointerEntered += (_, _) => owner.Reader.HoldNotice(true);
        Notice.PointerExited += (_, _) => owner.Reader.HoldNotice(false);
        Notice.GotFocus += (_, _) => owner.Reader.HoldNotice(true);
        Notice.LostFocus += (_, _) => owner.Reader.HoldNotice(false);
        noticeAction.Click += (_, _) =>
        {
            if (noticeKind is not { } action)
                return;
            if (action == ReaderAction.ChangeMode)
            {
                owner.Reader.Dismiss(null);
                owner.OpenSettings();
            }
            else
                owner.Reader.Act(action);
        };

        // Back to page N: the bottom leading corner.
        BackTo.HorizontalAlignment = HorizontalAlignment.Left;
        BackTo.VerticalAlignment = VerticalAlignment.Bottom;
        BackTo.Margin = new Thickness(16, 0, 16, 16);
        BackTo.MinHeight = 32;
        BackTo.Padding = new Thickness(14, 0, 14, 0);
        BackTo.CornerRadius = new CornerRadius(16);
        BackTo.Click += (_, _) => owner.Reader.Act(ReaderAction.BackTo);

        Paint(owner.Palette);
    }

    Button IconButton(string glyph, string name, string? key, RoutedEventHandler? click)
    {
        var button = new Button
        {
            Width = 32,
            Height = 32,
            Padding = new Thickness(0),
            CornerRadius = new CornerRadius(4),
            BorderThickness = new Thickness(0),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            Content = new FontIcon { Glyph = glyph, FontSize = 16 },
        };
        var spoken = key is null ? name : T.TooltipWithKey(name, key);
        ToolTipService.SetToolTip(button, spoken);
        AutomationProperties.SetName(button, name);
        if (key is not null)
            AutomationProperties.SetAcceleratorKey(button, key);
        if (click is not null)
            button.Click += click;
        return button;
    }

    /// <summary>The chrome in the reader's colours: the dark theme's, or paper's.</summary>
    public void Paint(ReaderPalette palette)
    {
        var silk = new SolidColorBrush(palette.Silk);
        var haze = new SolidColorBrush(palette.Haze);
        Top.Background = new LinearGradientBrush
        {
            StartPoint = new global::Windows.Foundation.Point(0, 0),
            EndPoint = new global::Windows.Foundation.Point(0, 1),
            GradientStops =
            {
                new GradientStop { Color = ReaderPalette.WithAlpha(palette.Ground, 0.85), Offset = 0 },
                new GradientStop { Color = ReaderPalette.WithAlpha(palette.Ground, 0), Offset = 1 },
            },
        };
        topTitle.Foreground = haze;
        credit.Background = new SolidColorBrush(palette.Glass);
        creditText.Foreground = haze;
        Group.Background = new AcrylicBrush
        {
            TintColor = ReaderPalette.WithAlpha(palette.Glass, 1),
            TintOpacity = palette.Glass.A / 255.0,
            TintLuminosityOpacity = 0.9,
            FallbackColor = ReaderPalette.WithAlpha(palette.Glass, 0.96),
        };
        Group.BorderBrush = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Rule, 0.6));
        title.Foreground = silk;
        line.Foreground = haze;
        leftNumber.Foreground = haze;
        rightNumber.Foreground = haze;
        ring.Stroke = silk;
        previewPage.Background = new SolidColorBrush(palette.Raised);
        previewPage.BorderBrush = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Rule, 0.8));
        previewImage.Dim = palette.Paper;
        previewPill.Background = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Glass, 0.96));
        previewNumber.Foreground = silk;
        foreach (var button in new[] { back, Chapters, Settings, Fullscreen })
            Tint(button, palette);
        Notice.Background = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Glass, 0.96));
        noticeText.Foreground = silk;
        Tint(noticeAction, palette);
        noticeAction.Background = new SolidColorBrush(palette.High);
        BackTo.Background = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Glass, 0.96));
        BackTo.Foreground = silk;
        Tint(BackTo, palette);
        BackTo.Resources["ButtonBackground"] = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Glass, 0.96));
        var res = scrubber.Resources;
        res["SliderTrackFill"] = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Silk, 0.25));
        res["SliderTrackFillPointerOver"] = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Silk, 0.3));
        res["SliderTrackFillPressed"] = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Silk, 0.3));
        res["SliderTrackValueFill"] = silk;
        res["SliderTrackValueFillPointerOver"] = silk;
        res["SliderTrackValueFillPressed"] = silk;
        res["SliderThumbBackground"] = silk;
        res["SliderThumbBackgroundPointerOver"] = silk;
        res["SliderThumbBackgroundPressed"] = silk;
        res["SliderOuterThumbBackground"] = new SolidColorBrush(palette.Raised);
    }

    static void Tint(Button button, ReaderPalette palette)
    {
        var text = new SolidColorBrush(palette.Silk);
        button.Foreground = text;
        var resources = button.Resources;
        resources["ButtonForeground"] = text;
        resources["ButtonForegroundPointerOver"] = text;
        resources["ButtonForegroundPressed"] = text;
        resources["ButtonBackgroundPointerOver"] = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Silk, 0.08));
        resources["ButtonBackgroundPressed"] = new SolidColorBrush(ReaderPalette.WithAlpha(palette.Silk, 0.05));
        resources["ButtonBorderBrush"] = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        resources["ButtonBorderBrushPointerOver"] = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        resources["ButtonBorderBrushPressed"] = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
    }

    /// <summary>The group is 640 wide, or the window less its margins.</summary>
    public void Fit(double width)
    {
        Group.Width = Math.Max(0, Math.Min(640, width - 32));
        windowWidth = width;
        Place();
    }

    double windowWidth;
    bool groupUp = true, noticeAtFoot;

    /// <summary>The group came up or went: the notice and Back to keep clear of it.</summary>
    public void GroupUp(bool up)
    {
        groupUp = up;
        Place();
    }

    /// <summary>
    /// A novel's notices sit in its bottom margin, as on the other apps, where they cover no
    /// text: above the group while it shows, 44 from the bottom once it goes. Pages keep the
    /// notice at the top centre.
    /// </summary>
    public void NoticeAtFoot(bool foot)
    {
        noticeAtFoot = foot;
        Place();
    }

    /// <summary>The height the group takes from the bottom, with its margin, while it shows.</summary>
    const double GroupRoom = 124;

    void Place()
    {
        if (noticeAtFoot)
        {
            Notice.VerticalAlignment = VerticalAlignment.Bottom;
            Notice.Margin = new Thickness(16, 0, 16, groupUp ? GroupRoom : 44);
        }
        else
        {
            Notice.VerticalAlignment = VerticalAlignment.Top;
            Notice.Margin = new Thickness(16, 56, 16, 0);
        }
        // Beside the group there is room for the pill only on a wide window; otherwise it sits
        // above the group while the group shows.
        var beside = (windowWidth - Group.Width) / 2 >= 200;
        BackTo.Margin = new Thickness(16, 0, 16, groupUp && !beside ? GroupRoom : 16);
    }

    public void Titles(string series, string? demoCredit)
    {
        title.Text = series;
        topTitle.Text = series;
        AutomationProperties.SetName(DragArea, series);
        creditText.Text = demoCredit ?? "";
        credit.Visibility = demoCredit is null ? Visibility.Collapsed : Visibility.Visible;
        topTitle.Visibility = demoCredit is null ? Visibility.Visible : Visibility.Collapsed;
    }

    public void Line(string text) => line.Text = text;

    public void FullscreenShown(bool on)
    {
        fullscreenIcon.Glyph = on ? "" : "";
        var name = on ? T.PlayerExitFullscreen : T.PlayerFullscreen;
        AutomationProperties.SetName(Fullscreen, name);
        ToolTipService.SetToolTip(Fullscreen, T.TooltipWithKey(name, "F11"));
    }

    // The scrubber

    int total = 1;
    bool reversed;

    /// <summary>
    /// Where the reader is, from 1, of <paramref name="count"/>. In a right-to-left book the
    /// track runs right to left: the total is on the left and where the reader is on the right.
    /// </summary>
    public void Scrub(int at, int count, bool rtl, int? ringAt)
    {
        total = Math.Max(count, 1);
        reversed = rtl;
        scrubRow.Visibility = count > 1 ? Visibility.Visible : Visibility.Collapsed;
        setting = true;
        scrubber.Maximum = total;
        scrubber.IsDirectionReversed = rtl;
        if (!scrubbing)
            scrubber.Value = Math.Clamp(at, 1, total);
        setting = false;
        ShowNumbers(scrubbing ? (int)scrubber.Value : at);
        if (ringAt is { } r && r != at && count > 1)
        {
            ringValue = Math.Clamp(r, 1, total);
            ring.Visibility = Visibility.Visible;
        }
        else
        {
            ringValue = null;
            ring.Visibility = Visibility.Collapsed;
        }
        PlaceRing();
    }

    int? ringValue;

    void ShowNumbers(int at)
    {
        var here = at.ToString(System.Globalization.CultureInfo.CurrentCulture);
        var all = total.ToString(System.Globalization.CultureInfo.CurrentCulture);
        leftNumber.Text = reversed ? all : here;
        rightNumber.Text = reversed ? here : all;
    }

    /// <summary>The ring of where the reader was before a jump, over the track.</summary>
    void PlaceRing()
    {
        if (ringValue is not { } r || scrubber.ActualWidth <= 0)
            return;
        // The track runs between the thumb's half-widths.
        const double thumb = 20;
        var fraction = total <= 1 ? 0 : (r - 1) / (double)(total - 1);
        if (reversed)
            fraction = 1 - fraction;
        var x = thumb / 2 + fraction * (scrubber.ActualWidth - thumb) - ring.Width / 2;
        Canvas.SetLeft(ring, x);
        Canvas.SetTop(ring, (scrubber.ActualHeight - ring.Height) / 2);
    }

    void EndScrub()
    {
        if (!scrubbing)
            return;
        scrubbing = false;
        HidePreview();
        // The page underneath waited for the release; now it crossfades to where the thumb is.
        owner.ScrubTo((int)Math.Round(scrubber.Value));
    }

    // The scrub preview

    /// <summary>
    /// The page under the thumb, above the group, following the thumb and kept 12 from the
    /// reader's edges. A novel has no pages to picture: its preview is the number alone.
    /// </summary>
    void ShowPreview(int value)
    {
        if (total <= 1)
            return;
        value = Math.Clamp(value, 1, total);
        var fresh = !previewShown;
        previewShown = true;
        if (value != previewAt || fresh)
        {
            previewAt = value;
            previewNumber.Text = value.ToString(System.Globalization.CultureInfo.CurrentCulture);
            Picture();
        }
        PlacePreview();
        if (!fresh)
            return;
        if (owner.PaperMode)
            Motion.Set(preview, true);
        else
            Motion.Show(preview);
    }

    /// <summary>The engine's file for the page under the thumb, or the Raised plate until it lands.</summary>
    void Picture()
    {
        var (file, pictured) = owner.PreviewOf(previewAt);
        previewPage.Visibility = pictured ? Visibility.Visible : Visibility.Collapsed;
        if (file is not null)
        {
            previewImage.Visibility = Visibility.Visible;
            previewImage.Show(file, owner.Scale);
        }
        else
            previewImage.Visibility = Visibility.Collapsed;
    }

#if KAERI_TESTING
    /// <summary>Shots: the preview as a drag to <paramref name="value"/> shows it, with no pointer.</summary>
    internal void ShotPreview(int value)
    {
        scrubbing = true;
        ShowPreview(value);
    }
#endif

    /// <summary>A page landed while the preview was up: it shows it.</summary>
    public void RefreshPreview()
    {
        if (scrubbing && previewShown && previewAt > 0)
            Picture();
    }

    void PlacePreview()
    {
        if (scrubber.ActualWidth <= 0 || PreviewLayer.ActualWidth <= 0 || previewAt < 1)
            return;
        // The thumb's centre, as PlaceRing finds the ring's.
        const double thumb = 20;
        var fraction = total <= 1 ? 0 : (previewAt - 1) / (double)(total - 1);
        if (reversed)
            fraction = 1 - fraction;
        var x = thumb / 2 + fraction * (scrubber.ActualWidth - thumb);
        var centre = scrubber.TransformToVisual(PreviewLayer).TransformPoint(new global::Windows.Foundation.Point(x, 0));
        var groupTop = Group.TransformToVisual(PreviewLayer).TransformPoint(new global::Windows.Foundation.Point(0, 0)).Y;
        var height = previewPage.Visibility == Visibility.Visible ? PreviewHeight + preview.Spacing + PillHeight : PillHeight;
        var left = Math.Clamp(centre.X - PreviewWidth / 2, 12, Math.Max(12, PreviewLayer.ActualWidth - PreviewWidth - 12));
        Canvas.SetLeft(preview, left);
        Canvas.SetTop(preview, Math.Max(12, groupTop - height - 8));
    }

    void HidePreview()
    {
        if (!previewShown)
            return;
        previewShown = false;
        previewAt = -1;
        if (owner.PaperMode)
            Motion.Set(preview, false);
        else
            Motion.Hide(preview);
    }

    public bool Scrubbing => scrubbing;

    // Notices

    public void ShowNotice(ReaderNoticeLine? notice)
    {
        if (notice is null)
        {
            noticeKind = null;
            if (!owner.PaperMode)
                Motion.Hide(Notice);
            else
                Motion.Set(Notice, false);
            return;
        }
        noticeText.Text = notice.Text;
        noticeKind = notice.Action;
        noticeAction.Content = notice.Action switch
        {
            ReaderAction.StartOver => T.WindowsReaderStartOver,
            ReaderAction.GoBack => T.WindowsReaderGoBack,
            ReaderAction.ChangeMode => T.WindowsReaderChange,
            _ => null,
        };
        noticeAction.Visibility = noticeAction.Content is null ? Visibility.Collapsed : Visibility.Visible;
        if (!owner.PaperMode)
            Motion.Show(Notice);
        else
            Motion.Set(Notice, true);
        // Narrator says every notice, once.
        ReaderSpeech.Raise(noticeText);
    }

    public void ShowBackTo(ReaderNoticeLine? line)
    {
        if (line is null)
        {
            Motion.Set(BackTo, false);
            return;
        }
        BackTo.Content = line.Text;
        AutomationProperties.SetName(BackTo, line.Text);
        if (!owner.PaperMode)
            Motion.Show(BackTo);
        else
            Motion.Set(BackTo, true);
        ReaderSpeech.Say(BackTo, line.Text, "kaeri-reader-back-to");
    }

    /// <summary>Keyboard focus or the pointer is in the chrome: it stays up.</summary>
    public bool Holds(XamlRoot? root)
    {
        if (scrubbing)
            return true;
        if (root is null || FocusManager.GetFocusedElement(root) is not Control { FocusState: FocusState.Keyboard } focused)
            return false;
        for (DependencyObject? at = focused; at is not null; at = VisualTreeHelper.GetParent(at))
            if (at == Group || at == Top)
                return true;
        return false;
    }
}
