import AppKit
import ObjectiveC

/// Shots only: draws a window as it looks key when macOS will not let it be key, such as while
/// the screen is locked. AppKit asks the window and the app whether they are key, main and
/// active; each answer is replaced with yes for this process, and the window told it became key.
enum KeyLook {
    @MainActor
    static func force(_ window: NSWindow) {
        let yes: @convention(block) (AnyObject) -> Bool = { _ in true }
        let imp = imp_implementationWithBlock(yes)
        for name in ["isKeyWindow", "isMainWindow", "_hasActiveAppearance", "_hasKeyAppearance",
                     "_hasMainAppearance", "_hasActiveControls", "_hasActiveAppearanceIgnoringKeyFocus"] {
            replace(name, in: type(of: window), with: imp)
        }
        for name in ["isActive", "_isActive"] {
            replace(name, in: NSApplication.self, with: imp)
        }
        NotificationCenter.default.post(name: NSApplication.didBecomeActiveNotification, object: NSApp)
        NotificationCenter.default.post(name: NSWindow.didBecomeMainNotification, object: window)
        NotificationCenter.default.post(name: NSWindow.didBecomeKeyNotification, object: window)
        window.contentView?.superview?.needsDisplay = true
        window.contentView?.superview?.needsLayout = true
        window.update()
    }

    private static func replace(_ name: String, in cls: AnyClass, with imp: IMP) {
        let selector = NSSelectorFromString(name)
        guard let method = class_getInstanceMethod(cls, selector) else { return }
        class_replaceMethod(cls, selector, imp, method_getTypeEncoding(method))
    }
}
