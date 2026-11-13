// TEMPORARY bug-hunt driver commands for the round 8 Mac hunt, never merged.
#if os(macOS)
import AppKit
import ApplicationServices
import Foundation
import KaeriKit
import KaeriStore
import KaeriUI
import WebKit

/// One accessibility element as the AX API reports it, read off the main thread.
struct AXLine: Sendable {
    var depth: Int
    var role: String
    var subrole: String
    var title: String
    var desc: String
    var value: String
    var help: String
    var enabled: Bool
    var focused: Bool
    var frame: CGRect
    var actions: [String]
}

enum AXWalk {
    static func string(_ el: AXUIElement, _ attr: String) -> String {
        var v: CFTypeRef?
        guard AXUIElementCopyAttributeValue(el, attr as CFString, &v) == .success, let v else { return "" }
        if let s = v as? String { return s }
        if let n = v as? NSNumber { return n.stringValue }
        return ""
    }

    static func bool(_ el: AXUIElement, _ attr: String) -> Bool {
        var v: CFTypeRef?
        guard AXUIElementCopyAttributeValue(el, attr as CFString, &v) == .success, let n = v as? NSNumber else { return false }
        return n.boolValue
    }

    static func frame(_ el: AXUIElement) -> CGRect {
        var p: CFTypeRef?, s: CFTypeRef?
        var point = CGPoint.zero, size = CGSize.zero
        if AXUIElementCopyAttributeValue(el, kAXPositionAttribute as CFString, &p) == .success, let p, CFGetTypeID(p) == AXValueGetTypeID() {
            AXValueGetValue(p as! AXValue, .cgPoint, &point)
        }
        if AXUIElementCopyAttributeValue(el, kAXSizeAttribute as CFString, &s) == .success, let s, CFGetTypeID(s) == AXValueGetTypeID() {
            AXValueGetValue(s as! AXValue, .cgSize, &size)
        }
        return CGRect(origin: point, size: size)
    }

    static func children(_ el: AXUIElement) -> [AXUIElement] {
        var v: CFTypeRef?
        guard AXUIElementCopyAttributeValue(el, kAXChildrenAttribute as CFString, &v) == .success, let arr = v as? [AXUIElement] else { return [] }
        return arr
    }

    static func line(_ el: AXUIElement, depth: Int) -> AXLine {
        var names: CFArray?
        _ = AXUIElementCopyActionNames(el, &names)
        return AXLine(depth: depth, role: string(el, kAXRoleAttribute), subrole: string(el, kAXSubroleAttribute),
                      title: string(el, kAXTitleAttribute), desc: string(el, kAXDescriptionAttribute),
                      value: String(string(el, kAXValueAttribute).prefix(80)), help: String(string(el, kAXHelpAttribute).prefix(80)),
                      enabled: bool(el, kAXEnabledAttribute), focused: bool(el, kAXFocusedAttribute), frame: frame(el),
                      actions: ((names as? [String]) ?? []).map { $0.replacingOccurrences(of: "AX", with: "") })
    }

    /// The app's own tree through the AX API, so SwiftUI builds its elements as it does for VoiceOver.
    static func walk(pid: pid_t, windowPrefix: String?, limit: Int) -> (AXError, [AXLine]) {
        let app = AXUIElementCreateApplication(pid)
        AXUIElementSetMessagingTimeout(app, 5)
        var probe: CFTypeRef?
        let err = AXUIElementCopyAttributeValue(app, kAXRoleAttribute as CFString, &probe)
        guard err == .success else { return (err, []) }
        var out: [AXLine] = []
        func go(_ el: AXUIElement, _ d: Int) {
            guard out.count < limit, d < 60 else { return }
            let l = line(el, depth: d)
            if d == 1, let windowPrefix, !l.title.hasPrefix(windowPrefix) { return }
            out.append(l)
            if l.role == "AXMenuBar" { return }
            for c in children(el) { go(c, d + 1) }
        }
        go(app, 0)
        return (.success, out)
    }

    static func focused(pid: pid_t) -> (AXError, AXLine?, [String]) {
        let app = AXUIElementCreateApplication(pid)
        AXUIElementSetMessagingTimeout(app, 5)
        var v: CFTypeRef?
        let err = AXUIElementCopyAttributeValue(app, kAXFocusedUIElementAttribute as CFString, &v)
        guard err == .success, let v, CFGetTypeID(v) == AXUIElementGetTypeID() else { return (err, nil, []) }
        let el = v as! AXUIElement
        // The path up to the window, for where the focus sits.
        var path: [String] = []
        var cur: AXUIElement? = el
        var n = 0
        while let c = cur, n < 12 {
            var p: CFTypeRef?
            guard AXUIElementCopyAttributeValue(c, kAXParentAttribute as CFString, &p) == .success, let p, CFGetTypeID(p) == AXUIElementGetTypeID() else { break }
            let parent = p as! AXUIElement
            let role = string(parent, kAXRoleAttribute)
            let name = string(parent, kAXTitleAttribute) + string(parent, kAXDescriptionAttribute)
            path.append("\(role)'\(name.prefix(30))'")
            if role == "AXWindow" { break }
            cur = parent; n += 1
        }
        return (.success, line(el, depth: 0), path)
    }

    static let actionable: Set<String> = ["AXButton", "AXCheckBox", "AXRadioButton", "AXPopUpButton", "AXMenuButton", "AXSlider",
                                          "AXTextField", "AXLink", "AXIncrementor", "AXComboBox", "AXDisclosureTriangle", "AXSegmentedControl"]

    static func text(_ l: AXLine) -> String {
        let f = l.frame
        return String(repeating: " ", count: min(l.depth, 30)) + "\(l.role)\(l.subrole.isEmpty ? "" : "/" + l.subrole) t='\(l.title)' d='\(l.desc)' v='\(l.value)'"
            + (l.help.isEmpty ? "" : " h='\(l.help)'") + (l.enabled ? "" : " OFF") + (l.focused ? " FOCUS" : "")
            + " \(Int(f.minX)),\(Int(f.minY)) \(Int(f.width))x\(Int(f.height))" + (l.actions.isEmpty ? "" : " [\(l.actions.joined(separator: ","))]")
    }
}

extension HuntDriver {
    /// Round 8 commands; false when the word is not one of them.
    static func r8(_ op: String, _ w: [String], store: AppStore, nav: Navigator) async -> Bool {
        let r = store.reader
        let pid = getpid()
        switch op {
        case "axapi":
            // axapi [limit] [window prefix|-] [all]: the app's AX tree through the AX API, with an audit of
            // actionable elements that have no name, and of images with no description
            let limit = ms(w, 0, 4000)
            let prefix = w.count > 1 && w[1] != "-" ? w[1...].filter { $0 != "all" }.joined(separator: " ") : nil
            let printAll = w.contains("all")
            let (err, lines) = await Task.detached { AXWalk.walk(pid: pid, windowPrefix: prefix, limit: limit) }.value
            guard err == .success else { say("axapi error \(err.rawValue) trusted=\(AXIsProcessTrusted())"); return true }
            var nameless = 0, images = 0
            var roles: [String: Int] = [:]
            for l in lines {
                roles[l.role, default: 0] += 1
                let named = !(l.title + l.desc).trimmingCharacters(in: .whitespaces).isEmpty
                if AXWalk.actionable.contains(l.role), !named, l.role != "AXTextField" || l.value.isEmpty {
                    nameless += 1; say("AUDIT nameless " + AXWalk.text(l))
                } else if l.role == "AXImage", !named, l.frame.width > 8 {
                    images += 1; say("AUDIT image " + AXWalk.text(l))
                } else if printAll { say(AXWalk.text(l)) }
            }
            say("axapi \(lines.count) elements, nameless actionable \(nameless), unnamed images \(images); roles \(roles.sorted { $0.value > $1.value }.prefix(14).map { "\($0.key.dropFirst(2))=\($0.value)" }.joined(separator: " "))")
        case "axq":
            // axq <text>: AX API elements whose title, description or value holds the text
            let needle = w.joined(separator: " ").lowercased()
            let (err, lines) = await Task.detached { AXWalk.walk(pid: pid, windowPrefix: nil, limit: 6000) }.value
            guard err == .success else { say("axq error \(err.rawValue)"); return true }
            var n = 0
            for l in lines where (l.title + "|" + l.desc + "|" + l.value).lowercased().contains(needle) { n += 1; if n <= 40 { say(AXWalk.text(l)) } }
            say("axq \(n) for '\(needle)'")
        case "axfocus":
            let (err, l, path) = await Task.detached { AXWalk.focused(pid: pid) }.value
            say("axfocus err=\(err.rawValue) " + (l.map(AXWalk.text) ?? "-") + " in " + path.reversed().joined(separator: ">")
                + " fr=\(NSApp.keyWindow?.firstResponder.map { String(describing: type(of: $0)) } ?? "nil") key='\(NSApp.keyWindow?.title ?? "-")'")
        case "tabs":
            // tabs <n> [shift] [gap ms]: Tab through the key window, with the focus after each
            let n = ms(w, 0, 10)
            let shift = w.contains("shift")
            let gap = w.count > 2 ? Int(w[2]) ?? 350 : (w.count > 1 && !shift ? Int(w[1]) ?? 350 : 350)
            for i in 0..<n {
                if let win = NSApp.keyWindow ?? mainWindow() {
                    for type in [NSEvent.EventType.keyDown, .keyUp] {
                        if let e = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: shift ? [.shift] : [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                    windowNumber: win.windowNumber, context: nil, characters: shift ? "\u{19}" : "\t",
                                                    charactersIgnoringModifiers: "\t", isARepeat: false, keyCode: 48) { NSApp.postEvent(e, atStart: false) }
                    }
                }
                try? await Task.sleep(for: .milliseconds(gap))
                let (_, l, path) = await Task.detached { AXWalk.focused(pid: pid) }.value
                say("tab \(i + 1): " + (l.map(AXWalk.text) ?? "-") + " in " + path.reversed().suffix(3).joined(separator: ">")
                    + " fr=\(NSApp.keyWindow?.firstResponder.map { String(describing: type(of: $0)) } ?? "nil")")
            }
        case "axpress":
            // axpress <text…> [#n]: AXPress on the nth AX API element whose title or description holds the text
            var words = w; var pick = 0
            if let last = words.last, last.hasPrefix("#"), let k = Int(last.dropFirst()) { pick = k; words.removeLast() }
            let needle = words.joined(separator: " ").lowercased()
            let result: String = await Task.detached {
                let app = AXUIElementCreateApplication(pid)
                AXUIElementSetMessagingTimeout(app, 5)
                var hits: [AXUIElement] = []
                func go(_ el: AXUIElement, _ d: Int) {
                    guard d < 60, hits.count <= pick else { return }
                    let t = (AXWalk.string(el, kAXTitleAttribute) + "|" + AXWalk.string(el, kAXDescriptionAttribute)).lowercased()
                    if t.contains(needle) { hits.append(el) }
                    if AXWalk.string(el, kAXRoleAttribute) == "AXMenuBar" { return }
                    for c in AXWalk.children(el) { go(c, d + 1) }
                }
                go(app, 0)
                guard hits.indices.contains(pick) else { return "axpress: no match #\(pick) for '\(needle)'" }
                let el = hits[pick]
                let e = AXUIElementPerformAction(el, kAXPressAction as CFString)
                return "axpress #\(pick) " + AXWalk.text(AXWalk.line(el, depth: 0)) + " -> \(e.rawValue)"
            }.value
            say(result)
            try? await Task.sleep(for: .milliseconds(1200)); say(state(r, nav))
        case "targets":
            store.reading.sending.loadTargets()
            try? await Task.sleep(for: .milliseconds(1500))
            say("targets " + store.reading.sending.targets.map { "\($0.id)" }.joined(separator: ",") + " default=\(store.reading.sending.defaultTarget?.id ?? "-")")
            for t in store.reading.sending.targets { say(" target \(String(describing: t).prefix(400))") }
        case "send":
            // send <work> <ch,ch> <target id> [novel]
            guard w.count >= 3, let t = store.reading.sending.targets.first(where: { $0.id == w[2] }) else { say("send? \(w)"); return true }
            let id = store.reading.sending.begin(work: w[0], chapters: w[1].split(separator: ",").map(String.init), target: t, novel: w.contains("novel"))
            say("send begun id=\(id.map(String.init) ?? "nil")")
        case "sends":
            for s in store.reading.sending.sends { say(" send \(String(describing: s).prefix(600))") }
            say("sends \(store.reading.sending.sends.count)")
        case "sendcancel": store.reading.sending.cancel(ms(w, 0, 1)); say("cancel sent")
        case "sendagain": say("again id=\(store.reading.sending.sendAgain(ms(w, 0, 1)).map(String.init) ?? "nil")")
        case "sharefiles": say("shareable \(store.reading.sending.shareable(ms(w, 0, 1)).map { $0.map(\.path) } ?? [])")
        case "feed":
            // feed load | on <password> | off | share <shelf> on|off
            switch w.first {
            case "on": store.reading.sending.setFeed(on: true, password: w.count > 1 ? w[1] : nil)
            case "off": store.reading.sending.setFeed(on: false)
            default: store.reading.sending.loadFeed()
            }
            try? await Task.sleep(for: .milliseconds(2000))
            say("feed \(String(describing: store.reading.sending.feed).prefix(1500))")
        case "nov":
            var found: [WKWebView] = []
            func find(_ v: NSView) { if let web = v as? WKWebView { found.append(web) }; v.subviews.forEach(find) }
            if let v = mainWindow()?.contentView { find(v) }
            for web in found {
                let js = "JSON.stringify({w: document.documentElement.scrollWidth, sx: window.scrollX, sy: window.scrollY, h: document.documentElement.scrollHeight, iw: innerWidth, ih: innerHeight, font: getComputedStyle(document.body).fontFamily, bg: getComputedStyle(document.body).backgroundColor, fg: getComputedStyle(document.body).color, fs: getComputedStyle(document.body).fontSize, text: (document.body.innerText||'').length})"
                let out = (try? await web.evaluateJavaScript(js)) as? String ?? "?"
                say("nov url=\(web.url?.lastPathComponent ?? "nil") hidden=\(web.isHidden) loading=\(web.isLoading) \(out)")
            }
            say("nov webs=\(found.count) " + state(r, nav))
        case "env":
            say("env FKA=\(UserDefaults.standard.integer(forKey: "AppleKeyboardUIMode")) kbnav=\(NSApp.isFullKeyboardAccessEnabled) trusted=\(AXIsProcessTrusted()) reduceMotion=\(NSWorkspace.shared.accessibilityDisplayShouldReduceMotion)")
        default:
            return false
        }
        return true
    }
}
#endif
