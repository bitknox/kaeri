using Kaeri.App;
using Kaeri.Engine.Ffi;
using Microsoft.UI.Dispatching;

namespace Kaeri.Windows;

/// <summary>
/// What the engine asks of Windows, from the engine's own threads: links open in the default
/// browser, notices become in-app toasts, and tracker tokens live in Credential Manager. A
/// fixture run keeps its tokens in memory, so nothing it does reaches the user's credentials.
/// </summary>
sealed class AppHost(DispatcherQueue ui, bool fixture) : FfiHost
{
    readonly Dictionary<string, string> memory = [];

    /// <summary>A notice for the window, on the UI thread.</summary>
    public Action<string, string>? Notified { get; set; }

    /// <summary>
    /// A link the engine opens. A provider plugin can name one itself, so only a web address
    /// opens, in the browser; every other scheme is refused.
    /// </summary>
    public bool OpenUrl(string url)
    {
        if (Outside.For(url, out var uri) != Launch.Open || uri is null)
            return false;
        return ui.TryEnqueue(async () => await global::Windows.System.Launcher.LaunchUriAsync(uri));
    }

    public void Notify(string title, string body) => ui.TryEnqueue(() => Notified?.Invoke(title, body));

    public string? SecretGet(string id)
    {
        if (fixture)
            lock (memory) return memory.GetValueOrDefault(id);
        return Credentials.Get(id);
    }

    public bool SecretSet(string id, string value)
    {
        if (fixture)
        {
            lock (memory) memory[id] = value;
            return true;
        }
        return Credentials.Set(id, value);
    }

    public bool SecretDelete(string id)
    {
        if (fixture)
            lock (memory) { memory.Remove(id); return true; }
        return Credentials.Delete(id);
    }
}
