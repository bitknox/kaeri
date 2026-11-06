// Keeping what a page drew: rows put in a new order around the one with the focus, and a text of
// what the engine's data says, to tell a change from the same thing said again.

using System.Collections.ObjectModel;
using Kaeri.App;
using Xunit;

namespace Kaeri.App.Tests;

public class DrawnTests
{
    sealed class Row(string name)
    {
        public override string ToString() => name;
    }

    /// <summary>A list that remembers every row taken out of it, as a panel would drop its focus.</summary>
    sealed class Watched : Collection<Row>
    {
        public readonly List<Row> TakenOut = [];

        protected override void RemoveItem(int index)
        {
            TakenOut.Add(this[index]);
            base.RemoveItem(index);
        }

        protected override void SetItem(int index, Row item)
        {
            TakenOut.Add(this[index]);
            base.SetItem(index, item);
        }
    }

    static Watched Of(params Row[] rows)
    {
        var list = new Watched();
        foreach (var row in rows)
            list.Add(row);
        return list;
    }

    [Fact]
    public void RowsThatStayAreNotTakenOut()
    {
        Row a = new("a"), b = new("b"), c = new("c"), d = new("d");
        var list = Of(a, b, c);
        Drawn.Arrange(list, [a, d, c]);
        Assert.Equal(new[] { a, d, c }, list);
        Assert.Equal(new[] { b }, list.TakenOut);
    }

    [Fact]
    public void TheKeptRowStaysInWhetherItMovesUpOrDown()
    {
        Row a = new("a"), b = new("b"), c = new("c");
        // Alt+Down on a, then Alt+Up on it again: the focused row is never taken out.
        var list = Of(a, b, c);
        Drawn.Arrange(list, [b, a, c], keep: a);
        Assert.Equal(new[] { b, a, c }, list);
        Drawn.Arrange(list, [a, b, c], keep: a);
        Assert.Equal(new[] { a, b, c }, list);
        Assert.DoesNotContain(a, list.TakenOut);
        // And to the far end and back.
        Drawn.Arrange(list, [b, c, a], keep: a);
        Assert.Equal(new[] { b, c, a }, list);
        Drawn.Arrange(list, [a, b, c], keep: a);
        Assert.Equal(new[] { a, b, c }, list);
        Assert.DoesNotContain(a, list.TakenOut);
    }

    [Fact]
    public void AnyNewOrderIsReachedWithoutTakingOutTheKeptRow()
    {
        var random = new Random(7);
        for (var round = 0; round < 500; round++)
        {
            var pool = Enumerable.Range(0, 8).Select(i => new Row($"r{i}")).ToArray();
            var list = Of([.. pool.Where(_ => random.Next(3) > 0)]);
            var desired = pool.Where(_ => random.Next(3) > 0).OrderBy(_ => random.Next()).ToList();
            var both = list.Intersect(desired).ToList();
            var keep = both.Count > 0 ? both[random.Next(both.Count)] : null;
            Drawn.Arrange(list, desired, keep);
            Assert.Equal(desired, list);
            if (keep is not null)
                Assert.DoesNotContain(keep, list.TakenOut);
        }
    }

    sealed record Plugin(string Id, string[] Hosts, Plugin? Next = null);

    [Fact]
    public void TheSameDataReadTwiceGivesTheSameText()
    {
        var first = new Plugin("one", ["a.example", "b.example"], new Plugin("two", []));
        var again = new Plugin("one", ["a.example", "b.example"], new Plugin("two", []));
        Assert.NotEqual(first, again);
        Assert.Equal(Drawn.Fingerprint(first), Drawn.Fingerprint(again));
        Assert.Equal(Drawn.Fingerprint((first, 3, true)), Drawn.Fingerprint((again, 3, true)));
    }

    [Fact]
    public void AChangeAnywhereInsideChangesTheText()
    {
        var first = new Plugin("one", ["a.example"], new Plugin("two", ["c.example"]));
        Assert.NotEqual(Drawn.Fingerprint(first), Drawn.Fingerprint(first with { Hosts = ["b.example"] }));
        Assert.NotEqual(Drawn.Fingerprint(first), Drawn.Fingerprint(first with { Next = new Plugin("two", ["d.example"]) }));
        Assert.NotEqual(Drawn.Fingerprint(first), Drawn.Fingerprint(first with { Next = null }));
        // Words cannot run into each other: ("a,", "b") is not ("a", ",b").
        Assert.NotEqual(Drawn.Fingerprint(("a,", "b")), Drawn.Fingerprint(("a", ",b")));
        Assert.NotEqual(Drawn.Fingerprint(new[] { "ab" }), Drawn.Fingerprint(new[] { "a", "b" }));
    }
}
