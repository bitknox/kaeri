using System.Diagnostics;
using System.Reflection;
using System.Runtime.InteropServices;
using Kaeri.App;
using Kaeri.Engine.Core;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.Win32.SafeHandles;

namespace Kaeri.Windows;

/// <summary>
/// Crash reports, kept on this device while Save crash reports is on and not at all otherwise.
/// Managed exceptions (the window's, the domain's, a task's) and native faults (through
/// <c>SetUnhandledExceptionFilter</c>, with a minidump that stays on the device) are handed to the
/// engine's <c>CrashSave</c>, which redacts them and keeps at most ten for thirty days. With
/// saving off every path here returns before it reads, builds or writes anything. Nothing is sent.
/// </summary>
static partial class CrashGuard
{
    /// <summary>Where the app is when a crash happens, as the report words it.</summary>
    public sealed record Where(CrashPlace Place, string Detail, uint Sources, bool DemoOn);

    static volatile bool saving;
    static Func<Where>? where;
    static string? dumps;
    static readonly Stopwatch Up = Stopwatch.StartNew();
    static readonly TimeSpan Keep = TimeSpan.FromDays(30);

    /// <summary>Whether crashes are being kept now.</summary>
    public static bool Saving => saving;

    /// <summary>Hooks the handlers up. They do nothing until <see cref="Follow"/> says saving is on.</summary>
    public static void Start(FfiDirs dirs, Func<Where> state)
    {
        where = state;
        dumps = Path.Combine(dirs.Data, "crash-dumps");
        AppDomain.CurrentDomain.UnhandledException += (_, e) =>
        {
            if (e.ExceptionObject is Exception ex)
                Caught(ex);
        };
        TaskScheduler.UnobservedTaskException += (_, e) => Caught(e.Exception);
        Prune();
    }

    /// <summary>Saving on or off, as the settings say. Turning it off stops everything at once.</summary>
    public static void Follow(bool on)
    {
        if (on == saving)
            return;
        saving = on;
        if (on)
            Hook();
    }

    /// <summary>Removes every kept dump, with the saved reports they belong to.</summary>
    public static void ForgetDumps()
    {
        try
        {
            if (dumps is not null && Directory.Exists(dumps))
                Directory.Delete(dumps, true);
        }
        catch (Exception)
        {
        }
    }

    /// <summary>A managed exception reached a handler. Saved only while saving is on.</summary>
    public static void Caught(Exception e)
    {
        if (!saving)
            return;
        try
        {
            var frames = new System.Diagnostics.StackTrace(e, false).GetFrames()
                .Select(f => f.GetMethod())
                .OfType<MethodBase>()
                .Take(32)
                .Select(m => new CrashFrame(m.Module.Name, 0, $"{m.DeclaringType?.FullName}.{m.Name}"))
                .ToArray();
            Save(CrashKind.Exception, e.GetType().FullName ?? "exception", frames);
        }
        catch (Exception)
        {
            // The report must not throw over the error it records.
        }
    }

    static void Save(CrashKind kind, string reason, CrashFrame[] frames)
    {
        var at = where?.Invoke() ?? new Where(CrashPlace.Other, "", 0, false);
        KaeriFfiMethods.CrashSave(new CrashRecord(
            Id: "",
            AtMs: DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(),
            UptimeSecs: (ulong)Up.Elapsed.TotalSeconds,
            Platform: "Windows",
            OsVersion: $"Windows {Environment.OSVersion.Version}",
            AppVersion: Updater.Current,
            Arch: RuntimeInformation.ProcessArchitecture.ToString().ToLowerInvariant(),
            Place: at.Place,
            Detail: at.Detail,
            Kind: kind,
            Reason: reason,
            Sources: at.Sources,
            DemoOn: at.DemoOn,
            Origin: CrashOrigin.Saved,
            Frames: frames));
    }

    static void Prune()
    {
        try
        {
            if (dumps is null || !Directory.Exists(dumps))
                return;
            foreach (var file in Directory.EnumerateFiles(dumps))
                if (DateTime.UtcNow - File.GetLastWriteTimeUtc(file) > Keep)
                    File.Delete(file);
        }
        catch (Exception)
        {
        }
    }

    // Native faults.

    static bool hooked;

    static unsafe void Hook()
    {
        if (hooked)
            return;
        hooked = true;
        SetUnhandledExceptionFilter((IntPtr)(delegate* unmanaged<IntPtr, int>)&Filter);
    }

    [UnmanagedCallersOnly]
    static int Filter(IntPtr pointers)
    {
        // Another handler may follow: the filter keeps what it can and lets the search go on.
        try
        {
            if (!saving || pointers == IntPtr.Zero)
                return 0;
            var record = Marshal.ReadIntPtr(pointers);
            var code = (uint)Marshal.ReadInt32(record);
            var address = Marshal.ReadIntPtr(record, IntPtr.Size == 8 ? 16 : 12);
            var (module, offset) = ModuleOf(address);
            WriteDump(pointers);
            Save(CrashKind.Signal, $"0x{code:X8}", [new CrashFrame(module, offset)]);
        }
        catch (Exception)
        {
        }
        return 0;
    }

    static (string Module, ulong Offset) ModuleOf(IntPtr address)
    {
        const uint FromAddress = 0x4, UnchangedRefCount = 0x2;
        if (!GetModuleHandleExW(FromAddress | UnchangedRefCount, address, out var module) || module == IntPtr.Zero)
            return ("unknown", (ulong)address);
        var name = new char[260];
        var length = GetModuleFileNameW(module, name, name.Length);
        var file = length > 0 ? Path.GetFileName(new string(name, 0, (int)length)) : "unknown";
        return (file, (ulong)((long)address - (long)module));
    }

    [StructLayout(LayoutKind.Sequential, Pack = 4)]
    struct DumpException
    {
        public uint ThreadId;
        public IntPtr Pointers;
        public int ClientPointers;
    }

    static void WriteDump(IntPtr pointers)
    {
        if (dumps is null)
            return;
        Directory.CreateDirectory(dumps);
        var path = Path.Combine(dumps, $"crash-{DateTimeOffset.UtcNow.ToUnixTimeSeconds()}.dmp");
        using var file = new FileStream(path, FileMode.Create, FileAccess.Write);
        var info = new DumpException { ThreadId = GetCurrentThreadId(), Pointers = pointers };
        // MiniDumpNormal: stacks and module list, no heap, so nothing the viewer typed or watched.
        MiniDumpWriteDump(GetCurrentProcess(), GetCurrentProcessId(), file.SafeFileHandle, 0, ref info, IntPtr.Zero, IntPtr.Zero);
    }

    [LibraryImport("kernel32.dll")]
    private static partial IntPtr SetUnhandledExceptionFilter(IntPtr filter);

    [LibraryImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool GetModuleHandleExW(uint flags, IntPtr address, out IntPtr module);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
    private static extern uint GetModuleFileNameW(IntPtr module, char[] name, int size);

    [LibraryImport("kernel32.dll")]
    private static partial IntPtr GetCurrentProcess();

    [LibraryImport("kernel32.dll")]
    private static partial uint GetCurrentProcessId();

    [LibraryImport("kernel32.dll")]
    private static partial uint GetCurrentThreadId();

    [DllImport("dbghelp.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool MiniDumpWriteDump(IntPtr process, uint processId, SafeFileHandle file, int type, ref DumpException exception, IntPtr user, IntPtr callback);
}
