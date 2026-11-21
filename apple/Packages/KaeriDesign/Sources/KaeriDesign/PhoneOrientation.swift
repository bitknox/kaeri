// Which way the iPhone may turn, from what covers the app: the player turns it to landscape (T2)
// and the reader follows the sensor. The iPad stays as it is held. The app delegate answers
// `supportedInterfaceOrientationsFor` with `PhoneOrientation.mask`.

#if os(iOS)
import UIKit
#endif

/// What covers the app, and so which way the iPhone may turn.
public struct PhoneOrientation: Equatable, Sendable {
    /// Which way the screen may face.
    public enum Allowed: Equatable, Sendable {
        case portrait, landscape, either
    }

    /// The player is up.
    public var player = false
    /// The reader is open.
    public var reader = false

    public init(player: Bool = false, reader: Bool = false) {
        self.player = player
        self.reader = reader
    }

    /// The player's landscape wins over the reader under it.
    public var allowed: Allowed {
        if player { return .landscape }
        return .either
    }
}

#if os(iOS)
@MainActor
extension PhoneOrientation {
    /// What covers the app now.
    public private(set) static var now = PhoneOrientation()

    /// What the app delegate answers for every window.
    public static var mask: UIInterfaceOrientationMask {
        guard UIDevice.current.userInterfaceIdiom == .phone else { return .allButUpsideDown }
        return switch now.allowed {
        case .portrait: .portrait
        case .landscape: .landscape
        case .either: .allButUpsideDown
        }
    }

    /// The player came up or went.
    public static func player(_ on: Bool) {
        change { $0.player = on }
    }

    /// The reader opened or closed.
    public static func reader(_ open: Bool) {
        change { $0.reader = open }
    }

    /// Applies the change and turns the screen when it now has to face one way.
    private static func change(_ apply: (inout PhoneOrientation) -> Void) {
        let before = now.allowed
        apply(&now)
        let after = now.allowed
        guard after != before, UIDevice.current.userInterfaceIdiom == .phone,
            let scene = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first
        else { return }
        for window in scene.windows {
            window.rootViewController?.setNeedsUpdateOfSupportedInterfaceOrientations()
        }
        let turn: UIInterfaceOrientationMask? = switch after {
        case .portrait: .portrait
        case .landscape: .landscape
        case .either: before == .landscape ? .portrait : nil
        }
        if let turn {
            scene.requestGeometryUpdate(.iOS(interfaceOrientations: turn)) { _ in }
        }
    }
}
#endif
