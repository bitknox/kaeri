namespace Kaeri.App;

/// <summary>
/// Stand-in words for the room, the board's English as drawn (board v5, main.js and board.js),
/// until the engine branch's catalogue lands. Then each is swapped for its <c>T.*</c> string and
/// this file goes. Nothing here is read by the strings lint: it scans Kaeri.Windows only, and the
/// pages take every word from here.
/// </summary>
public static class RoomWords
{
    // The column's header
    public const string Title = "Watch together";
    public const string Invite = "Invite";
    public const string Details = "Details";
    public const string Leave = "Leave";
    public const string HideRoom = "Hide room";
    public const string ShowRoom = "Show room";

    // The room's line
    public const string Watching = "Watching together";
    public static string WatchingAt(string episode, string at) => $"{episode} · {at}";
    public static string PausedFor(string names) => $"Paused for {names}";
    public const string PausedForYou = "Paused for you";
    public static string Waiting(string names, int many, long seconds) => $"{names} {(many > 1 ? "are" : "is")} waiting · {seconds} s";
    public static string Why(string why, long seconds) => $"{why} · {seconds} s";
    public const string WhyLoading = "Loading";
    public const string WhyBuffering = "Buffering";
    public static string StillLoading(string names, int many) => $"{names} {(many > 1 ? "are" : "is")} still loading";
    public const string WaitedCap = "The room has waited 20 s";
    public static string CarryOnWithout(string names) => $"Carry on without {names}";
    public static string StartWithout(string names) => $"Start without {names}";
    public const string Them = "them";
    public static string CarriedOn(string by, string names, int many) =>
        many > 1 ? $"{by} carried on · they join when ready" : $"{by} carried on · {names} joins when ready";
    public static string Paused(string by) => $"{by} paused";
    public const string YouPaused = "You paused";
    public static string PausedAt(string at) => $"at {at}";
    public const string NotStarted = "Not started yet";
    public const string PlayStartsIt = "Play starts it for everyone";
    public static string StartingIn(long n) => $"Starting in {n}";
    public const string EveryoneReady = "Everyone is ready";
    public static string StartingWhen(string names, int many) => $"Starting when {names} {(many > 1 ? "are" : "is")} ready";
    public static string UpNextIn(long seconds) => $"Up next in {seconds} s";
    public static string PlaysForEveryone(string episode) => $"{episode} plays for everyone";
    public const string ReconnectingRoom = "Reconnecting to the room";
    public const string NoRelayAnswers = "No relay is answering · your episode keeps playing";
    public static string RelaysDown(int down, int of) =>
        down == 1 ? $"1 of {of} relays isn't answering · the room carries on" : $"{down} of {of} relays aren't answering · the room carries on";
    public static string YouBehind(long seconds) => $"You're {seconds} s behind · catching up";
    public static string Invited(string by) => $"{by} invited you";
    public static string Over(int people) => $"{people} people are here already · rooms work best with 10 or fewer";
    public const string JoinRoom = "Join the room";
    public const string JoinAnyway = "Join anyway";
    public const string NotNow = "Not now";
    public static string MovedTheRoom(string by) => $"{by} moved the room";
    public static string GoAlongAsk(string others, int many, string at) =>
        many == 0 ? $"Go along? The episode carries on from {at}." : $"Go along? {others} {(many > 1 ? "are" : "is")} asked too. The episode carries on from {at}.";
    public const string GoAlong = "Go along";
    public const string StayHere = "Stay here";

    // Names joined
    public static string Two(string a, string b) => $"{a} and {b}";
    public static string People(int n) => $"{n} people";
    public static string You(string name) => $"{name} (you)";
    public const string YouAlone = "You";
    public const string YouMark = "(you)";

    // A row's word
    public const string StateWatching = "Watching";
    public const string StateReady = "Ready";
    public const string StatePausedIt = "Paused it";
    public const string StatePaused = "Paused";
    public static string StateBehind(long s) => $"Behind · {s} s";
    public static string StateLoading(long s) => $"Loading · {s} s";
    public static string StateBuffering(long s) => $"Buffering · {s} s";
    public static string StateOther(string episode) => $"On episode {episode}";
    public const string StateOtherUnknown = "On another episode";
    public const string StateChoosing = "Choosing a source";
    public static string StateNoSource(string episode) => $"No source for episode {episode}";
    public const string StateClosed = "Player closed";
    public const string StateReconnecting = "Reconnecting";
    public const string StateLeft = "Left";
    public static string Muted(string word) => $"{word} · Muted";
    public static string More(int n) => $"+{n}";
    public static string NotWatching(int n) => $"{n} not watching";
    public const string AllWatching = "All watching";

    // The chat
    public const string SaySomething = "Say something";
    public const string Send = "Send";
    public static string PausedForLine(string names) => $"Paused for {names} · loading";
    public const string PausedForYouLine = "Paused for you · loading";
    public static string CarriedOnLine(string by, string names) => $"{by} carried on without {names}";
    public static string SkippedLine(string by) => $"{by} skipped the opening";
    public static string JoinedAt(string name, string at) => $"{name} joined at {at}";
    public const string ChatOpensOnJoin = "The chat opens when you join";

    // Before play, over the video
    public const string UpInTheRoom = "Up in the room";
    public const string RoomIsWatching = "The room is watching";
    public static string EpisodeLength(string episode, long minutes) => $"{episode} · {minutes} min";
    public const string PlaysFromYourSources = "Plays from your sources";
    public const string PlayForEveryone = "Play for everyone";
    public const string AnyoneCanStart = "Anyone in the room can start it";
    public static string ChooseSourceFor(string episode) => $"Choose a source for {episode}";
    public const string RoomKeepsWatching = "The room keeps watching; you join at its place.";
    public const string PlayInRoom = "Play in the room";
    public static string NoneHas(string episode) => $"None of your sources has {episode}";
    public static string LookedIn(int sources) => $"Kaeri looked in your {sources} sources. You stay in the room and the chat while the others watch.";
    public const string OpenSources = "Open sources";
    public const string SearchTitles = "Search titles";
    public static string RoomPausedUntil(long seconds) => $"The room is paused until this starts · {seconds} s";
    public const string DontWait = "Don't wait for me";

    // Up next and Skip
    public const string UpNextForRoom = "Up next for the room";
    public static string PlaysInSeconds(long seconds) => $"Plays for everyone in {seconds} s";

    // Invite
    public const string InviteBody = "Anyone with this link can join. The link carries the room's key, so only people who have it can read the room.";
    public const string CopyLink = "Copy link";
    public const string Share = "Share…";
    public const string NewInvite = "New invite";
    public const string NewInviteBody = "New invite stops the old link working. Everyone in the room moves to the new one.";
    public const string MoveRoom = "Move to a new room…";
    public const string MoveRoomBody = "For someone already here: start a new room and pick who comes along.";

    // Details
    public const string Encryption = "Encryption";
    public const string EndToEnd = "End to end";
    public const string Relays = "Relays";
    public static string RelaysAnswering(int up, int of) => up == of ? $"{up} answering" : $"{up} of {of} answering";
    public static string Rtt(uint ms) => $"{ms} ms";
    public const string NotAnswering = "Not answering";
    public const string RelaysBody = "Relays pass the room's messages along and can't read them. These are public relays run by others; Kaeri doesn't run any of them. If one stops answering, the room swaps in a spare.";
    public const string EditRelays = "Edit relays…";
    public const string EditRelaysTitle = "Edit relays";
    public const string EditRelaysBody = "Changes this room's relays for everyone in it. New invites carry the list.";
    public static string Remove(string what) => $"Remove {what}";
    public const string RelayScheme = "wss://";
    public const string Add = "Add";
    public const string RelaysInSettings = "Settings ▸ Playback ▸ Relays sets the list for new rooms.";
    public const string Done = "Done";

    // Leave
    public const string LeaveTitle = "Leave the room?";
    public const string LeaveBody = "The others keep watching.";
    public const string LeaveCloseBody = "The others keep watching. Keep the room closes the player and keeps your place.";
    public const string KeepRoom = "Keep the room";
    public const string Cancel = "Cancel";

    // Move to a new room
    public const string MoveTitle = "Move to a new room";
    public const string MoveBody = "Everyone you tick is asked to go along. Nobody moves without saying yes, and anyone not ticked stays in this room.";
    public static string MoveFoot(string at) => $"The new room picks up at {at} with a new key. Its link reaches only the people ticked.";
    public const string AlwaysGoes = "Always goes";
    public const string MutedNotTicked = "Muted, so not ticked";
    public static string MoveWith(int others) => others == 1 ? "Move with 1 other" : $"Move with {others} others";
}
