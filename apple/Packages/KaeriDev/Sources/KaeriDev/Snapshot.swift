// The window rendered to a PNG from inside the process, which needs no Screen Recording
// permission: AppKit draws the window's views into a bitmap of their own.

import AppKit
import Darwin
import WebKit

enum Snapshot {
    enum Problem: Error, CustomStringConvertible {
        case noWindow
        case noBitmap
        case noPNG

        var description: String {
            switch self {
            case .noWindow: "there is no window to render"
            case .noBitmap: "the window gave no bitmap to render into"
            case .noPNG: "the bitmap could not be written as a PNG"
            }
        }
    }

    /// The app's main window: the visible one that is not a panel, nor the window Picture in
    /// Picture's frames are staged in.
    @MainActor
    static var window: NSWindow? {
        NSApp.windows.first {
            $0.isVisible && !($0 is NSPanel) && $0.identifier?.rawValue != "picture-in-picture-stage"
        }
    }

    /// `CGWindowListCreateImage`, which the SDK marks unavailable in favour of ScreenCaptureKit
    /// but which still renders a process's own window, glass included, without asking for Screen
    /// Recording, which ScreenCaptureKit does. Looked up at run time; `nil` where it is gone.
    private typealias CreateImage = @convention(c) (CGRect, UInt32, UInt32, UInt32) -> Unmanaged<CGImage>?
    private static let createImage: CreateImage? = {
        guard let handle = dlopen("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics", RTLD_NOW),
              let symbol = dlsym(handle, "CGWindowListCreateImage")
        else { return nil }
        return unsafeBitCast(symbol, to: CreateImage.self)
    }()

    /// Whether the login session's screen is locked, when the window server's pictures of a
    /// window come back blank.
    static var screenLocked: Bool {
        let session = CGSessionCopyCurrentDictionary() as? [String: Any]
        return session?["CGSSessionScreenIsLocked"] as? Bool ?? false
    }

    /// `window` as the window server composites it, at its backing scale.
    @MainActor
    static func composited(_ window: NSWindow) -> CGImage? {
        // kCGWindowListOptionIncludingWindow, and kCGWindowImageBoundsIgnoreFraming with
        // kCGWindowImageBestResolution.
        let image = createImage?(.null, 1 << 3, UInt32(window.windowNumber), (1 << 0) | (1 << 3))
        return image?.takeRetainedValue()
    }

    /// Render `window` to `url`: as the window server composites it when that is allowed, and
    /// drawn by AppKit otherwise, which leaves out what the window server draws itself, such as
    /// glass.
    @MainActor
    static func write(_ window: NSWindow, to url: URL) throws {
        // A locked screen's window server composites every window blank: AppKit draws it then.
        // A page in a web view is drawn by WebKit alone, which the window server's picture can leave blank.
        if !screenLocked, webPictures.isEmpty, let image = composited(window).flatMap({ withSheets($0, of: window) }), image.width > 1 {
            let bitmap = NSBitmapImageRep(cgImage: image)
            guard let png = bitmap.representation(using: .png, properties: [:]) else { throw Problem.noPNG }
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(), withIntermediateDirectories: true
            )
            try png.write(to: url)
            return
        }
        try drawn(window, to: url)
    }

    /// `image` of `window` with the sheet over it, that sheet's own, and the popovers riding on
    /// it, where the window server draws them: each is a window of its own, which a capture of one
    /// window leaves out. The picture grows to take in a popover that stands past the window's
    /// edge. `drawn` takes each as AppKit draws it, rather than as the window server composites it.
    @MainActor
    static func withSheets(_ image: CGImage, of window: NSWindow, drawn: Bool = false) -> CGImage? {
        var sheets: [NSWindow] = []
        var next = window.attachedSheet
        while let sheet = next {
            sheets.append(sheet)
            next = sheet.attachedSheet
        }
        let popovers = (window.childWindows ?? []).filter { $0.isVisible && !$0.isSheet && !sheets.contains($0) }
        guard !sheets.isEmpty || !popovers.isEmpty else { return image }
        let scale = window.backingScaleFactor
        // The canvas, in screen points from the bottom left as Core Graphics draws: the window,
        // and whatever of a popover stands past it.
        let canvas = popovers.reduce(window.frame) { $0.union(drawn ? pane($1) : $1.frame) }
        guard let context = CGContext(
            data: nil, width: Int(canvas.width * scale), height: Int(canvas.height * scale), bitsPerComponent: 8,
            bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { return image }
        func place(_ picture: CGImage, at frame: NSRect) {
            context.draw(picture, in: CGRect(
                x: (frame.minX - canvas.minX) * scale, y: (frame.minY - canvas.minY) * scale,
                width: CGFloat(picture.width), height: CGFloat(picture.height)))
        }
        place(image, at: window.frame)
        for sheet in sheets {
            if let picture = drawn ? cached(sheet) : composited(sheet) { place(picture, at: sheet.frame) }
        }
        for popover in popovers {
            if !drawn {
                if let picture = composited(popover) { place(picture, at: popover.frame) }
                continue
            }
            // AppKit draws a popover's glass edge in no colour it has: its content, on the
            // popover's dark ground, stands in for it.
            guard let content = popover.contentView, let picture = cached(content) else { continue }
            let frame = pane(popover)
            context.saveGState()
            context.addPath(CGPath(
                roundedRect: CGRect(
                    x: (frame.minX - canvas.minX) * scale, y: (frame.minY - canvas.minY) * scale,
                    width: frame.width * scale, height: frame.height * scale),
                cornerWidth: 14 * scale, cornerHeight: 14 * scale, transform: nil))
            context.clip()
            context.setFillColor(CGColor(srgbRed: 0.15, green: 0.15, blue: 0.17, alpha: 1))
            context.fill(CGRect(x: 0, y: 0, width: context.width, height: context.height))
            place(picture, at: frame)
            context.restoreGState()
        }
        return context.makeImage()
    }

    /// A popover's content, in screen points.
    @MainActor
    private static func pane(_ popover: NSWindow) -> NSRect {
        guard let content = popover.contentView else { return popover.frame }
        return popover.convertToScreen(content.convert(content.bounds, to: nil))
    }

    /// Render `window`, its toolbar and sidebar with it, at its backing scale, to `url`, as
    /// AppKit draws its views, with its sheets and popovers drawn the same way where they stand.
    @MainActor
    static func drawn(_ window: NSWindow, to url: URL) throws {
        guard let plain = cached(window) else { throw Problem.noBitmap }
        let image = withSheets(plain, of: window, drawn: true) ?? plain
        guard let png = NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:]) else { throw Problem.noPNG }
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(), withIntermediateDirectories: true
        )
        try png.write(to: url)
    }

    /// `window`'s views as AppKit draws them, at its backing scale: the frame view, which holds
    /// the toolbar and the content beneath it.
    @MainActor
    static func cached(_ window: NSWindow) -> CGImage? {
        guard let view = window.contentView?.superview ?? window.contentView else { return nil }
        return cached(view)
    }

    /// What each web view in `window` shows, taken by WebKit itself: AppKit's own drawing leaves
    /// a web view blank, so `cached` lays these over it. Awaited before every shot.
    @MainActor
    static func snapshotWebViews(in window: NSWindow) async {
        webPictures = [:]
        var found: [WKWebView] = []
        func walk(_ view: NSView) {
            if let web = view as? WKWebView { found.append(web) }
            view.subviews.forEach(walk)
        }
        if let root = window.contentView { walk(root) }
        for web in found where !web.isHidden && web.bounds.width > 1 {
            // A locked screen can hand back an empty page at first: ask again until it has ink.
            for _ in 0..<12 {
                let config = WKSnapshotConfiguration()
                if let picture = try? await web.takeSnapshot(configuration: config),
                   let cg = picture.cgImage(forProposedRect: nil, context: nil, hints: nil) {
                    webPictures[ObjectIdentifier(web)] = cg
                    if hasInk(cg) { break }
                }
                try? await Task.sleep(for: .milliseconds(500))
            }
        }
    }

    /// Whether `image` holds dark pixels, text that is, sampled along a grid.
    private static func hasInk(_ image: CGImage) -> Bool {
        guard let data = image.dataProvider?.data, let bytes = CFDataGetBytePtr(data) else { return false }
        let bpp = image.bitsPerPixel / 8
        guard bpp >= 3 else { return false }
        for y in stride(from: 0, to: image.height, by: 3) {
            for x in stride(from: 0, to: image.width, by: 3) {
                let o = y * image.bytesPerRow + x * bpp
                if Int(bytes[o]) + Int(bytes[o + 1]) + Int(bytes[o + 2]) < 330 { return true }
            }
        }
        return false
    }

    @MainActor private(set) static var webPictures: [ObjectIdentifier: CGImage] = [:]

    /// `view` as AppKit draws it, at its window's backing scale.
    @MainActor
    static func cached(_ view: NSView) -> CGImage? {
        view.layoutSubtreeIfNeeded()
        view.displayIfNeeded()
        let bounds = view.bounds
        guard let bitmap = view.bitmapImageRepForCachingDisplay(in: bounds) else { return nil }
        view.cacheDisplay(in: bounds, to: bitmap)
        guard let base = bitmap.cgImage else { return nil }
        return withWebViews(base, in: view)
    }

    /// `base`, the picture of `view`, with the web views' pictures drawn where they stand.
    @MainActor
    private static func withWebViews(_ base: CGImage, in view: NSView) -> CGImage {
        var drawn: [(NSRect, CGImage)] = []
        func walk(_ v: NSView) {
            if let web = v as? WKWebView, let picture = webPictures[ObjectIdentifier(web)] {
                drawn.append((web.convert(web.bounds, to: view), picture))
            }
            v.subviews.forEach(walk)
        }
        walk(view)
        guard !drawn.isEmpty, let context = CGContext(
            data: nil, width: base.width, height: base.height, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { return base }
        let scale = CGFloat(base.width) / view.bounds.width
        context.draw(base, in: CGRect(x: 0, y: 0, width: base.width, height: base.height))
        // Multiplied, so what the page leaves paper-coloured keeps what AppKit drew over it.
        context.setBlendMode(.multiply)
        for (frame, picture) in drawn {
            // `convert` gives AppKit's bottom-left coordinates when the view is not flipped.
            let y = view.isFlipped ? view.bounds.height - frame.maxY : frame.minY
            context.draw(picture, in: CGRect(x: frame.minX * scale, y: y * scale,
                                            width: frame.width * scale, height: frame.height * scale))
        }
        return context.makeImage() ?? base
    }
}

extension Snapshot {
    /// Click `window` at `point`, in points from its top left, and write the window with the menu
    /// that opens over it to `url`. The click is an event handed to the window itself, so the
    /// pointer stays where it is. A menu is a window of its own that a capture of one window
    /// leaves out, and it runs a tracking loop of its own, so the picture is taken from a timer
    /// that fires in that loop, and Escape closes the menu after.
    @MainActor
    static func withMenu(at point: CGPoint, open window: NSWindow, to url: URL) async {
        let location = NSPoint(x: point.x, y: window.frame.height - point.y)
        func click(_ type: NSEvent.EventType) -> NSEvent? {
            NSEvent.mouseEvent(
                with: type, location: location, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)
        }
        await withCheckedContinuation { (done: CheckedContinuation<Void, Never>) in
            let timer = Timer(timeInterval: 0.8, repeats: false) { _ in
                MainActor.assumeIsolated {
                    if let image = composedWithMenus(window), let png = NSBitmapImageRep(cgImage: image)
                        .representation(using: .png, properties: [:]) {
                        try? png.write(to: url)
                        print("KaeriDev: wrote \(url.path)")
                    }
                    if let escape = NSEvent.keyEvent(
                        with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
                        windowNumber: window.windowNumber, context: nil, characters: "\u{1b}",
                        charactersIgnoringModifiers: "\u{1b}", isARepeat: false, keyCode: 53
                    ) { NSApp.postEvent(escape, atStart: true) }
                    done.resume()
                }
            }
            RunLoop.main.add(timer, forMode: .common)
            if let down = click(.leftMouseDown) { window.sendEvent(down) }
            if let up = click(.leftMouseUp) { window.sendEvent(up) }
        }
    }

    /// `window` and this process's menus on screen over it, in the window server's coordinates,
    /// top left first.
    @MainActor
    private static func composedWithMenus(_ window: NSWindow) -> CGImage? {
        guard let base = composited(window),
              let list = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String: Any]]
        else { return nil }
        let own = list.filter { ($0[kCGWindowOwnerPID as String] as? Int32) == getpid() }
        func bounds(_ info: [String: Any]) -> CGRect? {
            (info[kCGWindowBounds as String] as? NSDictionary).flatMap { CGRect(dictionaryRepresentation: $0 as CFDictionary) }
        }
        guard let frame = own.first(where: { ($0[kCGWindowNumber as String] as? Int) == window.windowNumber })
            .flatMap(bounds) else { return base }
        let menus: [(CGRect, CGImage)] = own.compactMap { info in
            guard (info[kCGWindowLayer as String] as? Int ?? 0) > 0,
                  let number = info[kCGWindowNumber as String] as? Int, let rect = bounds(info),
                  let image = createImage?(.null, 1 << 3, UInt32(number), (1 << 0) | (1 << 3))?.takeRetainedValue()
            else { return nil }
            return (rect, image)
        }
        let canvas = menus.reduce(frame) { $0.union($1.0) }
        let scale = window.backingScaleFactor
        guard let context = CGContext(
            data: nil, width: Int(canvas.width * scale), height: Int(canvas.height * scale), bitsPerComponent: 8,
            bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { return base }
        func place(_ image: CGImage, at rect: CGRect) {
            context.draw(image, in: CGRect(
                x: (rect.minX - canvas.minX) * scale, y: (canvas.maxY - rect.maxY) * scale,
                width: rect.width * scale, height: rect.height * scale))
        }
        place(base, at: frame)
        for (rect, image) in menus { place(image, at: rect) }
        return context.makeImage()
    }
}

