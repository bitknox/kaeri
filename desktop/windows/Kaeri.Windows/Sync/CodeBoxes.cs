using Kaeri.App;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.System;

namespace Kaeri.Windows.Sync;

/// <summary>
/// A pairing code typed into six boxes, three and three: a digit moves on to the next box,
/// Backspace in an empty box goes back, and a pasted code fills them all.
/// </summary>
sealed class CodeBoxes
{
    public readonly StackPanel View = new() { Orientation = Orientation.Horizontal, Spacing = 6, HorizontalAlignment = HorizontalAlignment.Center };
    readonly TextBox[] boxes = new TextBox[6];
    bool filling;

    public event Action? Changed;

    public string Text => string.Concat(boxes.Select(b => b.Text));

    public CodeBoxes()
    {
        for (var i = 0; i < boxes.Length; i++)
        {
            var at = i;
            var box = new TextBox
            {
                Width = 40,
                Height = 48,
                MaxLength = 1,
                FontFamily = new FontFamily("Cascadia Mono, Consolas"),
                FontSize = 24,
                TextAlignment = TextAlignment.Center,
                VerticalContentAlignment = VerticalAlignment.Center,
                Padding = new Thickness(0, 6, 0, 0),
                InputScope = new InputScope { Names = { new InputScopeName(InputScopeNameValue.Number) } },
            };
            AutomationProperties.SetName(box, $"{T.PairCodeField} {at + 1}");
            box.TextChanged += (_, _) => Typed(at);
            box.Paste += (_, e) =>
            {
                e.Handled = true;
                Paste(at);
            };
            box.KeyDown += (_, e) =>
            {
                if (e.Key == VirtualKey.Back && box.Text.Length == 0 && at > 0)
                {
                    boxes[at - 1].Text = "";
                    boxes[at - 1].Focus(FocusState.Keyboard);
                    e.Handled = true;
                }
            };
            boxes[i] = box;
            View.Children.Add(box);
            if (i == 2)
                View.Children.Add(new Border { Width = 8 });
        }
    }

    public void Focus() => boxes[0].Focus(FocusState.Programmatic);

    void Typed(int at)
    {
        if (filling)
            return;
        var box = boxes[at];
        var digits = SyncWords.CodeDigits(box.Text);
        if (digits != box.Text)
        {
            filling = true;
            box.Text = digits;
            filling = false;
        }
        if (digits.Length == 1 && at + 1 < boxes.Length)
            boxes[at + 1].Focus(FocusState.Keyboard);
        Changed?.Invoke();
    }

    async void Paste(int at)
    {
        var view = global::Windows.ApplicationModel.DataTransfer.Clipboard.GetContent();
        if (!view.Contains(global::Windows.ApplicationModel.DataTransfer.StandardDataFormats.Text))
            return;
        var digits = SyncWords.CodeDigits(await view.GetTextAsync());
        filling = true;
        for (var i = 0; i < digits.Length && at + i < boxes.Length; i++)
            boxes[at + i].Text = digits[i].ToString();
        filling = false;
        boxes[Math.Min(boxes.Length - 1, at + digits.Length)].Focus(FocusState.Keyboard);
        Changed?.Invoke();
    }
}
