// `--toolbar-sweep`: the window narrowed past where the toolbar's items go into its overflow menu
// and widened again, on every section and on a title's page, while what the toolbar shows changes
// on its own as it does in use: the sync capsule's words, the activity line and private mode.
//
// AppKit ends the app ("more Update Constraints in Window passes than there are views") when a
// toolbar item's size keeps changing as the item moves in or out of the overflow menu. The sweep
// either runs to the end and prints KAERI_SWEEP_PASSED, or the process dies there.

import AppKit
import KaeriKit
import KaeriStore
import KaeriUI

@MainActor
enum ToolbarSweep {
    /// Content widths the sweep passes through, widest first: the default window, down to the
    /// window's minimum.
    static let widest: CGFloat = 1280
    static let narrowest: CGFloat = 960

    static func run(window: NSWindow, store: AppStore, navigator: Navigator, step: CGFloat) async -> Bool {
        let height = window.contentLayoutRect.height
        var tick = 0
        // KAERI_SWEEP_CHURN=sync,jobs,private picks what changes (all three unless it says), and
        // KAERI_SWEEP_TRACE=1 prints every step, for finding which change and width set it off.
        let environment = ProcessInfo.processInfo.environment
        let kinds = Set((environment["KAERI_SWEEP_CHURN"] ?? "sync,jobs,private").split(separator: ",").map(String.init))
        let trace = environment["KAERI_SWEEP_TRACE"] == "1"
        setvbuf(stdout, nil, _IOLBF, 0)
        /// What changes on its own while the window moves: every few steps the next of these.
        func churn() {
            tick += 1
            let job = JobId(9_000_000 + UInt64(tick))
            let change: (kind: String, event: Event)
            switch tick % 6 {
            case 0: change = ("sync", .sync(SyncState(tracker: "anilist", connected: true, user: "sweep", outbox: 3, deferred: 0)))
            case 1: change = ("jobs", .jobStarted(job: job, activity: "loading the sweep's list"))
            case 2: change = ("private", .private(true))
            case 3: change = ("sync", .sync(SyncState(tracker: "anilist", connected: true, user: "sweep", outbox: 0, deferred: 0)))
            case 4: change = ("jobs", .jobDone(JobId(9_000_000 + UInt64(tick - 3))))
            default: change = ("private", .private(false))
            }
            guard kinds.contains(change.kind) else { return }
            if trace { print("KaeriDev: sweep \(navigator.section.rawValue) \(Int(window.contentLayoutRect.width)) \(change.event)") }
            store.apply(change.event)
        }
        func resize(to width: CGFloat) async {
            window.setContentSize(NSSize(width: width, height: height))
            window.layoutIfNeeded()
            if trace { print("KaeriDev: sweep \(navigator.section.rawValue) \(Int(width)) overflow \(overflowed(window)) \(items(window))") }
            try? await Task.sleep(for: .milliseconds(16))
        }
        func pass(_ name: String) async {
            var widths = Array(stride(from: widest, through: narrowest, by: -step))
            widths += widths.reversed()
            var overflowSeen = 0
            for (index, width) in widths.enumerated() {
                await resize(to: width)
                if index % 3 == 0 { churn() }
                overflowSeen = max(overflowSeen, overflowed(window))
            }
            print("KaeriDev: sweep \(name): at most \(overflowSeen) toolbar items in the overflow menu")
        }

        // KAERI_SWEEP_SECTIONS=library,downloads sweeps only those.
        let named = environment["KAERI_SWEEP_SECTIONS"].map { $0.split(separator: ",").compactMap { AppSection(rawValue: String($0)) } }
        let sections = named ?? AppSection.allCases
        for section in sections {
            navigator.section = section
            try? await Task.sleep(for: .milliseconds(300))
            await pass(section.rawValue)
        }
        // A title's page over the section, whose toolbar is its own.
        navigator.section = .home
        navigator.openTitle(900001)
        try? await Task.sleep(for: .milliseconds(800))
        await pass("title")
        navigator.back()
        try? await Task.sleep(for: .milliseconds(500))
        // Narrow, from section to section, as a click in the sidebar changes them.
        await resize(to: narrowest)
        for round in 0..<3 {
            for section in sections {
                navigator.section = section
                churn()
                try? await Task.sleep(for: .milliseconds(round == 0 ? 120 : 30))
            }
        }
        // And back and forth across a few points either side of each width, the drag a person
        // makes along the edge of where an item goes in or out.
        for section in sections {
            navigator.section = section
            try? await Task.sleep(for: .milliseconds(150))
            for width in stride(from: narrowest, through: widest, by: 24) {
                for wiggle in [CGFloat(-3), 3, -1, 1, 0] {
                    await resize(to: max(narrowest, width + wiggle))
                }
                churn()
            }
        }
        store.apply(.private(false))
        return true
    }

    /// Each toolbar item, + in the bar or - in the overflow menu, with its view's width and the
    /// width it fits in.
    static func items(_ window: NSWindow) -> String {
        (window.toolbar?.items ?? []).map { item in
            let width = Int(item.view?.frame.width ?? -1)
            let fit = item.view.map { Int($0.fittingSize.width) } ?? -1
            return "\(item.isVisible ? "+" : "-")\(item.itemIdentifier.rawValue.suffix(6)):\(width)/\(fit)"
        }.joined(separator: " ")
    }

    /// How many of the toolbar's items are in its overflow menu now.
    static func overflowed(_ window: NSWindow) -> Int {
        (window.toolbar?.items ?? []).filter { !$0.isVisible }.count
    }
}
