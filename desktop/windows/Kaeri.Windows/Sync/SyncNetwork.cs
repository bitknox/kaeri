using System.Runtime.InteropServices;
using Kaeri.Engine.Ffi;
using Kaeri.Windows.Tv;

namespace Kaeri.Windows.Sync;

/// <summary>
/// Pairing by a typed code over Windows' DNS-SD, <c>_kaeri-sync._tcp</c> in <c>local</c>. The
/// browse is the device watcher Play on TV uses. The advertisement is dnsapi's
/// <c>DnsServiceRegister</c>, which publishes a port the engine already listens on, where
/// <c>DnssdServiceInstance</c> would want a socket of its own. Called on the engine's threads.
/// </summary>
sealed class SyncNetwork : FfiSyncNetwork
{
    const string Service = "_kaeri-sync._tcp.local";

    readonly DnssdWatch watch = new("_kaeri-sync._tcp", "sync");
    readonly object gate = new();
    readonly Dictionary<string, Registration> registered = [];

    public void Browse(FfiSyncSink sink) =>
        watch.Start((instance, addresses, port, txt) => sink.Found(instance, addresses, port, txt.Select(p => new FfiSyncTxt(p.Key, p.Value)).ToArray()), sink.Lost);

    public void StopBrowsing() => watch.Stop();

    public void Advertise(FfiSyncAdvert advert)
    {
        lock (gate)
        {
            WithdrawLocked(advert.Instance);
            try
            {
                if (Registration.Start(advert, Service) is { } made)
                    registered[advert.Instance] = made;
            }
            catch (Exception e) when (e is DllNotFoundException or EntryPointNotFoundException)
            {
                // No DNS-SD here: the code still pairs by address or QR code.
                KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "sync", $"the pairing advertisement did not start: {e.Message}");
            }
        }
    }

    public void Withdraw(string instance)
    {
        lock (gate)
            WithdrawLocked(instance);
    }

    void WithdrawLocked(string instance)
    {
        if (registered.Remove(instance, out var was))
            was.Stop();
    }

    /// <summary>
    /// One registration, held in unmanaged memory from <c>DnsServiceRegister</c> until
    /// <c>DnsServiceDeRegister</c>, as dnsapi asks.
    /// </summary>
    sealed class Registration
    {
        const uint Pending = 9506; // DNS_REQUEST_PENDING
        const uint Version1 = 1; // DNS_QUERY_REQUEST_VERSION1

        [StructLayout(LayoutKind.Sequential)]
        struct RegisterRequest
        {
            public uint Version;
            public uint InterfaceIndex;
            public IntPtr Instance;
            public IntPtr Complete;
            public IntPtr Context;
            public IntPtr Credentials;
            public int UnicastEnabled;
        }

        [UnmanagedFunctionPointer(CallingConvention.Winapi)]
        delegate void Completed(uint status, IntPtr context, IntPtr instance);

        // Kept for the life of the process: dnsapi calls it on its own threads. The instance it
        // hands back is left alone, since registering and withdrawing both report through here.
        static readonly Completed Done = (status, _, _) =>
        {
            if (status != 0)
                KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "sync", $"the pairing advertisement ended with {status}");
        };

        static readonly IntPtr DonePointer = Marshal.GetFunctionPointerForDelegate(Done);

        [DllImport("dnsapi.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
        static extern IntPtr DnsServiceConstructInstance(string serviceName, string hostName, IntPtr ip4, IntPtr ip6, ushort port, ushort priority, ushort weight, uint properties,
            [MarshalAs(UnmanagedType.LPArray, ArraySubType = UnmanagedType.LPWStr)] string[] keys,
            [MarshalAs(UnmanagedType.LPArray, ArraySubType = UnmanagedType.LPWStr)] string[] values);

        [DllImport("dnsapi.dll", ExactSpelling = true)]
        static extern void DnsServiceFreeInstance(IntPtr instance);

        [DllImport("dnsapi.dll", ExactSpelling = true)]
        static extern uint DnsServiceRegister(IntPtr request, IntPtr cancel);

        [DllImport("dnsapi.dll", ExactSpelling = true)]
        static extern uint DnsServiceDeRegister(IntPtr request, IntPtr cancel);

        IntPtr request;

        public static Registration? Start(FfiSyncAdvert advert, string service)
        {
            var name = $"{Label(advert.Instance)}.{service}";
            var host = $"{Environment.MachineName}.local";
            var keys = advert.Txt.Select(t => t.Key).ToArray();
            var values = advert.Txt.Select(t => t.Value).ToArray();
            var made = DnsServiceConstructInstance(name, host, IntPtr.Zero, IntPtr.Zero, advert.Port, 0, 0, (uint)keys.Length, keys, values);
            if (made == IntPtr.Zero)
                return null;
            var request = Marshal.AllocHGlobal(Marshal.SizeOf<RegisterRequest>());
            Marshal.StructureToPtr(new RegisterRequest { Version = Version1, Instance = made, Complete = DonePointer }, request, false);
            var said = DnsServiceRegister(request, IntPtr.Zero);
            if (said != Pending)
            {
                KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "sync", $"the pairing advertisement was refused with {said}");
                Marshal.FreeHGlobal(request);
                DnsServiceFreeInstance(made);
                return null;
            }
            return new Registration { request = request };
        }

        public void Stop()
        {
            if (request == IntPtr.Zero)
                return;
            DnsServiceDeRegister(request, IntPtr.Zero);
            // The request and the instance stay allocated: dnsapi may still read them while the
            // goodbye goes out, and a pairing code is shown a handful of times in a run.
            request = IntPtr.Zero;
        }

        /// <summary>An instance name as one DNS label: dots and backslashes escaped.</summary>
        static string Label(string instance) => instance.Replace("\\", "\\\\").Replace(".", "\\.");
    }
}
