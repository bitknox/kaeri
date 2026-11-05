using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;

namespace Kaeri.Windows.Pages;

/// <summary>
/// The title page's browsing extras: Another (a Surprise me pick's next), Go to episode from the
/// range menu, Ctrl+G and a typed number, and the range shortcuts. The engine decides what a
/// number means; the page lands on the row, crossfading when it has to change range or open a fold.
/// </summary>
public sealed partial class TitlePage
{
    /// <summary>The pause after the last digit before the number is taken.</summary>
    static readonly TimeSpan TypedPause = TimeSpan.FromMilliseconds(600);

    int shownRange;
    string? targetNumber;
    string typed = "";
    bool picking;
    readonly DispatcherTimer typedTimer = new() { Interval = TypedPause };

    void KeysForGo()
    {
        typedTimer.Tick += (_, _) =>
        {
            typedTimer.Stop();
            TakeTyped();
        };
        Accelerate(global::Windows.System.VirtualKey.G, global::Windows.System.VirtualKeyModifiers.Control, () => OpenGoTo());
        // Ctrl+Shift+[ and ]: the previous and next range of 100.
        const global::Windows.System.VirtualKeyModifiers both = global::Windows.System.VirtualKeyModifiers.Control | global::Windows.System.VirtualKeyModifiers.Shift;
        Accelerate((global::Windows.System.VirtualKey)219, both, () => StepRange(-1));
        Accelerate((global::Windows.System.VirtualKey)221, both, () => StepRange(1));
        EpisodeList.PreviewKeyDown += OnTypedKey;
    }

    void Accelerate(global::Windows.System.VirtualKey key, global::Windows.System.VirtualKeyModifiers modifiers, Action run)
    {
        var accelerator = new KeyboardAccelerator { Key = key, Modifiers = modifiers };
        accelerator.Invoked += (_, a) =>
        {
            a.Handled = true;
            run();
        };
        KeyboardAccelerators.Add(accelerator);
    }

    // Another.

    void ShowAnother()
    {
        AnotherGlyph.Content ??= Dice.Icon();
        AnotherAction.Visibility = ctx.Pick is { More: true } pick && pick.Id == id ? Visibility.Visible : Visibility.Collapsed;
    }

    void OnAnother(object sender, RoutedEventArgs e)
    {
        if (picking || ctx.Pick is not { } pick || pick.Id != id)
            return;
        picking = true;
        AnotherAction.IsEnabled = false;
        ctx.Send(new Command.Surprise(pick.Pool));
    }

    void HeardAnother(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            if (e is not Event.Surprise answer || !picking)
                continue;
            picking = false;
            AnotherAction.IsEnabled = true;
            if (answer.Outcome is SurpriseOutcome.Picked picked)
            {
                ctx.Pick = new Pick(picked.Id, answer.Pool, picked.More);
                ctx.Window.SwapTitle(picked.Id);
            }
        }
    }

    // Go to episode.

    EpisodeRow[] Rows() => ctx.Known.Episodes.TryGetValue(id, out var rows) ? rows : [];

    /// <summary>Ctrl+G and the range menu's last item: a dialog with the number to go to.</summary>
    async void OpenGoTo(string? stage = null)
    {
        var rows = Rows();
        if (rows.Length == 0)
            return;
        if (await GoToDialog.Ask(XamlRoot, rows, stage) is { } row)
            Land(row);
    }

    /// <summary>
    /// Goes to the row: a scroll when it is within a screen, else a crossfade (across a range, out
    /// of the fold, or further down the same range, so a long jump never streaks). With animation
    /// effects off every jump is the crossfade.
    /// </summary>
    void Land(GoTo.Row row)
    {
        var rows = Rows();
        if (row.Index < 0 || row.Index >= rows.Length)
            return;
        var number = rows[row.Index].Number;
        var across = ranges.Length > 1 && (int)row.Range != shownRange;
        var hidden = !Episodes().Any(i => i.Row.Number == number);
        targetNumber = number;
        void Mark(bool animate)
        {
            foreach (var item in Episodes())
                item.Target = item.Row.Number == number;
            var target = Episodes().FirstOrDefault(i => i.Row.Number == number);
            if (target is null)
                return;
            if (animate)
            {
                DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, () =>
                {
                    if (EpisodeList.ContainerFromItem(target) is UIElement container)
                        container.StartBringIntoView(new BringIntoViewOptions { VerticalAlignmentRatio = 0, AnimationDesired = true });
                });
                return;
            }
            // Under the crossfade: the new rows are laid out now, so the first frame after the
            // change already has the row at the top, and a row not built yet is never missed.
            EpisodeList.UpdateLayout();
            if (EpisodeList.ContainerFromItem(target) is UIElement row)
                row.StartBringIntoView(new BringIntoViewOptions { VerticalAlignmentRatio = 0, AnimationDesired = false });
        }
        if (!across && !hidden && Motion.Enabled && WithinScreen(number))
        {
            Mark(true);
            return;
        }
        Crossfade.Swap(Scroll, () =>
        {
            // Within the range the rows stand; only the scroll changes under the fade.
            if (across)
            {
                chosenRange = (int)row.Range;
                opened = false;
                ShowEpisodes();
            }
            if (!Episodes().Any(i => i.Row.Number == number))
            {
                opened = true;
                ShowEpisodes();
            }
            Mark(false);
        });
    }

    /// <summary>Whether the row is at most a screen's scroll away from the top of the view.</summary>
    bool WithinScreen(string number)
    {
        var target = Episodes().FirstOrDefault(i => i.Row.Number == number);
        if (target is null || EpisodeList.ContainerFromItem(target) is not UIElement container)
            return false;
        var top = container.TransformToVisual(Scroll).TransformPoint(new global::Windows.Foundation.Point(0, 0)).Y;
        return Math.Abs(top) <= Scroll.ViewportHeight;
    }

    // Ctrl+Shift+[ and ].

    void StepRange(int by)
    {
        if (ranges.Length < 2)
            return;
        var next = Browsing.StepRange(ranges.Length, shownRange, by);
        if (next != shownRange)
            OnRangePicked(next);
    }

    // Typed digits.

    void OnTypedKey(object sender, KeyRoutedEventArgs e)
    {
        var ctrl = InputKeyboardSource.GetKeyStateForCurrentThread(global::Windows.System.VirtualKey.Control).HasFlag(global::Windows.UI.Core.CoreVirtualKeyStates.Down);
        var alt = InputKeyboardSource.GetKeyStateForCurrentThread(global::Windows.System.VirtualKey.Menu).HasFlag(global::Windows.UI.Core.CoreVirtualKeyStates.Down);
        if (ctrl || alt)
            return;
        var key = e.Key;
        int? digit = key is >= global::Windows.System.VirtualKey.Number0 and <= global::Windows.System.VirtualKey.Number9
            ? key - global::Windows.System.VirtualKey.Number0
            : key is >= global::Windows.System.VirtualKey.NumberPad0 and <= global::Windows.System.VirtualKey.NumberPad9
                ? key - global::Windows.System.VirtualKey.NumberPad0
                : null;
        if (digit is { } d)
        {
            e.Handled = true;
            TypeDigit(d);
        }
        else if (typed.Length > 0 && key == global::Windows.System.VirtualKey.Enter)
        {
            e.Handled = true;
            typedTimer.Stop();
            TakeTyped();
        }
        else if (typed.Length > 0 && key is global::Windows.System.VirtualKey.Escape or global::Windows.System.VirtualKey.Back)
        {
            e.Handled = true;
            typedTimer.Stop();
            typed = "";
            Motion.Hide(TypedCapsule);
        }
    }

    /// <summary>A digit typed on the list: five at most, and no leading zero; the pause takes the number.</summary>
    void TypeDigit(int d)
    {
        if (typed.Length >= 5 || (typed.Length == 0 && d == 0))
            return;
        typed += d.ToString(System.Globalization.CultureInfo.InvariantCulture);
        SayTyped();
        typedTimer.Stop();
        typedTimer.Start();
    }

    void SayTyped()
    {
        // Under Another when a pick shows it, else where it would stand.
        TypedCapsule.Margin = new Thickness(0, AnotherAction.Visibility == Visibility.Visible ? 64 : 24, 36, 0);
        // "Episode 382": the words around the number in haze, the number in silk.
        var number = long.Parse(typed, System.Globalization.CultureInfo.InvariantCulture);
        var words = T.EpisodeNumber(number);
        var shown = number.ToString(System.Globalization.CultureInfo.CurrentCulture);
        var at = words.IndexOf(shown, StringComparison.Ordinal);
        TypedText.Inlines.Clear();
        if (at < 0)
            TypedText.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = words });
        else
        {
            var haze = Ui.Res<Microsoft.UI.Xaml.Media.Brush>("HazeBrush");
            TypedText.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = words[..at], Foreground = haze });
            TypedText.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = shown });
            TypedText.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = words[(at + shown.Length)..], Foreground = haze });
        }
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(TypedCapsule, words);
        Motion.Show(TypedCapsule);
    }

    /// <summary>The pause or Enter: the number typed is gone to, or its error shown in the capsule's place.</summary>
    void TakeTyped()
    {
        var text = typed;
        typed = "";
        if (text.Length == 0)
            return;
        var result = Browsing.Go(Rows(), text);
        if (result is GoTo.Row row)
        {
            Motion.Hide(TypedCapsule);
            Land(row);
            return;
        }
        // The capsule says why, then goes.
        TypedText.Text = Browsing.GoMessage(result) ?? text;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(TypedCapsule, TypedText.Text);
        var gone = new DispatcherTimer { Interval = TimeSpan.FromSeconds(3) };
        gone.Tick += (_, _) =>
        {
            gone.Stop();
            if (typed.Length == 0)
                Motion.Hide(TypedCapsule);
        };
        gone.Start();
    }
}
