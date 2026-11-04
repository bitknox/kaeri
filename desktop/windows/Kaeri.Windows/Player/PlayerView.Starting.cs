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
using Microsoft.UI.Xaml.Shapes;
using global::Windows.Graphics.Imaging;
using global::Windows.Storage.Streams;

namespace Kaeri.Windows.Player;

/// <summary>
/// The starting screen (the starting-screen board). The engine says what a start is doing
/// (<see cref="Event.StartingStep"/>) and the <see cref="StartScreen"/> model decides what the
/// screen shows at each moment; this draws it: the step line and its detail crossfading in place,
/// the way out arriving by time in room kept for it, the 2 px line, Details, and a failure's
/// screen. Nothing in the column moves once a start is on screen: its top is set when the start
/// begins, and what comes later grows downward.
/// </summary>
public sealed partial class PlayerView
{
    StartScreen? start;
    (uint Id, string Episode, double? Resume)? staged;
    FadingText? stepText, detailText;
    DispatcherQueueTimer? startTick;
    readonly Dictionary<StartAction, Button> startButtons = [];
    string startOfflineKey = "", startFailingKey = "";
    string startRowsKey = "";
    readonly List<TextBlock> startRowTimes = [];
    string loaderMode = "";
    double loaderWidth;
    bool startFailedSaid;
    bool startingDemo, movingOn;
    /// <summary>The art and its ground: the art fades in once both are known.</summary>
    bool startArtLoaded, startGroundKnown;
    /// <summary>The column's height when the start began, Details closed: where its top goes.</summary>
    double startColumnHeight;
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
                {
                    // The screen can open before the engine says where it resumes: the row then
                    // gets its Play from the beginning slot, and the kept room with it.
                    var had = open.Resumes;
                    open.ResumeAt = st.Resume;
                    if (open.Resumes != had)
                    {
                        BuildStartRows(open.Resumes);
                        RefreshStart();
                    }
                }
                break;
            case Event.StartingStep step when start is { } s && s.Id == step.Id && s.Episode == step.Episode:
                s.Step(step.Step, step.Line, StartElapsed);
                if (step.Step is StartStep.WaitingForConnection)
                    Say(T.PlayerStartOfflineDetail(s.Episode), assertive: false);
                RefreshStart();
                break;
            case Event.PlaybackEnded { Failure: { } failure, RollingOn: false } when start is { } s:
                s.Fail(failure);
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
        startFrozen = null;
        startFailedSaid = false;
        StartingShow.Text = s.Title;
        StartingEpisode.Text = Lines.EpisodeNamed(s.EpisodeValue, s.EpisodeTitle);
        StartingCredit.Visibility = startingDemo ? Visibility.Visible : Visibility.Collapsed;
        StartingLoader.Opacity = 1;
        stepText = new FadingText(new(StepRowA, StepA, StepDotA), new(StepRowB, StepB, StepDotB));
        detailText = new FadingText(new(DetailA, DetailA, null), new(DetailB, DetailB, null));
        loaderMode = "";
        BuildStartRows(start.Resumes);
        StartingRows.Children.Clear();
        startRowTimes.Clear();
        startRowsKey = "";
        Motion.Set(StartingDetails, false);
        // The art waits for its ground, then fades in over it.
        startArtLoaded = startGroundKnown = false;
        StartingArt.Source = null;
        StartingArt.Opacity = 0;
        StartingGround.Opacity = StartGround.Normal;
        var art = s.BannerUrl ?? s.CoverUrl;
        ctx?.Art.Bind(art, image =>
        {
            StartingArt.Source = image;
            startArtLoaded = true;
            RevealStartArt();
        });
        if (art is { Length: > 0 })
            ctx?.Art.Bytes(art, bytes => _ = MeasureGround(bytes));
        else
            startGroundKnown = true;
        ShowStarting();
        var frame = start.Frame(TimeSpan.Zero);
        stepText.Reset(frame.Step);
        detailText.Reset(frame.Detail);
        ShowFrame(frame);
        PlaceStartColumn(measure: true);
        Say(T.PlayerStartAnnounce(start.Episode, s.Title), assertive: false);
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

    /// <summary>
    /// The app is closing. The window disposes the player next, so every clock that reaches it
    /// stops here and the view lets go of it: a tick already queued then finds no player, where
    /// a disposed one would throw inside the timer's callback and end the process.
    /// </summary>
    internal void Quiet()
    {
        EndStart();
        Starting.Visibility = Visibility.Collapsed;
        tick.Stop();
        idle.Stop();
        panelPace?.Stop();
        ClosePreview();
        player = null;
    }

    /// <summary>The start is over, by a first frame, a close or a new playback: the clock and the line stop.</summary>
    void EndStart()
    {
        startTick?.Stop();
        loaderMode = "";
        StopLoader();
    }

    /// <summary>The ground by the art's luminance: 55%, or 80% over art that is bright. Set before the art shows.</summary>
    async Task MeasureGround(byte[] bytes)
    {
        try
        {
            using var stream = new InMemoryRandomAccessStream();
            await stream.WriteAsync(System.Runtime.InteropServices.WindowsRuntime.WindowsRuntimeBufferExtensions.AsBuffer(bytes));
            stream.Seek(0);
            var decoder = await BitmapDecoder.CreateAsync(stream);
            var transform = new BitmapTransform { ScaledWidth = 64, ScaledHeight = Math.Max(1, (uint)(64.0 * decoder.PixelHeight / Math.Max(1, decoder.PixelWidth))) };
            var data = await decoder.GetPixelDataAsync(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Ignore, transform, ExifOrientationMode.IgnoreExifOrientation, ColorManagementMode.DoNotColorManage);
            var luminance = StartGround.Luminance(data.DetachPixelData(), (int)transform.ScaledWidth, (int)transform.ScaledHeight);
            StartingGround.Opacity = StartGround.For(luminance);
        }
        catch (Exception)
        {
            // Art this cannot read keeps the usual ground.
        }
        startGroundKnown = true;
        RevealStartArt();
    }

    void RevealStartArt()
    {
        if (startArtLoaded && startGroundKnown && StartingArt.Opacity < 1)
            Motion.FadeTo(StartingArt, 1, Motion.FadeInTime);
    }

    // Where the column sits: its top set when the start begins, centred on the column as it is
    // then with the room of every button it can show, and kept while the start lasts.

    void PlaceStartColumn(bool measure)
    {
        var (width, height) = (Starting.ActualWidth, Starting.ActualHeight);
        if (width <= 0 || height <= 0)
            (width, height) = (ActualWidth, ActualHeight);
        if (width <= 0 || height <= 0)
            return;
        StartingColumn.Width = Math.Max(240, Math.Min(520, width - 48));
        if (measure || startColumnHeight <= 0)
        {
            var open = StartingDetails.Visibility == Visibility.Visible;
            StartingDetails.Visibility = Visibility.Collapsed;
            StartingColumn.Measure(new global::Windows.Foundation.Size(StartingColumn.Width, double.PositiveInfinity));
            startColumnHeight = StartingColumn.DesiredSize.Height;
            if (open)
                StartingDetails.Visibility = Visibility.Visible;
        }
        var top = Math.Max(48, (height - startColumnHeight) / 2);
        StartingColumn.Margin = new Thickness(0, top, 0, 24);
    }

    /// <summary>The column follows the window's size, from the height it had when the start began.</summary>
    void OnStartingSized(object sender, SizeChangedEventArgs e) => PlaceStartColumn(measure: startColumnHeight <= 0);

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
        var detail = frame.Failure?.Reason ?? frame.Detail;
        stepText.Set(step, dot: failed, two: false);
        detailText.Set(detail, dot: false, two: failed);
        AutomationProperties.SetName(StartingColumn, detail.Length == 0 ? step : T.PlayerStartStatus(step, detail));
        if (failed && !startFailedSaid)
        {
            startFailedSaid = true;
            Say(T.PlayerStartFailedAnnounce(step, detail), assertive: true);
            ShowPointer();
        }
        ShowLoader(failed ? "gone" : frame.Percent is { } ? "determinate" : "going", frame.Percent);
        ShowButtons(frame);
        ShowDetails(frame);
    }

    // The 2 px line: silk at 14% and a 32% stretch of amber that moves across and back, or, with
    // animation effects off, rests at the start and breathes: 2 s from 100% to 40%, 2 s back.

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
            {
                Motion.FadeTo(StartingLoader, 0, Motion.Standard);
                _ = StopLoaderOnceGone();
            }
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
                loaderBoard.Children.Add(Loop(LoaderStretch, "Opacity", 1, 0.4, TimeSpan.FromSeconds(2), new SineEase { EasingMode = EasingMode.EaseInOut }));
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

    /// <summary>The line's loop stops once its fade away is done, so nothing runs under a failure.</summary>
    async Task StopLoaderOnceGone()
    {
        await Task.Delay(Motion.Standard);
        if (loaderMode == "gone")
            StopLoader();
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

    // The way out. The waiting row has a place for each button from the start, in its order, and
    // a button fades in at its place when it applies and away when it stops; the room under the
    // rows is as wide as the widest waiting row can get, so nothing below or beside moves.

    static readonly StartAction[] Waiting = [StartAction.Cancel, StartAction.FromBeginning, StartAction.ChooseSource, StartAction.Details, StartAction.Skip];

    /// <summary>The longest a Skip label runs before it is cut, and the name the kept room is measured with.</summary>
    const double SkipWidest = 180;
    static string SkipRoom => T.PlayerStartSkipSource(new string('m', 12));

    void BuildStartRows(bool resumes)
    {
        startButtons.Clear();
        StartingButtons.Children.Clear();
        StartingRoom.Children.Clear();
        foreach (var action in Waiting)
        {
            if (action == StartAction.FromBeginning && !resumes)
                continue;
            var (label, look) = action switch
            {
                StartAction.Cancel => (T.ActionCancel, StartLook.Quiet),
                StartAction.FromBeginning => (T.PlayerStartFromBeginning, StartLook.Tonal),
                StartAction.ChooseSource => (T.PlayerChooseSource, StartLook.Tonal),
                StartAction.Details => (T.PlayerStartDetails, StartLook.Quiet),
                _ => (SkipRoom, StartLook.Tonal),
            };
            var button = MakeStartButton(new StartButton(action, label, look), detailsOpen: false);
            Present(button, false, at: true);
            startButtons[action] = button;
            StartingButtons.Children.Add(button);
            var room = MakeStartButton(new StartButton(action, label, look), detailsOpen: false);
            room.IsTabStop = false;
            StartingRoom.Children.Add(room);
        }
        StartingOffline.Children.Clear();
        StartingFailing.Children.Clear();
        startOfflineKey = startFailingKey = "";
        ShowRow(StartingButtons);
    }

    /// <summary>A button on show or not: it keeps its place, and an absent one takes no clicks, focus or reading.</summary>
    static void Present(Button button, bool on, bool at = false)
    {
        var shown = button.IsHitTestVisible;
        if (shown == on && !at)
            return;
        button.IsHitTestVisible = on;
        button.IsTabStop = on;
        AutomationProperties.SetAccessibilityView(button, on ? AccessibilityView.Content : AccessibilityView.Raw);
        if (at)
            button.Opacity = on ? 1 : 0;
        else
            Motion.FadeTo(button, on ? 1 : 0, on ? Motion.FadeInTime : Motion.Standard);
    }

    /// <summary>The row on show: the others fade away where they are and stop taking clicks and focus.</summary>
    void ShowRow(WrapRow row)
    {
        foreach (var each in new[] { StartingButtons, StartingOffline, StartingFailing })
        {
            var on = each == row;
            if (each.IsHitTestVisible == on && Math.Abs(each.Opacity - (on ? 1 : 0)) < 0.01)
                continue;
            each.IsHitTestVisible = on;
            foreach (var child in each.Children.OfType<Control>())
                child.IsTabStop = on && (each != StartingButtons || child.IsHitTestVisible);
            AutomationProperties.SetAccessibilityView(each, on ? AccessibilityView.Content : AccessibilityView.Raw);
            Motion.FadeTo(each, on ? 1 : 0, Motion.Standard);
        }
    }

    void ShowButtons(StartFrame frame)
    {
        if (frame.Failure is not null)
        {
            var key = string.Join(",", frame.Buttons.Select(b => b.Action));
            if (key != startFailingKey)
            {
                startFailingKey = key;
                StartingFailing.Children.Clear();
                foreach (var spec in frame.Buttons)
                    StartingFailing.Children.Add(MakeStartButton(spec, frame.DetailsOpen));
            }
            ShowRow(StartingFailing);
            if (FocusManager.GetFocusedElement(XamlRoot) is not Button { Parent: WrapRow { Name: nameof(StartingFailing) } }
                && StartingFailing.Children.FirstOrDefault() is Button primary)
                primary.Focus(FocusState.Programmatic);
            return;
        }
        if (frame.Phase == StartPhase.Waiting)
        {
            var key = string.Join(",", frame.Buttons.Select(b => b.Action));
            if (key != startOfflineKey)
            {
                startOfflineKey = key;
                StartingOffline.Children.Clear();
                foreach (var spec in frame.Buttons)
                    StartingOffline.Children.Add(MakeStartButton(spec, frame.DetailsOpen));
            }
            ShowRow(StartingOffline);
            return;
        }
        ShowRow(StartingButtons);
        foreach (var (action, button) in startButtons)
        {
            var spec = frame.Buttons.FirstOrDefault(b => b.Action == action);
            if (spec is not null)
                DressStartButton(button, spec, frame.DetailsOpen);
            var on = spec is { Shown: true };
            if (!on && button.IsHitTestVisible && button.FocusState != FocusState.Unfocused)
                FocusNextStartButton(action, frame, button.FocusState);
            Present(button, on);
        }
    }

    /// <summary>A button about to hide had focus: it moves on to the next one on show, or back to Cancel.</summary>
    void FocusNextStartButton(StartAction leaving, StartFrame frame, FocusState how)
    {
        var after = Array.IndexOf(Waiting, leaving);
        var next = Waiting.Skip(after + 1)
            .Where(a => frame.Buttons.Any(b => b.Action == a && b.Shown))
            .Select(a => startButtons.GetValueOrDefault(a))
            .FirstOrDefault(b => b is { IsHitTestVisible: true });
        (next ?? startButtons.GetValueOrDefault(StartAction.Cancel))?.Focus(how == FocusState.Keyboard ? FocusState.Keyboard : FocusState.Programmatic);
    }

    Button MakeStartButton(StartButton spec, bool detailsOpen)
    {
        var button = new Button { MinHeight = 34, Padding = new Thickness(14, 0, 14, 0), FontWeight = Microsoft.UI.Text.FontWeights.Bold };
        var action = spec.Action;
        button.Click += (_, _) => StartPressed(action);
        DressStartButton(button, spec, detailsOpen);
        return button;
    }

    void DressStartButton(Button button, StartButton spec, bool detailsOpen)
    {
        var look = $"{spec.Look}{detailsOpen}{spec.Label}";
        if (button.Tag as string == look)
            return;
        button.Tag = look;
        AutomationProperties.SetName(button, spec.Label);
        if (spec.Action == StartAction.Details)
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
            row.Children.Add(new TextBlock { Text = spec.Label });
            row.Children.Add(new FontIcon { Glyph = detailsOpen ? "" : "", FontSize = 10, VerticalAlignment = VerticalAlignment.Center });
            button.Content = row;
        }
        else if (spec.Action == StartAction.Skip)
            button.Content = new TextBlock { Text = spec.Label, MaxWidth = SkipWidest, TextTrimming = TextTrimming.CharacterEllipsis };
        else
            button.Content = spec.Label;
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

    // Details: what each source said, and the audio and subtitles this start will use. It opens
    // under the row, which stays where it is.

    void ShowDetails(StartFrame frame)
    {
        var open = frame.DetailsOpen && frame.Buttons.Any(b => b.Action == StartAction.Details);
        if (open && StartingDetails.Visibility != Visibility.Visible)
            Motion.FadeIn(StartingDetails, Motion.Standard);
        else if (!open && StartingDetails.Visibility == Visibility.Visible)
            Motion.HideStandard(StartingDetails);
        var structure = string.Join("|", frame.Rows.Select(r => $"{r.Mark}/{r.Name}/{r.Note}/{r.Skip}"));
        if (structure != startRowsKey)
        {
            // What was there before: a row or a row's Skip that is new fades in, the rest stays.
            var before = startRowsKey.Split('|', StringSplitOptions.RemoveEmptyEntries)
                .Select(r => r.Split('/'))
                .Where(p => p.Length >= 4)
                .ToDictionary(p => p[1], p => p[3].Length > 0);
            startRowsKey = structure;
            StartingRows.Children.Clear();
            startRowTimes.Clear();
            foreach (var row in frame.Rows)
            {
                var (made, skip) = MakeRow(row);
                StartingRows.Children.Add(made);
                if (!before.ContainsKey(row.Name))
                    Motion.FadeIn(made);
                else if (skip is not null && !before[row.Name])
                    Motion.FadeIn(skip);
            }
        }
        else
            for (var i = 0; i < frame.Rows.Count && i < startRowTimes.Count; i++)
                if (startRowTimes[i].Text != frame.Rows[i].Time)
                    startRowTimes[i].Text = frame.Rows[i].Time;
        var known = frame.Translation is not null;
        StartingLanguage.Visibility = known ? Visibility.Visible : Visibility.Collapsed;
        if (frame.Translation is { } translation)
        {
            StartingSub.IsChecked = translation == Kaeri.Engine.Core.Translation.Sub;
            StartingDub.IsChecked = translation == Kaeri.Engine.Core.Translation.Dub;
        }
    }

    (UIElement Row, Button? Skip) MakeRow(StartRow row)
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
        var text = new TextBlock { VerticalAlignment = VerticalAlignment.Center, TextWrapping = TextWrapping.NoWrap, TextTrimming = TextTrimming.CharacterEllipsis };
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
        Button? skip = null;
        if (row.Skip is { } label)
        {
            skip = new Button
            {
                Content = new TextBlock { Text = label, MaxWidth = SkipWidest, TextTrimming = TextTrimming.CharacterEllipsis },
                MinHeight = 24,
                Padding = new Thickness(8, 0, 8, 0),
            };
            AutomationProperties.SetName(skip, label);
            skip.Click += (_, _) => StartPressed(StartAction.Skip);
            Grid.SetColumn(skip, 3);
            grid.Children.Add(skip);
        }
        return (grid, skip);
    }

    void OnStartingSub(object sender, RoutedEventArgs e) => ChooseStartTranslation(Kaeri.Engine.Core.Translation.Sub);
    void OnStartingDub(object sender, RoutedEventArgs e) => ChooseStartTranslation(Kaeri.Engine.Core.Translation.Dub);

    void ChooseStartTranslation(Kaeri.Engine.Core.Translation translation)
    {
        StartingSub.IsChecked = translation == Kaeri.Engine.Core.Translation.Sub;
        StartingDub.IsChecked = translation == Kaeri.Engine.Core.Translation.Dub;
        Send(new PlayerCommand.Choose(new PlayerChoice.Translation(translation)));
    }

    // What a screen reader hears: the start, the slow moment, the offline wait and a failure.
    // The lines that fade are read as one element, not announced at each step.

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

    /// <summary>One of the two halves of a line: what fades, its text, and the failure's dot.</summary>
    sealed record FadingPage(UIElement Root, TextBlock Text, Ellipse? Dot);

    /// <summary>
    /// Text that changes in place: the new text fades in over the old, where the old one is, on
    /// the standard token, whether or not animation effects are on. A line whose words stay and
    /// whose numbers move changes in place. A change waits out 0.6 s after the one before; the
    /// caller asks again on its next tick, so the newest wins.
    /// </summary>
    sealed class FadingText(FadingPage first, FadingPage second)
    {
        FadingPage on = first, off = second;
        string text = "", key = "";
        long swapped;

        static string Key(string value, bool dot, bool two) =>
            $"{dot}{two}{new string(value.Where(c => !char.IsDigit(c)).ToArray())}";

        /// <summary>The text there from the start, with no fade.</summary>
        public void Reset(string value)
        {
            Dress(on, value, false, false);
            on.Root.Opacity = value.Length > 0 ? 1 : 0;
            Dress(off, "", false, false);
            off.Root.Opacity = 0;
            text = value;
            key = Key(value, false, false);
            swapped = 0;
        }

        static void Dress(FadingPage page, string value, bool dot, bool two)
        {
            page.Text.Text = value;
            page.Text.TextWrapping = two ? TextWrapping.WrapWholeWords : TextWrapping.NoWrap;
            page.Text.MaxLines = two ? 2 : 1;
            if (page.Dot is { } d)
            {
                d.Visibility = dot ? Visibility.Visible : Visibility.Collapsed;
                d.Fill = new SolidColorBrush(Ui.Vermilion);
            }
        }

        public void Set(string value, bool dot, bool two)
        {
            if (value == text)
                return;
            var next = Key(value, dot, two);
            if (text.Length == 0)
            {
                // Arriving: it fades in where it will stay.
                Dress(on, value, dot, two);
                Motion.FadeTo(on.Root, value.Length > 0 ? 1 : 0, Motion.FadeInTime);
            }
            else if (value.Length == 0)
                Motion.FadeTo(on.Root, 0, Motion.Standard);
            else if (next == key)
                Dress(on, value, dot, two);
            else
            {
                if (swapped != 0 && Stopwatch.GetElapsedTime(swapped) < StartScreen.Hold)
                    return;
                Dress(off, value, dot, two);
                Motion.Crossfade(on.Root, off.Root);
                (on, off) = (off, on);
                swapped = Stopwatch.GetTimestamp();
            }
            text = value;
            key = next;
        }
    }

#if KAERI_TESTING
    /// <summary>
    /// <c>--show=starting-&lt;state&gt;[-details][-beginning][-&lt;seconds&gt;][-reduced]</c>: the engine's made-up start in that
    /// state (the names of <see cref="StartingFixture"/>, kebab-cased), played on its own clock, or
    /// with a number of seconds frozen at that moment for a screenshot. With <c>-details</c>,
    /// Details opens 5 s in, as the viewer opens it; with <c>-beginning</c>, Play from the beginning is
    /// pressed 4 s in, from the keyboard, so focus moves on.
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
        var beginning = rest.EndsWith("-beginning", StringComparison.Ordinal);
        if (beginning)
            rest = rest[..^"-beginning".Length];
        var details = rest.EndsWith("-details", StringComparison.Ordinal);
        if (details)
            rest = rest[..^"-details".Length];
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
            "offline-timed-out" => StartingFixture.OfflineTimedOut,
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
        if (details && freeze is null)
            _ = OpenDetailsLater();
        if (beginning && freeze is null)
            _ = FromBeginningLater();
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
            if (details && start is { } s)
                s.DetailsOpen = true;
            RefreshStart();
        }
    }

    async Task FromBeginningLater()
    {
        await Task.Delay(TimeSpan.FromSeconds(4));
        if (startButtons.TryGetValue(StartAction.FromBeginning, out var button) && button.IsHitTestVisible)
            button.Focus(FocusState.Keyboard);
        StartPressed(StartAction.FromBeginning);
    }

    async Task OpenDetailsLater()
    {
        await Task.Delay(TimeSpan.FromSeconds(5));
        if (start is { DetailsOpen: false })
            StartPressed(StartAction.Details);
    }
#endif
}
