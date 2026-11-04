using Kaeri.Engine.Ffi;
using Kaeri.Windows.Sync;

namespace Kaeri.Windows;

/// <summary>
/// Sync, the window's side: Windows' DNS-SD handed to the engine for pairing by a typed code,
/// and the dialogs set to hear the engine. A fixture run never looks on the real network.
/// </summary>
public sealed partial class MainWindow
{
    void StartSync(FfiEngine engine)
    {
        if (!options.Fixture)
            engine.SetSyncNetwork(new SyncNetwork());
        SyncDialogs.Install(dirs.Config);
#if KAERI_TESTING
        if (options.Show?.StartsWith("sync-", StringComparison.Ordinal) == true)
            SyncDialogs.Stage(options.Show);
#endif
    }
}
