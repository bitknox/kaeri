using Kaeri.App;

namespace Kaeri.Windows;

/// <summary>
/// Hide chat, the app's own setting: lines, reactions, ticks and the field stay off in every
/// party on this device until it is turned off. The room itself does not change, and the
/// engine keeps the conversation either way. Kept as a flag file beside the app's other small
/// choices.
/// </summary>
static class ChatSetting
{
    static string? path;
    static bool hidden;

    /// <summary>Hide chat was turned on or off.</summary>
    public static event Action? Changed;

    public static void Load(string configDir)
    {
        path = Path.Combine(configDir, "windows-hide-chat");
        hidden = SmallFiles.Read(path) is not null;
    }

    public static bool Hidden => hidden;

    public static void Set(bool hide)
    {
        if (hide == hidden)
            return;
        hidden = hide;
        if (path is not null)
        {
            if (hide)
                SmallFiles.Write(path, "");
            else
                SmallFiles.Delete(path);
        }
        Changed?.Invoke();
    }
}
