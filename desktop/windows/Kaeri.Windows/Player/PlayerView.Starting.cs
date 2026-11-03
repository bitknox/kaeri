using System.Diagnostics;
using System.Globalization;
using Kaeri.App;
using Kaeri.Engine.Core;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Kaeri.Windows.Pages;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using global::Windows.Graphics.Imaging;
using global::Windows.Storage.Streams;

namespace Kaeri.Windows.Player;

/// <summary>
/// The starting screen (the starting-screen board). The engine says what a start is doing
/// (<see cref="Event.StartingStep"/>) and the <see cref="StartScreen"/> model decides what the
/// screen shows at each moment; this draws it: the step line and its detail crossfading in place,
/// the way out arriving by time, the 2 px line, Details, and a failure's screen. A start that
/// has not failed says so only when the engine says it ended.
/// </summary>
public sealed partial class PlayerView
{
    StartScreen? start;
    (uint Id, string Episode, double? Resume)? staged;
    FadingText? stepText, detailText;
    DispatcherQueueTimer? startTick;
    readonly Dictionary<StartAction, Button> startButtons = [];
    string startButtonsKey = "";
    string startRowsKey = "";
    readonly List<TextBlock> startRowTimes = [];
    string loaderMode = "";
    double loaderWidth;
    string startKey = "";
    bool startFailedSaid;
    bool startingDemo, movingOn;
    /// <summary>A screenshot run's: the screen frozen at this moment.</summary>
    TimeSpan? startFrozen;

    /// <summary>Whether a start is on screen: its alert is the failure screen's to say.</summary>
    internal bool IsStarting => startingShown && start is not null;

    TimeSpan StartElapsed => startFrozen ?? Stopwatch.GetElapsedTime(startingFrom);

    // What the engine says.

    void HeardStart(Event e)
    {
        switch (e)
        {
            case Event.PlaybackStaged st:
                staged = (st.Id, st.Episode, st.Resume);
                if (start is { } open && open.Id == st.Id && open.Episode == st.Episode)
                    open.ResumeAt = st.Resume;
                break;
            case Event.StartingStep step when start is { } s && s.Id == step.Id && s.Episode == step.Episode:
                s.Step(step.Step, step.Line, StartElapsed);
                RefreshStart();
                break;
            case Event.PlaybackEnded { Failure: { } failure, RollingOn: false } when start is { } s:
                s.Fail(failure);
                RefreshStart();
                break;
            case Event.Connectivity c when start is { } s:
                s.Connectivity(c.Offline, StartElapsed);
                RefreshStart();
                break;
        }
    }

    // The screen opens.

    void BeginStart(PlayerUi.Starting s)
    {
        var resume = staged is { } st && st.Id == s.Id && st.Episode == s.EpisodeValue ? st.Resume : null;
        start = new StartScreen(s.Id, s.EpisodeValue, resume) { DetailsOpen = DetailsRemembered() };
        startingEpisode = s.EpisodeValue;
        startingDemo = ctx is { } k && (k.Known.DemoPlay || k.Known.Entries.TryGetValue(s.Id, out var shown) && shown.IsDemo);
        if (ctx is not null)
            ctx.Known.DemoPlay = false;
        if (ctx?.Known.Offline == true)
            start.Connectivity(true, TimeSpan.Zero);
        startFrozen = null;
        startFailedSaid = false;
        startKey = "";
        startButtons.Clear();
        StartingButtons.Children.Clear();
        startButtonsKey = startRowsKey = "";
        StartingRows.Children.Clear();
        startRowTimes.Clear();
        StartingShow.Text = s.Title;
        StartingEpisode.Text = Lines.EpisodeNamed(s.EpisodeValue, s.EpisodeTitle);
        StartingCredit.Visibility = startingDemo ? Visibility.Visible : Visibility.Collapsed;
        StartingLoader.Opacity = 1;
        stepText = new FadingText(StepA, StepB);
        detailText = new FadingText(DetailA, DetailB);
        loaderMode = "";
        StartingArt.Source = null;
        StartingGround.Opacity = StartGround.Normal;
        var art = s.BannerUrl ?? s.CoverUrl;
        ctx?.Art.Bind(art, image => StartingArt.Source = image);
        if (art is { Length: > 0 })
            ctx?.Art.Bytes(art, bytes => _ = MeasureGround(bytes));
        ShowStarting();
        var frame = start.Frame(TimeSpan.Zero);
        stepText.Reset(frame.Step, frame.Key);
        detailText.Reset(frame.Detail, frame.Lead);
        ShowFrame(frame);
        Say(T.WindowsStartAnnounce(start.Number, s.Title), assertive: false);
        startTick ??= MakeStartTick();
        startTick.Start();
    }

    DispatcherQueueTimer MakeStartTick()
    {
        var timer = DispatcherQueue.GetForCurrentThread().CreateTimer();
        timer.Interval = TimeSpan.FromMilliseconds(100);
        timer.Tick += (_, _) => RefreshStart();
        return timer;
    }

    void ShowStarting()
    {
        startingShown = true;
        startingFrom = Stopwatch.GetTimestamp();
        if (movingOn)
        {
            // Moving on to the next episode: the last frame fades to the ground over 0.25 s (T5).
            movingOn = false;
            Starting.Opacity = 0;
            Motion.FadeIn(Starting, TimeSpan.FromMilliseconds(250));
        }
        else
            Motion.Set(Starting, true);
    }

    /// <summary>The app is closing: the clock, the loader and the crossfades stop before the window goes.</summary>
    internal void Quiet()
    {
        EndStart();
        Starting.Visibility = Visibility.Collapsed;
    }

    /// <summary>The start is over, by a first frame, a close or a new playback: the clock stops.</summary>
    void EndStart()
    {
        startTick?.Stop();
        loaderMode = "";
        StopLoader();
    }

    /// <summary>The ground by the art's luminance: 55%, or 80% over art that is bright.</summary>
    async Task MeasureGround(byte[] bytes)
    {
        try
        {
            using var stream = new InMemoryRandomAccessStream();
            await stream.WriteAsync(System.Runtime.InteropServices.WindowsRuntime.WindowsRuntimeBufferExtensions.AsBuffer(bytes));
            stream.Seek(0);
            var decoder = await BitmapDecoder.CreateAsync(stream);
            var transform = new BitmapTransform { ScaledWidth = 128, ScaledHeight = Math.Max(1, (uint)(128.0 * decoder.PixelHeight / Math.Max(1, decoder.PixelWidth))) };
            var data = await decoder.GetPixelDataAsync(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Ignore, transform, ExifOrientationMode.IgnoreExifOrientation, ColorManagementMode.DoNotColorManage);
            var luminance = StartGround.Luminance(data.DetachPixelData(), (int)transform.ScaledWidth, (int)transform.ScaledHeight);
            Motion.FadeTo(StartingGround, StartGround.For(luminance), Motion.Standard);
        }
        catch (Exception)
        {
            // Art this cannot read keeps the usual ground.
        }
    }

    // The clock and the frame.

    void RefreshStart()
    {
        if (start is not { } s || !startingShown)
            return;
        var at = StartElapsed;
        s.Tick(at);
        ShowFrame(s.Frame(at));
        if (s.SlowAnnouncement(at) is { } slow)
            Say(slow, assertive: false);
    }

    void ShowFrame(StartFrame frame)
    {
        if (stepText is null || detailText is null)
            return;
        var failed = frame.Failure is not null;
        var step = frame.Failure?.Title ?? frame.Step;
        var key = failed ? "failure" : frame.Key;
        var detail = frame.Failure?.Reason ?? frame.Detail;
        stepText.Set(step, key);
        detailText.Set(detail, failed ? "failure" : frame.Lead);
        AutomationProperties.SetName(StartingColumn, T.WindowsStartStatus(step, detail));
        if (failed && !startFailedSaid)
        {
            startFailedSaid = true;
            Say(T.WindowsStartFailedAnnounce(step, detail), assertive: true);
            ShowPointer();
        }
        else if (!failed && key != startKey && key != "buffering")
            Say(step, assertive: false);
        startKey = key;
        ShowLoader(failed ? "gone" : frame.Percent is { } ? "determinate" : "going", frame.Percent);
        ShowButtons(frame);
        ShowDetails(frame);
    }

    // The 2 px line: silk at 14% and a 32% stretch of amber that moves across and back, or, with
    // animation effects off, rests at the start and breathes between 100% and 40%.

    Storyboard? loaderBoard;

    void OnLoaderSized(object sender, SizeChangedEventArgs e)
    {
        loaderWidth = e.NewSize.Width;
        var mode = loaderMode;
        loaderMode = "";
        ShowLoader(mode, lastPercent);
    }

    uint? lastPercent;

    void ShowLoader(string mode, uint? percent)
    {
        if (loaderWidth <= 0)
        {
            loaderMode = mode;
            lastPercent = percent;
            return;
        }
        if (mode == "going" && !Motion.Enabled)
            mode = "resting";
        if (mode == "gone")
        {
            if (loaderMode != "gone")
                Motion.FadeTo(StartingLoader, 0, Motion.Standard);
            loaderMode = "gone";
            return;
        }
        if (loaderMode == "gone")
            Motion.FadeTo(StartingLoader, 1, Motion.Standard);
        if (mode == "determinate")
        {
            var width = loaderWidth * Math.Clamp((percent ?? 0) / 100.0, 0, 1);
            if (loaderMode != "determinate")
            {
                StopLoader();
                LoaderStretch.Opacity = 1;
                LoaderShift.X = 0;
            }
            var from = LoaderStretch.Width is { } w and > 0 && !double.IsNaN(w) ? w : 0;
            Motion.Ease(LoaderStretch, from, width, value => LoaderStretch.Width = value);
        }
        else if (mode != loaderMode)
        {
            StopLoader();
            LoaderStretch.Width = loaderWidth * 0.32;
            loaderBoard = new Storyboard();
            if (mode == "resting")
            {
                // Reduced motion: no travel; the stretch breathes.
                LoaderShift.X = 0;
                loaderBoard.Children.Add(Loop(LoaderStretch, "Opacity", 1, 0.4, TimeSpan.FromSeconds(1), new SineEase { EasingMode = EasingMode.EaseInOut }));
            }
            else
            {
                LoaderStretch.Opacity = 1;
                loaderBoard.Children.Add(Loop(LoaderShift, "X", 0, loaderWidth * 0.68, TimeSpan.FromSeconds(1.4), new SineEase { EasingMode = EasingMode.EaseInOut }));
            }
            loaderBoard.Begin();
        }
        loaderMode = mode;
        lastPercent = percent;
    }

    static DoubleAnimation Loop(DependencyObject target, string property, double from, double to, TimeSpan time, EasingFunctionBase ease)
    {
        var animation = new DoubleAnimation
        {
            From = from,
            To = to,
            Duration = time,
            AutoReverse = true,
            RepeatBehavior = RepeatBehavior.Forever,
            EasingFunction = ease,
            EnableDependentAnimation = true,
        };
        Storyboard.SetTarget(animation, target);
        Storyboard.SetTargetProperty(animation, property);
        return animation;
    }

    void StopLoader()
    {
        loaderBoard?.Stop();
        loaderBoard = null;
    }

    // The way out: buttons arrive at the trailing end of the row, each fading in over 200 ms.

    void ShowButtons(StartFrame frame)
    {
        var wanted = frame.Buttons.Select(b => b.Action).ToHashSet();
        foreach (var spec in frame.Buttons)
        {
            if (!startButtons.TryGetValue(spec.Action, out var button))
            {
                button = MakeStartButton(spec);
                startButtons[spec.Action] = button;
                button.Opacity = 0;
                StartingButtons.Children.Add(button);
                Motion.FadeIn(button);
            }
            else if (button.Visibility == Visibility.Collapsed)
                Motion.FadeIn(button);
            DressStartButton(button, spec, frame.DetailsOpen);
        }
        foreach (var (action, button) in startButtons)
            if (!wanted.Contains(action) && button.Visibility == Visibility.Visible)
                Motion.Hide(button);
        // The order of the row is the model's.
        var order = string.Join(",", frame.Buttons.Select(b => b.Action));
        if (order != startButtonsKey)
        {
            startButtonsKey = order;
            var focus = FocusManager.GetFocusedElement(XamlRoot) as Button;
            var sequence = frame.Buttons.Select(b => startButtons[b.Action]).Concat(startButtons.Where(p => !wanted.Contains(p.Key)).Select(p => p.Value)).ToList();
            if (!StartingButtons.Children.SequenceEqual(sequence))
            {
                StartingButtons.Children.Clear();
                foreach (var button in sequence)
                    StartingButtons.Children.Add(button);
                focus?.Focus(FocusState.Programmatic);
            }
        }
        if (frame.Failure is not null && FocusManager.GetFocusedElement(XamlRoot) is not Button && frame.Buttons.Count > 0)
            startButtons[frame.Buttons[0].Action].Focus(FocusState.Programmatic);
    }

    Button MakeStartButton(StartButton spec)
    {
        var button = new Button { MinHeight = 34, Padding = new Thickness(14, 0, 14, 0), FontWeight = Microsoft.UI.Text.FontWeights.Bold };
        var action = spec.Action;
        button.Click += (_, _) => StartPressed(action);
        return button;
    }

    void DressStartButton(Button button, StartButton spec, bool detailsOpen)
    {
        if (spec.Action == StartAction.Details)
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
            row.Children.Add(new TextBlock { Text = spec.Label });
            row.Children.Add(new FontIcon { Glyph = detailsOpen ? "" : "", FontSize = 10, VerticalAlignment = VerticalAlignment.Center });
            if (button.Tag as string != $"{spec.Look}{detailsOpen}{spec.Label}")
                button.Content = row;
            AutomationProperties.SetName(button, spec.Label);
        }
        else if (button.Content as string != spec.Label)
            button.Content = spec.Label;
        var look = $"{spec.Look}{detailsOpen}{spec.Label}";
        if (button.Tag as string == look)
            return;
        button.Tag = look;
        switch (spec.Look)
        {
            case StartLook.Primary:
                button.Style = Ui.Res<Style>("AccentButtonStyle");
                button.ClearValue(Control.BackgroundProperty);
                button.ClearValue(Control.BorderThicknessProperty);
                break;
            case StartLook.Quiet:
                button.ClearValue(FrameworkElement.StyleProperty);
                button.Background = new SolidColorBrush(global::Windows.UI.Color.FromArgb(0, 0, 0, 0));
                button.BorderThickness = new Thickness(0);
                break;
            default:
                button.ClearValue(FrameworkElement.StyleProperty);
                button.ClearValue(Control.BackgroundProperty);
                button.ClearValue(Control.BorderThicknessProperty);
                break;
        }
    }

    void StartPressed(StartAction action)
    {
        if (start is not { } s)
            return;
        switch (action)
        {
            case StartAction.Cancel or StartAction.Close:
                Close();
                break;
            case StartAction.FromBeginning:
                Send(new PlayerCommand.PlayFromBeginning());
                s.BeginFromStart();
                RefreshStart();
                break;
            case StartAction.Skip:
                Send(new PlayerCommand.SkipSource());
                break;
            case StartAction.ChooseSource or StartAction.ChooseAnother:
                if (ctx is { } c && XamlRoot is { } root)
                    _ = PartySourceDialog.ForEpisode(c, root, s.Id, s.Episode).ShowAsync();
                break;
            case StartAction.Details:
                s.DetailsOpen = !s.DetailsOpen;
                RememberDetails(s.DetailsOpen);
                RefreshStart();
                break;
            case StartAction.ShowDownloads:
                Close();
                ctx?.Go("downloads");
                break;
            case StartAction.TryAgain:
                ctx?.Play(s.Id, s.Episode);
                break;
            case StartAction.FixMatch:
                if (ctx is { } m && XamlRoot is { } r)
                    _ = MatchPickerDialog.Open(m, s.Id, StartingShow.Text, r, false);
                break;
            case StartAction.AddStore:
                Close();
                ctx?.Go("stores");
                break;
        }
    }

    // Details: what each source said, and the audio and subtitles this start will use.

    void ShowDetails(StartFrame frame)
    {
        var open = frame.DetailsOpen;
        if (open && StartingDetails.Visibility != Visibility.Visible)
            Motion.FadeIn(StartingDetails, Motion.Standard);
        else if (!open && StartingDetails.Visibility == Visibility.Visible)
            Motion.HideStandard(StartingDetails);
        var structure = string.Join("|", frame.Rows.Select(r => $"{r.Mark}/{r.Name}/{r.Note}/{r.Skip}"));
        if (structure != startRowsKey)
        {
            startRowsKey = structure;
            StartingRows.Children.Clear();
            startRowTimes.Clear();
            foreach (var row in frame.Rows)
                StartingRows.Children.Add(MakeRow(row));
        }
        else
            for (var i = 0; i < frame.Rows.Count && i < startRowTimes.Count; i++)
                startRowTimes[i].Text = frame.Rows[i].Time;
        var known = frame.Translation is not null;
        StartingLanguage.Visibility = known ? Visibility.Visible : Visibility.Collapsed;
        if (frame.Translation is { } translation)
        {
            StartingSub.IsChecked = translation == Kaeri.Engine.Core.Translation.Sub;
            StartingDub.IsChecked = translation == Kaeri.Engine.Core.Translation.Dub;
        }
    }

    UIElement MakeRow(StartRow row)
    {
        var grid = new Grid { MinHeight = 28, ColumnSpacing = 10 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var silk = Ui.Res<SolidColorBrush>("SilkBrush").Color;
        var amber = Ui.Res<SolidColorBrush>("AmberBrush").Color;
        var dot = row.Mark switch
        {
            StartMark.Now => Ui.Dot(amber),
            StartMark.Empty => Ui.Dot(Ui.Vermilion),
            StartMark.Rest => Ui.Dot(silk, ring: true),
            _ => Ui.Dot(silk),
        };
        dot.Width = dot.Height = 8;
        dot.Opacity = row.Mark == StartMark.Done ? 0.6 : 1;
        dot.VerticalAlignment = VerticalAlignment.Center;
        grid.Children.Add(dot);
        var text = new TextBlock { VerticalAlignment = VerticalAlignment.Center, TextWrapping = TextWrapping.WrapWholeWords };
        text.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = row.Name, Foreground = Ui.Res<SolidColorBrush>("SilkBrush") });
        if (row.Note.Length > 0)
            text.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = $" · {row.Note}", Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush") });
        Grid.SetColumn(text, 1);
        grid.Children.Add(text);
        var time = new TextBlock
        {
            Text = row.Time,
            VerticalAlignment = VerticalAlignment.Center,
            Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush"),
        };
        Microsoft.UI.Xaml.Documents.Typography.SetNumeralAlignment(time, FontNumeralAlignment.Tabular);
        Grid.SetColumn(time, 2);
        grid.Children.Add(time);
        startRowTimes.Add(time);
        if (row.Skip)
        {
            var skip = new Button { Content = T.WindowsStartRowSkip, MinHeight = 24, Padding = new Thickness(8, 0, 8, 0) };
            skip.Click += (_, _) => StartPressed(StartAction.Skip);
            Grid.SetColumn(skip, 3);
            grid.Children.Add(skip);
        }
        return grid;
    }

    void OnStartingSub(object sender, RoutedEventArgs e) => ChooseStartTranslation(Kaeri.Engine.Core.Translation.Sub);
    void OnStartingDub(object sender, RoutedEventArgs e) => ChooseStartTranslation(Kaeri.Engine.Core.Translation.Dub);

    void ChooseStartTranslation(Kaeri.Engine.Core.Translation translation)
    {
        StartingSub.IsChecked = translation == Kaeri.Engine.Core.Translation.Sub;
        StartingDub.IsChecked = translation == Kaeri.Engine.Core.Translation.Dub;
        Send(new PlayerCommand.Choose(new PlayerChoice.Translation(translation)));
    }

    /// <summary>The column is 520 wide, less the margins in a narrow window.</summary>
    void OnStartingSized(object sender, SizeChangedEventArgs e) =>
        StartingColumn.Width = Math.Max(240, Math.Min(520, e.NewSize.Width - 48));

    // What a screen reader hears.

    void Say(string text, bool assertive)
    {
        AutomationProperties.SetLiveSetting(StartingAnnouncer, assertive ? AutomationLiveSetting.Assertive : AutomationLiveSetting.Polite);
        StartingAnnouncer.Text = text;
        Announce(StartingAnnouncer);
    }

    // Details stays as the viewer left it, on this device.

    static bool? detailsKept;

    string DetailsFile => System.IO.Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "kaeri", "data", "windows-start-details");

    bool DetailsRemembered()
    {
        // A fixture run keeps nothing on the profile.
        if (ctx?.Options.Fixture == true)
            return detailsKept ?? false;
        return detailsKept ??= SmallFiles.Read(DetailsFile) == "open";
    }

    void RememberDetails(bool open)
    {
        detailsKept = open;
        if (ctx?.Options.Fixture != true)
            SmallFiles.Write(DetailsFile, open ? "open" : "closed");
    }

    /// <summary>Text that changes in place: the new text fades in over the old, which fades away.</summary>
    sealed class FadingText(TextBlock first, TextBlock second)
    {
        TextBlock on = first, off = second;
        string text = "", key = "";

        /// <summary>The text there from the start, with no fade.</summary>
        public void Reset(string value, string identity)
        {
            on.Text = value;
            on.Opacity = value.Length > 0 ? 1 : 0;
            off.Text = "";
            off.Opacity = 0;
            text = value;
            key = identity;
        }

        public void Set(string value, string identity)
        {
            if (value == text && identity == key)
                return;
            if (text.Length == 0)
            {
                // Arriving: it fades in where it will stay.
                on.Text = value;
                Motion.FadeTo(on, value.Length > 0 ? 1 : 0, Motion.FadeInTime);
            }
            else if (value.Length == 0)
                Motion.FadeTo(on, 0, Motion.Standard);
            else if (identity == key)
                on.Text = value;
            else
            {
                off.Text = value;
                Motion.Crossfade(on, off);
                (on, off) = (off, on);
            }
            text = value;
            key = identity;
        }
    }

#if KAERI_TESTING
    /// <summary>
    /// <c>--show=starting-&lt;state&gt;[-&lt;seconds&gt;][-reduced]</c>: the engine's made-up start in that state
    /// (the names of <see cref="StartingFixture"/>, kebab-cased), played on its own clock, or with
    /// a number of seconds frozen at that moment for a screenshot.
    /// </summary>
    internal async Task StartDemo(string what)
    {
        try
        {
            await StartDemoRun(what);
        }
        catch (Exception e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Error, "windows", $"the starting demo failed: {e}");
        }
    }

    async Task StartDemoRun(string what)
    {
        KaeriFfiMethods.LogLine(FfiLogLevel.Info, "windows", $"starting demo {what}");
        var rest = what["starting-".Length..];
        if (rest.EndsWith("-reduced", StringComparison.Ordinal))
        {
            Motion.Override = false;
            rest = rest[..^"-reduced".Length];
        }
        double? freeze = null;
        var cut = rest.LastIndexOf('-');
        if (cut > 0 && double.TryParse(rest[(cut + 1)..], NumberStyles.Float, CultureInfo.InvariantCulture, out var seconds))
        {
            freeze = seconds;
            rest = rest[..cut];
        }
        StartingFixture? state = rest switch
        {
            "title-lookup" => StartingFixture.TitleLookup,
            "sources-loading" => StartingFixture.SourcesLoading,
            "asking-first" => StartingFixture.AskingFirst,
            "asking-pinned" => StartingFixture.AskingPinned,
            "source-missed" => StartingFixture.SourceMissed,
            "source-failed" => StartingFixture.SourceFailed,
            "sources-resting" => StartingFixture.SourcesResting,
            "slow" => StartingFixture.Slow,
            "finding-peers" => StartingFixture.FindingPeers,
            "peers-known" => StartingFixture.PeersKnown,
            "found" => StartingFixture.Found,
            "fetching-subtitles" => StartingFixture.FetchingSubtitles,
            "another-stream" => StartingFixture.AnotherStream,
            "buffering-torrent" => StartingFixture.BufferingTorrent,
            "buffering-direct" => StartingFixture.BufferingDirect,
            "offline" => StartingFixture.Offline,
            "failed-no-episode" => StartingFixture.FailedNoEpisode,
            "failed-no-match" => StartingFixture.FailedNoMatch,
            "failed-no-sources" => StartingFixture.FailedNoSources,
            "failed-not-aired" => StartingFixture.FailedNotAired,
            _ => null,
        };
        if (state is not { } fixture)
            return;
        startingHeld = true;
        // The player opens for the start as the engine's PlayerUi.Starting makes it, with the art of
        // the scene's first title.
        var art = ctx?.Known.Entries.Values.FirstOrDefault();
        Heard([new Event.PlayerUi(1, new PlayerUi.Starting(900_001, "The Lamp Road", "7", "The Lamp Seller", art?.CoverUrl, art?.BannerUrl))]);
        var events = KaeriFfiMethods.StartingFixture(fixture);
        var began = Stopwatch.GetTimestamp();
        foreach (var item in events.OrderBy(e => e.AtMs))
        {
            if (freeze is { } f)
            {
                if (item.AtMs > f * 1000)
                    break;
                // Said at its own moment on the screen's clock.
                if (start is not null)
                    startingFrom = Stopwatch.GetTimestamp() - (long)(item.AtMs / 1000.0 * Stopwatch.Frequency);
                Heard([item.Event]);
                continue;
            }
            var wait = TimeSpan.FromMilliseconds(item.AtMs) - Stopwatch.GetElapsedTime(began);
            if (wait > TimeSpan.Zero)
                await Task.Delay(wait);
            Heard([item.Event]);
        }
        if (freeze is { } at)
        {
            startFrozen = TimeSpan.FromSeconds(at);
            startTick?.Stop();
            RefreshStart();
        }
    }
#endif
}
