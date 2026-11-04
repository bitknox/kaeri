#if KAERI_TESTING
namespace Kaeri.Windows.Sync;

/// <summary>
/// The profile screens for the shot run (<c>--show=profiles-*</c>): <c>--scene=profiles</c> has
/// three made-up profiles, Alex in use and Sam a kids profile, and any other scene has one. Each
/// state opens its dialog or the account flyout once the engine has said what the profiles are.
/// </summary>
static partial class Profiles
{
    public static void Stage(string show)
    {
        Ctx.Window.DispatcherQueue.TryEnqueue(async () =>
        {
            await Task.Delay(4000);
            var profiles = Ctx.Known.Profiles;
            var other = profiles.FirstOrDefault(p => !p.Active && !p.Kids);
            var kids = profiles.FirstOrDefault(p => p.Kids);
            switch (show)
            {
                case "profiles-add":
                    Edit(null);
                    break;
                case "profiles-edit" when other is not null:
                    Edit(other);
                    break;
                case "profiles-kids" when kids is not null:
                    Edit(kids);
                    break;
                case "profiles-delete" when other is not null:
                    Delete(other);
                    break;
                case "profiles-flyout":
                    Ctx.Window.ShowProfileFlyout();
                    break;
            }
        });
    }
}
#endif
