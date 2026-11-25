// Which way the iPhone may turn, from what covers the app: the app runs in portrait, the player
// turns it to landscape (T2), and the reader follows the sensor while it is open and gives the
// app back in portrait. The iPad stays as it is held. The app delegate answers
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
    /// The player is in a room: it opens upright, and turning it is full screen (the room board).
    public var room = false

    public init(player: Bool = false, reader: Bool = false, room: Bool = false) {
        self.player = player
        self.reader = reader
        self.room = room
    }

    /// Portrait with nothing over the app (native-ux, iPhone). The player's landscape wins over
    /// the reader under it.
    public var allowed: Allowed {
        if player { return room ? .either : .landscape }
        if reader { return .either }
        return .portrait
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

    /// The player went into a room or left it. Into one, the screen turns upright once, then
    /// follows the sensor.
    public static func room(_ on: Bool) {
        let entering = on && !now.room && now.player
        change { $0.room = on }
        guard entering, UIDevice.current.userInterfaceIdiom == .phone,
              let scene = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first
        else { return }
        scene.requestGeometryUpdate(.iOS(interfaceOrientations: .portrait)) { _ in }
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
        // The reader follows the sensor from where it is.
        case .either: nil
        }
        if let turn {
            scene.requestGeometryUpdate(.iOS(interfaceOrientations: turn)) { _ in }
        }
    }
}
#endif
