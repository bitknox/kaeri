using System.Globalization;
using System.Runtime.CompilerServices;

namespace Kaeri.App.Tests;

/// <summary>
/// The tests expect English formats (45%, March 2026, 1.0 kB) whatever the machine's regional
/// format is. A test that needs another culture sets its own.
/// </summary>
static class TestCulture
{
    [ModuleInitializer]
    internal static void English()
    {
        var us = CultureInfo.GetCultureInfo("en-US");
        CultureInfo.DefaultThreadCurrentCulture = us;
        CultureInfo.DefaultThreadCurrentUICulture = us;
        CultureInfo.CurrentCulture = us;
        CultureInfo.CurrentUICulture = us;
    }
}
