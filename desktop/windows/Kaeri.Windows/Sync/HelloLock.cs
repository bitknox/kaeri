using Kaeri.App;
using Kaeri.Engine.Ffi;
using Windows.Security.Credentials.UI;

namespace Kaeri.Windows.Sync;

/// <summary>
/// The PC's own lock before the recovery key shows again: Windows Hello (face, fingerprint or
/// PIN). Where Hello is not set up on this PC there is nothing to ask, so the key shows, as on
/// Linux without polkit. A person who cancels sees nothing.
/// </summary>
static class HelloLock
{
    public static async void Ask(Action then)
    {
        UserConsentVerifierAvailability available;
        try
        {
            available = await UserConsentVerifier.CheckAvailabilityAsync();
        }
        catch (Exception e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "sync", $"Windows Hello could not say whether it is set up: {e.Message}");
            available = UserConsentVerifierAvailability.DeviceNotPresent;
        }
        if (available != UserConsentVerifierAvailability.Available)
        {
            then();
            return;
        }
        try
        {
            if (await UserConsentVerifier.RequestVerificationAsync(T.RkeyHelloReason) == UserConsentVerificationResult.Verified)
                then();
        }
        catch (Exception e)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "sync", $"Windows Hello did not ask: {e.Message}");
        }
    }
}
