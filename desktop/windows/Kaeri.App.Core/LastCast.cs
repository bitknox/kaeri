namespace Kaeri.App;

/// <summary>
/// The show on the TV, kept so that the mini can name it while a cast left running is joined
/// again, after a relaunch too. The file outlives the session, so it holds the show only while
/// Kaeri may keep what it records of it (<see cref="Privately.MayKeep"/>). A private cast leaves
/// no file: its show is held in memory until the app quits.
/// </summary>
public sealed class LastCast(string path, Privately privately)
{
    (uint Id, string Title)? playing;
    /// <summary>Whether this launch has written or cleared the file, and the show it holds since.</summary>
    bool known;
    uint? kept;

    /// <summary>The TV plays show <paramref name="id"/>, called <paramref name="title"/>.</summary>
    public void Playing(uint id, string title)
    {
        playing = (id, title);
        if (privately.MayKeep(id))
        {
            if (known && kept == id)
                return;
            known = SmallFiles.Write(path, $"{id}\t{title}");
            kept = known ? id : null;
        }
        else if (!known || kept is not null)
        {
            // Nor does an earlier show stay to be named in its place.
            known = SmallFiles.Delete(path);
            kept = null;
        }
    }

    /// <summary>The show the mini names while a cast is joined again: this launch's, or the one a launch before kept.</summary>
    public (uint Id, string Title)? Remembered() => playing ?? Kept();

    (uint Id, string Title)? Kept() =>
        SmallFiles.Read(path) is { } line && line.Split('\t', 2) is [var id, var title] && uint.TryParse(id, out var show)
            ? (show, title)
            : null;
}
