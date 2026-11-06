using System.Collections;
using System.Globalization;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Text;

namespace Kaeri.App;

/// <summary>
/// What lets a page keep what it drew from one engine event to the next: rows put in a new order
/// without taking out the one with the focus, and a text of what a piece of data says, to compare
/// with what was last drawn.
/// </summary>
public static class Drawn
{
    /// <summary>
    /// Puts <paramref name="items"/> in the order of <paramref name="desired"/>, moving as little as
    /// it can: what is gone is taken out, what is new goes in at its place, and the rest stay where
    /// they are. <paramref name="keep"/>, such as the row holding the focus, is never taken out, so
    /// it keeps the focus and the pointer while the rows around it move.
    /// </summary>
    public static void Arrange<T>(IList<T> items, IReadOnlyList<T> desired, T? keep = null) where T : class
    {
        var wanted = new HashSet<T>(desired, ReferenceEqualityComparer.Instance);
        for (var i = items.Count - 1; i >= 0; i--)
            if (!wanted.Contains(items[i]))
                items.RemoveAt(i);
        for (var i = 0; i < desired.Count; i++)
        {
            var want = desired[i];
            if (i < items.Count && ReferenceEquals(items[i], want))
                continue;
            if (ReferenceEquals(want, keep) && IndexOf(items, want) >= 0)
            {
                // The kept one stays put: what stands before it out of order goes to the end, to be
                // placed again when its own turn comes.
                while (!ReferenceEquals(items[i], want))
                {
                    var before = items[i];
                    items.RemoveAt(i);
                    items.Add(before);
                }
                continue;
            }
            var at = IndexOf(items, want);
            if (at >= 0)
                items.RemoveAt(at);
            items.Insert(Math.Min(i, items.Count), want);
        }
    }

    static int IndexOf<T>(IList<T> items, T item) where T : class
    {
        for (var i = 0; i < items.Count; i++)
            if (ReferenceEquals(items[i], item))
                return i;
        return -1;
    }

    /// <summary>
    /// What a piece of the engine's data says, as text: its values all the way down, so two
    /// readings of the same thing give the same text even when they are different objects. A
    /// page compares it with what it last drew to know whether to draw again.
    /// </summary>
    public static string Fingerprint(object? value)
    {
        var text = new StringBuilder();
        Print(value, text, 0);
        return text.ToString();
    }

    static void Print(object? value, StringBuilder text, int depth)
    {
        switch (value)
        {
            case null:
                text.Append('∅');
                return;
            case string s:
                // Its length first, so no string can pass for the marks around another.
                text.Append(s.Length).Append(':').Append(s);
                return;
            case IFormattable or bool or char or Enum:
                text.Append(Convert.ToString(value, CultureInfo.InvariantCulture));
                return;
            case ITuple tuple:
                text.Append('(');
                for (var i = 0; i < tuple.Length; i++)
                {
                    Print(tuple[i], text, depth + 1);
                    text.Append(',');
                }
                text.Append(')');
                return;
            case IEnumerable items:
                text.Append('[');
                foreach (var item in items)
                {
                    Print(item, text, depth + 1);
                    text.Append(',');
                }
                text.Append(']');
                return;
        }
        var type = value.GetType();
        text.Append(type.Name);
        if (depth > 12)
            return;
        text.Append('{');
        foreach (var property in type.GetProperties(BindingFlags.Instance | BindingFlags.Public))
        {
            if (property.GetIndexParameters().Length > 0)
                continue;
            text.Append(property.Name).Append('=');
            Print(property.GetValue(value), text, depth + 1);
            text.Append(';');
        }
        text.Append('}');
    }
}
