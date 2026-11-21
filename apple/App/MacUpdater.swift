// The Mac's updater: Sparkle 2, driven by the app's own Settings row and notice instead of its
// windows. The app is signed ad hoc, with no Developer ID, so what Sparkle trusts is the EdDSA
// signature each release's appcast gives its .dmg, checked against SUPublicEDKey in Info.plist
// before the image is opened (SUVerifyUpdateBeforeExtraction). The feed itself is signed too
// (SURequireSignedFeed).
//
// Sparkle never looks on its own (SUEnableAutomaticChecks is off): the engine's daily check
// says when there is a release, and Sparkle reads its feed only when the viewer presses Update.
// Even then it installs only what the viewer asked for (UpdateGate). Its installer swaps the
// bundle in place, takes the new one out of quarantine so macOS does not ask for Open Anyway
// again, and relaunches the app.

#if os(macOS) && !FIXTURES
import KaeriKit
import KaeriStore
import KaeriStrings
import Foundation
import Sparkle

@MainActor
final class MacUpdater: NSObject, UpdateInstaller, SPUUserDriver, SPUUpdaterDelegate {
    private var updater: SPUUpdater?
    private var report: (@MainActor (UpdateProgress) -> Void)?
    private var gate = UpdateGate()
    /// The version the gate took from the feed, until the update ends.
    private var found: String?
    private var expected: UInt64?
    private var received: UInt64 = 0
    /// Stop the feed check while it runs, and the download while that runs. Unpacking has no
    /// stop: an update stopped then is skipped once it is ready to install.
    private var cancelCheck: (() -> Void)?
    private var cancelDownload: (() -> Void)?

    override init() {
        super.init()
        let updater = SPUUpdater(hostBundle: .main, applicationBundle: .main, userDriver: self, delegate: self)
        updater.automaticallyChecksForUpdates = false
        updater.automaticallyDownloadsUpdates = false
        do {
            try updater.start()
            self.updater = updater
        } catch {
            AppLog.error("updates", "Sparkle did not start: \(error.localizedDescription)")
        }
    }

    // MARK: UpdateInstaller

    func install(_ release: Release, progress: @escaping @MainActor (UpdateProgress) -> Void) {
        report = progress
        guard let updater, updater.canCheckForUpdates else {
            progress(.failed(L.updatesInstallerNotReady))
            return
        }
        gate.arm(release.version)
        AppLog.info("updates", "updating to \(release.version)")
        updater.checkForUpdates()
    }

    func cancel() {
        cancelCheck?()
        cancelDownload?()
        end(nil)
    }

    // MARK: SPUUpdaterDelegate

    /// The feed of the release the viewer pressed Update for, which is the one the engine
    /// offered: a beta is in no `releases/latest` feed. Every appcast is signed the same way,
    /// and `UpdateGate` still installs only the version asked for. Without one, Info.plist's
    /// `SUFeedURL` stands.
    nonisolated func feedURLString(for updater: SPUUpdater) -> String? {
        MainActor.assumeIsolated {
            gate.armed.flatMap { UpdatesModel.feedURL(forVersion: $0)?.absoluteString }
        }
    }

    /// Word of the update reaches Settings while it runs. Once it ended or was stopped, what
    /// Sparkle still does with it says nothing.
    private func say(_ progress: UpdateProgress) {
        guard gate.armed != nil else { return }
        report?(progress)
    }

    /// The update ended or was stopped: say how, once, and keep nothing of it.
    private func end(_ last: UpdateProgress?) {
        if let last { say(last) }
        gate.disarm()
        found = nil
        cancelCheck = nil
        cancelDownload = nil
    }

    private func failed(_ error: any Error) {
        AppLog.error("updates", "the update stopped: \(error.localizedDescription)")
        end(.failed(error.localizedDescription))
    }

    // MARK: SPUUserDriver

    func show(_ request: SPUUpdatePermissionRequest, reply: @escaping (SUUpdatePermissionResponse) -> Void) {
        // Sparkle's own scheduled checks stay off: the engine's check is the one that runs.
        reply(SUUpdatePermissionResponse(automaticUpdateChecks: false, sendSystemProfile: false))
    }

    func showUserInitiatedUpdateCheck(cancellation: @escaping () -> Void) {
        cancelCheck = cancellation
        say(.preparing)
    }

    func showUpdateFound(with appcastItem: SUAppcastItem, state: SPUUserUpdateState, reply: @escaping (SPUUserUpdateChoice) -> Void) {
        // The feed check is over.
        cancelCheck = nil
        let version = appcastItem.displayVersionString
        let choice = gate.choice(found: version, informationOnly: appcastItem.isInformationOnlyUpdate)
        AppLog.info("updates", "the feed has \(version): \(choice)")
        switch choice {
        case .install:
            found = version
            reply(.install)
        case .dismiss:
            reply(.dismiss)
            end(.failed(L.updatesFeedMissing))
        case .stop:
            // Skip stops one that began installing earlier, which Dismiss leaves to install when
            // the app quits. Before that, Dismiss is enough, and Skip would also mark the version.
            reply(state.stage == .installing ? .skip : .dismiss)
            end(.failed(L.updatesFeedMissing))
        }
    }

    func showUpdateReleaseNotes(with downloadData: SPUDownloadData) {}

    func showUpdateReleaseNotesFailedToDownloadWithError(_ error: any Error) {}

    func showUpdateNotFoundWithError(_ error: any Error, acknowledgement: @escaping () -> Void) {
        acknowledgement()
        end(.failed(L.updatesFeedMissing))
    }

    func showUpdaterError(_ error: any Error, acknowledgement: @escaping () -> Void) {
        acknowledgement()
        failed(error)
    }

    func showDownloadInitiated(cancellation: @escaping () -> Void) {
        cancelDownload = cancellation
        expected = nil
        received = 0
        say(.downloading(received: 0, expected: nil))
    }

    func showDownloadDidReceiveExpectedContentLength(_ expectedContentLength: UInt64) {
        expected = expectedContentLength > 0 ? expectedContentLength : nil
        say(.downloading(received: received, expected: expected))
    }

    func showDownloadDidReceiveData(ofLength length: UInt64) {
        received += length
        say(.downloading(received: received, expected: expected))
    }

    func showDownloadDidStartExtractingUpdate() {
        cancelDownload = nil
        say(.unpacking(0))
    }

    func showExtractionReceivedProgress(_ progress: Double) {
        say(.unpacking(progress))
    }

    func showReady(toInstallAndRelaunch reply: @escaping (SPUUserUpdateChoice) -> Void) {
        // Only the update the viewer pressed Update for gets this far, and they asked for it now,
        // unless they stopped it while it unpacked. Skip stops it then: Dismiss would leave it to
        // install when the app quits.
        guard let found, gate.choice(found: found, informationOnly: false) == .install else {
            reply(.skip)
            return
        }
        say(.relaunching)
        reply(.install)
    }

    func showInstallingUpdate(withApplicationTerminated applicationTerminated: Bool, retryTerminatingApplication: @escaping () -> Void) {
        say(.relaunching)
    }

    func showUpdateInstalledAndRelaunched(_ relaunched: Bool, acknowledgement: @escaping () -> Void) {
        acknowledgement()
    }

    func showUpdateInFocus() {}

    func dismissUpdateInstallation() {
        // The session ended without installing: back to the offer, unless it already said why.
        end(.idle)
    }
}
#endif
