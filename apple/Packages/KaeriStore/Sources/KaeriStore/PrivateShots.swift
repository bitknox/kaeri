// Private mode for screenshots: the Fixtures app and the dev harness say `-KaeriPrivate <state>`,
// and the store turns it on as a person would.

import Foundation
import KaeriKit

/// The states `-KaeriPrivate` stages.
///
///     on       private mode on, from the switch: no notice
///     notice   private mode on from the menu item or the shortcut: the notice up
///     kept     on, with the open title's place held in memory ("Your place is kept")
///     always   the title `-KaeriTitle` names (else 900001) always private, the mode off
///     series   the series `-KaeriWork` names always private, the mode off
public enum PrivateScene: String, Sendable {
    case on, notice, kept, always, series

    public static let argument = "-KaeriPrivate"

    public static func named(in arguments: [String] = ProcessInfo.processInfo.arguments) -> PrivateScene? {
        guard let index = arguments.firstIndex(of: argument), arguments.indices.contains(index + 1)
        else { return nil }
        return PrivateScene(rawValue: arguments[index + 1])
    }

    /// The value after `flag` in the launch arguments.
    static func value(_ flag: String, in arguments: [String]) -> String? {
        guard let index = arguments.firstIndex(of: flag), arguments.indices.contains(index + 1) else { return nil }
        return arguments[index + 1]
    }
}

extension AppStore {
    /// Stage `scene`, and give the engine a moment to answer.
    public func stage(_ scene: PrivateScene, arguments: [String] = ProcessInfo.processInfo.arguments) async {
        let title = PrivateScene.value("-KaeriTitle", in: arguments).flatMap { AnilistId($0) } ?? 900_001
        switch scene {
        case .on:
            setPrivate(true)
        case .notice:
            setPrivate(true, notice: true)
        case .kept:
            // After the engine's answer, whose progress for the open title would say otherwise.
            setPrivate(true)
            try? await Task.sleep(for: .milliseconds(400))
            privacy.stageKept(title)
        case .always:
            setAlwaysPrivate(show: title, true)
        case .series:
            guard let work = PrivateScene.value("-KaeriWork", in: arguments) else { return }
            setAlwaysPrivate(series: work, true)
        }
        try? await Task.sleep(for: .milliseconds(400))
    }
}
