// "Report a bug": the address of a pre-filled GitHub issue, which the app opens in the browser.
// The engine writes the text, the same on every front end, and strips what is private from it;
// this adds what only the app knows about itself.

import KaeriKit
import Foundation
#if canImport(UIKit)
import UIKit
#endif

/// What a bug report says about this app and this device.
public struct BugReportDevice: Equatable, Sendable {
    public var appVersion: String
    public var platform: String
    public var osVersion: String
    public var device: DeviceClass
    public var build: String

    public init(appVersion: String, platform: String, osVersion: String, device: DeviceClass, build: String) {
        self.appVersion = appVersion
        self.platform = platform
        self.osVersion = osVersion
        self.device = device
        self.build = build
    }

    /// The device this process runs on, with the version its bundle says.
    public static func current(bundle: Bundle = .main) -> BugReportDevice {
        let system = ProcessInfo.processInfo.operatingSystemVersion
        let version = "\(system.majorVersion).\(system.minorVersion)"
        #if os(macOS)
        let (platform, device) = ("macOS", DeviceClass.computer)
        #else
        let pad = UIDevice.current.userInterfaceIdiom == .pad
        let (platform, device) = pad ? ("iPadOS", DeviceClass.tablet) : ("iOS", DeviceClass.phone)
        #endif
        #if DEBUG
        var build = "debug"
        #else
        var build = "release"
        #endif
        if bundle.bundleIdentifier?.hasSuffix(".fixtures") == true { build = "fixtures" }
        return BugReportDevice(
            appVersion: bundle.infoDictionary?["CFBundleShortVersionString"] as? String ?? "",
            platform: platform, osVersion: version, device: device, build: build
        )
    }
}

/// An error a report starts from: what kind of problem, and what it said.
public struct BugContext: Equatable, Sendable {
    public var kind: String
    public var message: String
    /// The title of the show it was about, if any, which the report removes wherever it appears.
    public var title: String?

    public init(kind: String, message: String, title: String? = nil) {
        self.kind = kind
        self.message = message
        self.title = title
    }
}

extension AppStore {
    /// The address of a new issue for this device, with `context` as its last error and the saved
    /// crashes `crashes` in it. `playerLog` adds the line that says the log is attached by hand.
    public func bugReportURL(
        context: BugContext? = nil, crashes: [String] = [], playerLog: Bool = false, device: BugReportDevice = .current()
    ) -> URL {
        BugReports.url(device: device, providers: providers.rows, context: context, crashes: crashes, playerLog: playerLog)
    }

    /// The whole report as the view shows it and Copy report puts it on the clipboard.
    public func bugReportText(
        context: BugContext? = nil, crashes: [String] = [], playerLog: Bool = false, device: BugReportDevice = .current()
    ) -> String {
        reportText(report: bugReport(context: context, crashes: crashes, playerLog: playerLog, device: device))
    }

    /// The report itself, for the calls that read it.
    public func bugReport(
        context: BugContext? = nil, crashes: [String] = [], playerLog: Bool = false, device: BugReportDevice = .current()
    ) -> BugReport {
        BugReports.report(device: device, providers: providers.rows, context: context, crashes: crashes, playerLog: playerLog)
    }
}

public enum BugReports {
    public static func report(
        device: BugReportDevice, providers: [ProviderRow], context: BugContext?, crashes: [String], playerLog: Bool = false
    ) -> BugReport {
        BugReport(
            appVersion: device.appVersion, platform: device.platform, osVersion: device.osVersion,
            device: device.device, build: device.build, providers: providers,
            error: context.map { BugError(kind: $0.kind, message: $0.message) },
            private: context?.title.map { [$0] } ?? [], crashes: crashes, playerLog: playerLog
        )
    }

    /// The address for `device`, the engine's sources, an optional error and the crashes named.
    public static func url(
        device: BugReportDevice, providers: [ProviderRow], context: BugContext?, crashes: [String] = [], playerLog: Bool = false
    ) -> URL {
        let report = report(device: device, providers: providers, context: context, crashes: crashes, playerLog: playerLog)
        // The engine's address is always well formed; the project's tracker is the fallback.
        return URL(string: bugReportUrl(report: report)) ?? URL(string: "https://github.com/bitknox/kaeri/issues/new")!
    }

    /// The kind of error a failed playback gives a report: Player log starts ticked for it.
    public static let playbackError = "playback"

    /// What the player log holds now, for the Player log row. Cached by the engine until the log
    /// changes, so it is cheap when Report opens.
    public static func playerLogStats() -> PlayerLogStats { KaeriKit.playerLogStats() }

    /// Whether Player log starts ticked: the report starts from a playback error, or carries a
    /// crash in the player.
    public static func playerLogStartsTicked(_ report: BugReport) -> Bool {
        KaeriKit.playerLogStartsTicked(report: report)
    }

    /// The player log's file text, redacted. Up to a megabyte goes through the redaction, so this
    /// runs off the main actor.
    @concurrent public static func playerLogText(_ report: BugReport) async -> String {
        KaeriKit.playerLog(report: report)
    }

    /// Writes `text` to `url` off the main actor: whether it was written.
    @concurrent public static func write(_ text: String, to url: URL) async -> Bool {
        do {
            try Data(text.utf8).write(to: url, options: .atomic)
            return true
        } catch {
            AppLog.warn("report", "couldn't save the player log: error \((error as NSError).domain) \((error as NSError).code)")
            return false
        }
    }
}
