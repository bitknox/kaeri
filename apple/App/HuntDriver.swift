// TEMPORARY bug-hunt driver, never committed. Reads commands from $KAERI_HUNT_DIR/cmd and
// appends what it saw to $KAERI_HUNT_DIR/log, then writes $KAERI_HUNT_DIR/ack.
import Darwin
import Foundation
import KaeriKit
import KaeriStore
import KaeriUI
#if os(macOS)
import AppKit
import ObjectiveC
import WebKit
#else
import UIKit
#endif

@MainActor
enum HuntDriver {
    static var dir: URL?

    static func footprint() -> Double {
        var info = task_vm_info_data_t()
        var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<natural_t>.size)
        let kr = withUnsafeMutablePointer(to: &info) {
            $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
                task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count)
            }
        }
        return kr == KERN_SUCCESS ? Double(info.phys_footprint) / 1_048_576 : -1
    }

    static func threads() -> Int {
        var list: thread_act_array_t?
        var count: mach_msg_type_number_t = 0
        guard task_threads(mach_task_self_, &list, &count) == KERN_SUCCESS, let list else { return -1 }
        for i in 0..<Int(count) { mach_port_deallocate(mach_task_self_, list[i]) }
        vm_deallocate(mach_task_self_, vm_address_t(UInt(bitPattern: list)), vm_size_t(Int(count) * MemoryLayout<thread_t>.stride))
        return Int(count)
    }

    static func fds() -> Int {
        var n = 0
        for fd in 0..<4096 where fcntl(Int32(fd), F_GETFD) != -1 { n += 1 }
        return n
    }

    static func cpu() -> Double {
        var usage = rusage()
        getrusage(RUSAGE_SELF, &usage)
        return Double(usage.ru_utime.tv_sec) + Double(usage.ru_utime.tv_usec) / 1e6
            + Double(usage.ru_stime.tv_sec) + Double(usage.ru_stime.tv_usec) / 1e6
    }

    static func say(_ line: String) {
        guard let dir else { return }
        let stamp = String(format: "%.3f", Date().timeIntervalSince1970)
        let text = "\(stamp) \(line)\n"
        let log = dir.appending(path: "log")
        if let h = try? FileHandle(forWritingTo: log) {
            h.seekToEndOfFile(); h.write(text.data(using: .utf8)!); try? h.close()
        } else {
            try? text.write(to: log, atomically: true, encoding: .utf8)
        }
    }

    static func stats() -> String {
        String(format: "mem=%.1f threads=%d fds=%d cpu=%.2f", footprint(), threads(), fds(), cpu())
    }

    static func state(_ r: ReaderModel, _ nav: Navigator) -> String {
        var s = "phase=\(r.phase) req=\(r.request.map { "\($0.work)/\($0.chapter ?? "-")" } ?? "nil")"
        if let v = r.view {
            s += " ch=\(v.chapter.id) label='\(v.chapter.label)' mode=\(v.mode) dir=\(v.direction) next=\(v.next?.id ?? "-") prev=\(v.previous?.id ?? "-")"
        }
        let leafPages = r.leaves.indices.contains(r.current) ? r.leaves[r.current].parts.map { String($0.page) }.joined(separator: "+") : "-"
        s += " pos=\(r.position.index)/\(r.position.count) cur=\(r.current) leafpg=\(leafPages) leaves=\(r.leaves.count) pages=\(r.pages.count) fails=\(r.failures.count)"
        s += " end=\(r.end.map { "next:\($0.next?.id ?? "-") why:\(String(describing: $0.why))" } ?? "nil") flowing=\(r.flowing ?? "-") trail=\(r.trail.chapters.count)"
        s += " sheet=\(String(describing: r.sheet)) chrome=\(r.chromeShown) notice='\(r.notice?.text ?? "")' backTo=\(r.backToPage.map(String.init) ?? "-") novel=\(r.novel != nil) read=\(r.read)"
        s += " nav=\(nav.section.rawValue) depth=\(nav.paths[nav.section]?.count ?? 0) canBack=\(nav.canGoBack)"
        return s + " " + stats()
    }

    static var phases: SystemPhases?

    static func run(store: AppStore, navigator: Navigator, phases: SystemPhases? = nil) async {
        guard let path = ProcessInfo.processInfo.environment["KAERI_HUNT_DIR"] else { return }
        let dir = URL(filePath: path)
        Self.dir = dir
        Self.phases = phases
        #if os(macOS)
        try? await Task.sleep(for: .seconds(1))
        if ProcessInfo.processInfo.environment["KAERI_HUNT_LOOKKEY"] == "1" {
            if let win = mainWindow() { pinned = win; lookKey(win) }
        } else {
            NSApp.activate(ignoringOtherApps: true)
            mainWindow()?.makeKeyAndOrderFront(nil)
        }
        #endif
        say("driver up \(stats())")
        let cmd = dir.appending(path: "cmd")
        while true {
            if let text = try? String(contentsOf: cmd, encoding: .utf8) {
                try? FileManager.default.removeItem(at: cmd)
                for line in text.split(separator: "\n") {
                    let words = line.split(separator: " ").map(String.init)
                    guard let first = words.first else { continue }
                    if first == "#" { continue }
                    say("> \(line)")
                    await perform(first, Array(words.dropFirst()), store: store, nav: navigator)
                }
                try? "done".write(to: dir.appending(path: "ack"), atomically: true, encoding: .utf8)
            }
            try? await Task.sleep(for: .milliseconds(150))
        }
    }

    static func waitOpen(_ r: ReaderModel, seconds: Double = 30) async -> Double {
        let start = ContinuousClock.now
        while ContinuousClock.now - start < .seconds(seconds) {
            if r.phase == .open, r.novel != nil || !r.pages.isEmpty { break }
            if case .failed = r.phase { break }
            try? await Task.sleep(for: .milliseconds(50))
        }
        let d = ContinuousClock.now - start
        return Double(d.components.seconds) + Double(d.components.attoseconds) / 1e18
    }

    static func si(_ v: CGFloat) -> Int { v.isFinite ? Int(v) : -1 }

    static func ms(_ words: [String], _ i: Int, _ d: Int) -> Int { words.count > i ? Int(words[i]) ?? d : d }

    static func perform(_ op: String, _ w: [String], store: AppStore, nav: Navigator) async {
        let r = store.reader
        switch op {
        case "open":
            let at: OpenAt = w.count > 2 ? (w[2] == "start" ? .start : .resume) : .resume
            r.open(work: w[0], chapter: w.count > 1 && w[1] != "-" ? w[1] : nil, at: at)
            let t = await waitOpen(r)
            say(String(format: "opened in %.2fs ", t) + state(r, nav))
        case "openq":
            r.open(work: w[0], chapter: w.count > 1 && w[1] != "-" ? w[1] : nil, at: .resume)
        case "close": r.close()
        case "state": say(state(r, nav))
        case "mem": say(stats())
        case "sleep": try? await Task.sleep(for: .milliseconds(ms(w, 0, 1000)))
        case "turn", "line":
            let n = ms(w, 1, 1), gap = ms(w, 2, 400)
            for _ in 0..<n {
                r.requestTurn(forward: w.first != "b", line: op == "line")
                try? await Task.sleep(for: .milliseconds(gap))
            }
            say(state(r, nav))
        case "jump": r.requestJump(ms(w, 0, 0)); try? await Task.sleep(for: .milliseconds(600)); say(state(r, nav))
        case "chapter": r.openChapter(w[0], atEnd: w.count > 1); let t = await waitOpen(r); say(String(format: "chapter in %.2fs ", t) + state(r, nav))
        case "flow": r.flowOn(); say(state(r, nav))
        case "mode":
            let m: ReaderModel.ReadingModeChoice = switch w.first { case "rtl": .rightToLeft; case "ltr": .leftToRight; case "strip": .strip; default: .auto }
            r.chooseMode(m); try? await Task.sleep(for: .milliseconds(800)); say(state(r, nav))
        case "series": nav.openSeries(w[0]); try? await Task.sleep(for: .milliseconds(800)); say(state(r, nav))
        case "loadch": store.reading.loadChapters(w[0])
        case "navback": nav.back(); try? await Task.sleep(for: .milliseconds(600)); say(state(r, nav))
        case "settings":
            #if os(macOS)
            nav.openSettings(.reading)
            #endif
            try? await Task.sleep(for: .milliseconds(800)); say(state(r, nav))
        case "chapters":
            // chapters <work> [max rows] [waitsecs]: the series page's chapter list as the engine gives it
            let wait = ms(w, 2, 0)
            let start = ContinuousClock.now
            while store.reading.chapters[w[0]] == nil, ContinuousClock.now - start < .seconds(wait) {
                try? await Task.sleep(for: .milliseconds(200))
            }
            dumpChapters(store, w[0], max: ms(w, 1, 60))
        case "seriesv":
            if let s = store.reading.series[w[0]] {
                say("series \(s.work) medium=\(s.medium) title='\(s.titles.display)' action=\(s.action) chTotal=\(s.chaptersTotal.map(String.init) ?? "-") volTotal=\(s.volumesTotal.map(String.init) ?? "-") synopsis=\(s.synopsis?.count ?? 0)ch edition=\(s.edition.map { "\($0.sourceName)/\($0.chapters)/latest=\($0.latest ?? "-")" } ?? "nil")")
            } else { say("series \(w[0]) not loaded") }
            if let t = store.reading.tracking[w[0]] { say("tracking \(t.map { "\($0)" }.joined(separator: " | ").prefix(800))") }
        case "editions":
            store.bookSources.loadForWork(w[0])
            try? await Task.sleep(for: .milliseconds(ms(w, 1, 1500)))
            if let e = store.bookSources.forWork[w[0]] {
                say("editions asking=\(e.asking) missing=\(e.missing.count)")
                for x in e.editions { say("  ed \(x.sourceId) '\(x.sourceName)' lang=\(x.language ?? "-") ch=\(x.chapters) latest=\(x.latest ?? "-") pref=\(x.preferred) inUse=\(x.inUse) ver=\(x.versionLabel ?? "-")") }
            } else { say("editions none") }
        case "dl":
            // dl <work> next <n> | dl <work> ids <id,id>
            let pick: DownloadPick = w[1] == "ids" ? .chapters(ids: w[2].split(separator: ",").map(String.init)) : .nextUnread(n: UInt32(ms(w, 2, 1)))
            store.reading.download(w[0], pick); say("dl sent")
        case "dlcancel": store.reading.cancelDownloads(w[0], chapters: w[1].split(separator: ",").map(String.init)); say("cancel sent")
        case "dldelete": store.reading.deleteDownloads(w[0], chapters: w[1].split(separator: ",").map(String.init)); say("delete sent")
        case "dlwatch":
            // dlwatch <work> <secs> <everyms>: the download states of the rows that have one, as they change
            guard !w.isEmpty else { say("dlwatch needs a work"); break }
            let secs = ms(w, 1, 30), every = ms(w, 2, 500)
            var last = ""
            let start = ContinuousClock.now
            while ContinuousClock.now - start < .seconds(secs) {
                let rows = (store.reading.chapters[w[0]]?.volumes ?? []).flatMap(\.chapters).filter { $0.download != nil }
                let now = rows.map { "\($0.number ?? $0.label):\(dlText($0.download))" }.joined(separator: " ")
                if now != last {
                    let d = ContinuousClock.now - start
                    say(String(format: "dl[%.1fs] ", Double(d.components.seconds) + Double(d.components.attoseconds) / 1e18) + now)
                    last = now
                }
                try? await Task.sleep(for: .milliseconds(every))
            }
        case "offline":
            phases?.network(expensive: false, constrained: false, offline: w.first == "on"); try? await Task.sleep(for: .milliseconds(800)); say("offline \(w.first ?? "?") " + state(r, nav))
        case "switch":
            // switch showVolumes on|off
            let key: SettingKey? = switch w[0] { case "showVolumes": .showVolumes; default: nil }
            if let key { store.setSwitch(key, on: w[1] == "on"); try? await Task.sleep(for: .milliseconds(800)); say("switch \(w[0])=\(store.settings.isOn(key))") }
        case "srcoff":
            // srcoff <source id> on|off: switch a reading source off or back on
            store.bookSources.setOff(w[0], off: w[1] == "on"); try? await Task.sleep(for: .milliseconds(1000)); say("srcoff \(w[0]) \(w[1])")
        case "home":
            store.reading.loadHome(refresh: w.first == "refresh"); try? await Task.sleep(for: .milliseconds(ms(w, 1, 3000)))
            say("readhome \(String(describing: store.reading.home).prefix(3000))")
        case "sources":
            store.bookSources.load(); try? await Task.sleep(for: .milliseconds(1500))
            say("book sources \(String(describing: store.bookSources.view).prefix(2000))")
        case "section":
            if let s = AppSection(rawValue: w[0]) { nav.open(s) }
            try? await Task.sleep(for: .milliseconds(500)); say(state(r, nav))
        case "sheet":
            r.sheet = switch w.first { case "settings": .settings; case "chapters": .chapters; case "more": .more; default: nil }
            try? await Task.sleep(for: .milliseconds(500)); say(state(r, nav))
        case "chrome": r.chromeShown = w.first == "on"
        case "crop": r.set(.cropBorders, .toggle(on: w.first == "on")); try? await Task.sleep(for: .milliseconds(1500)); say(state(r, nav))
        case "nmode": r.set(.mode, .choice(name: w.first ?? "paged")); try? await Task.sleep(for: .milliseconds(1500)); say(state(r, nav))
        case "layout":r.set(.pageLayout, .choice(name: w.first ?? "auto")); try? await Task.sleep(for: .milliseconds(1500)); say(state(r, nav))
        case "loopopen":
            // loopopen <work> <chapter> <n> <holdms>
            let n = ms(w, 2, 10), hold = ms(w, 3, 1500)
            say("loop start " + stats())
            for i in 1...n {
                r.open(work: w[0], chapter: w[1], at: .start)
                let t = await waitOpen(r, seconds: 20)
                try? await Task.sleep(for: .milliseconds(hold))
                let ok = r.phase == .open
                r.close()
                try? await Task.sleep(for: .milliseconds(400))
                say(String(format: "loop %d open=%@ t=%.2f ", i, ok ? "y" : "N", t) + stats())
            }
            try? await Task.sleep(for: .seconds(5))
            say("loop end +5s " + stats())
        case "loopfast":
            // loopfast <work> <chapter> <n>: open and leave at once
            let n = ms(w, 2, 20)
            for i in 1...n {
                r.open(work: w[0], chapter: w[1], at: .resume)
                try? await Task.sleep(for: .milliseconds(ms(w, 3, 30)))
                r.close()
                try? await Task.sleep(for: .milliseconds(ms(w, 4, 30)))
                if i % 10 == 0 { say("fast \(i) " + stats()) }
            }
            try? await Task.sleep(for: .seconds(3))
            say("fast end " + state(r, nav))
        case "cross":
            // cross <n> <gapms>: turn forward until the chapter changes, n times
            let n = ms(w, 0, 5), gap = ms(w, 1, 250)
            for i in 1...n {
                let from = r.view?.chapter.id ?? "?"
                var turns = 0
                let start = ContinuousClock.now
                while (r.view?.chapter.id ?? from) == from || r.phase != .open, turns < 400 {
                    r.requestTurn(forward: true)
                    turns += 1
                    try? await Task.sleep(for: .milliseconds(gap))
                }
                let d = ContinuousClock.now - start
                say("cross \(i) \(from)->\(r.view?.chapter.id ?? "?") turns=\(turns) secs=\(d.components.seconds) " + state(r, nav))
            }
        case "crossjump":
            // crossjump <n> <gapms>: jump to the last leaf, then turn on, n times
            let n = ms(w, 0, 5), gap = ms(w, 1, 400)
            for i in 1...n {
                let from = r.view?.chapter.id ?? "?"
                r.requestJump(max(r.position.count - 1, 0))
                try? await Task.sleep(for: .milliseconds(gap))
                var turns = 0
                while (r.view?.chapter.id ?? from) == from || r.phase != .open, turns < 60 {
                    r.requestTurn(forward: true)
                    turns += 1
                    try? await Task.sleep(for: .milliseconds(gap))
                }
                say("xj \(i) \(from)->\(r.view?.chapter.id ?? "?") turns=\(turns) " + state(r, nav))
            }
        #if os(macOS)
        case "keyk": key(w, target: NSApp.keyWindow)
            try? await Task.sleep(for: .milliseconds(400)); say(state(r, nav) + " fields=\(fields())")
        case "ask": r.askingPage = w.first != "off"; try? await Task.sleep(for: .milliseconds(800)); say("key=\(String(describing: NSApp.keyWindow)) fr=\(String(describing: NSApp.keyWindow?.firstResponder)) fields=\(fields())")
        case "key": key(w)
            try? await Task.sleep(for: .milliseconds(400)); say(state(r, nav))
        case "menu": menu(w.joined(separator: " ")); try? await Task.sleep(for: .milliseconds(400)); say(state(r, nav))
        case "menus": dumpMenus()
        case "click":
            // click <x> <y> [n]: a synthetic left click at a point from the window's top left, in-process only.
            if let win = mainWindow() {
                let x = Double(w[0]) ?? 0, y = win.frame.height - (Double(w[1]) ?? 0)
                let n = w.count > 2 ? Int(w[2]) ?? 1 : 1
                for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
                    if let e = NSEvent.mouseEvent(with: type, location: CGPoint(x: x, y: y), modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                  windowNumber: win.windowNumber, context: nil, eventNumber: 0, clickCount: n, pressure: 1) {
                        NSApp.sendEvent(e)
                    }
                }
            }
            try? await Task.sleep(for: .milliseconds(700)); say(state(r, nav)); await window(["list"])
        case "tl":
            // The traffic lights: alpha, hidden, and what a click at each one's centre would hit.
            if let win = mainWindow() {
                for kind in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
                    guard let b = win.standardWindowButton(kind), let sup = b.superview else { continue }
                    let centre = sup.convert(CGPoint(x: b.frame.midX, y: b.frame.midY), to: nil)
                    let root = win.contentView?.superview
                    let hit = root?.hitTest(root?.convert(centre, from: nil) ?? .zero)
                    say("tl \(kind.rawValue) alpha=\(b.alphaValue) hidden=\(b.isHidden) enabled=\(b.isEnabled) at=\(centre) hit=\(hit.map { String(describing: type(of: $0)) } ?? "nil") isButton=\(hit === b)")
                }
            }
        case "win": await window(w)
        case "ax": if let win = mainWindow() { dumpAX(win, depth: 0, max: ms(w, 0, 40)) }
        case "first": say("firstResponder=\(String(describing: mainWindow()?.firstResponder)) key=\(String(describing: NSApp.keyWindow?.title)) active=\(NSApp.isActive)")
        #else
        case "ax": if let win = keyWindowIOS() { dumpAXIOS(win, depth: 0, max: ms(w, 0, 40)) }
        case "suspendin":
            // suspendin <ms>: send the app to the background after ms (cuts off a gesture in flight)
            let wait = ms(w, 0, 1200)
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(wait))
                say("suspending now")
                UIControl().sendAction(#selector(URLSessionTask.suspend), to: UIApplication.shared, for: nil)
            }
        case "scene":
            for case let scene as UIWindowScene in UIApplication.shared.connectedScenes {
                say("scene \(scene.activationState.rawValue) bounds=\(scene.coordinateSpace.bounds) h=\(scene.traitCollection.horizontalSizeClass.rawValue) v=\(scene.traitCollection.verticalSizeClass.rawValue) orient=\(scene.effectiveGeometry.interfaceOrientation.rawValue) windows=\(scene.windows.count) keyfr=\(String(describing: scene.keyWindow.map { type(of: $0) }))")
            }
        case "narrow":
            // narrow <maxwidth>: cap the scene's width (iPad windowed), 0 to lift it
            if let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene {
                let width = ms(w, 0, 0)
                scene.sizeRestrictions?.maximumSize = width > 0 ? CGSize(width: width, height: 5000) : CGSize(width: 5000, height: 5000)
                scene.sizeRestrictions?.minimumSize = width > 0 ? CGSize(width: min(width, 320), height: 300) : CGSize(width: 320, height: 300)
                try? await Task.sleep(for: .seconds(1))
                say("scene size \(scene.coordinateSpace.bounds) " + state(r, nav))
            }
        #endif
        #if os(macOS)
        case "shot": await shot(w.first ?? "shot")
        case "rshot": await rshot(w.first ?? "rshot")
        case "activate": NSApp.activate(ignoringOtherApps: true); mainWindow()?.makeKeyAndOrderFront(nil); try? await Task.sleep(for: .milliseconds(500)); say("active=\(NSApp.isActive) key=\(String(describing: NSApp.keyWindow?.title)) fr=\(String(describing: NSApp.keyWindow?.firstResponder))")
        case "wheel":
            // wheel <dy points> <n> <gapms> [x y from top left]: continuous scroll-wheel events at a point, through AppKit's dispatch
            if let win = mainWindow() {
                let dy = Int32(ms(w, 0, -60)), n = ms(w, 1, 10), gap = ms(w, 2, 16)
                let x = w.count > 3 ? Double(w[3]) ?? win.frame.width / 2 : win.frame.width / 2
                let yTop = w.count > 4 ? Double(w[4]) ?? win.frame.height / 2 : win.frame.height / 2
                let screenH = NSScreen.screens.first?.frame.height ?? 0
                let gpt = CGPoint(x: win.frame.minX + x, y: screenH - (win.frame.maxY - yTop))
                for i in 0..<n {
                    guard let cg = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1, wheel1: dy, wheel2: 0, wheel3: 0) else { break }
                    cg.location = gpt
                    cg.setIntegerValueField(.scrollWheelEventIsContinuous, value: 1)
                    cg.setIntegerValueField(.scrollWheelEventScrollPhase, value: Int64(i == 0 ? 1 : (i == n - 1 ? 4 : 2)))
                    if let e = NSEvent(cgEvent: cg) { NSApp.sendEvent(e) }
                    try? await Task.sleep(for: .milliseconds(gap))
                }
            }
            try? await Task.sleep(for: .milliseconds(500)); say(state(r, nav))
        case "move":
            // move <x> <y>: a mouse-moved event at a point from the window's top left (hover)
            if let win = mainWindow() {
                let p = CGPoint(x: Double(w[0]) ?? 0, y: win.frame.height - (Double(w[1]) ?? 0))
                if let e = NSEvent.mouseEvent(with: .mouseMoved, location: p, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                              windowNumber: win.windowNumber, context: nil, eventNumber: 0, clickCount: 0, pressure: 0) { NSApp.sendEvent(e) }
            }
            try? await Task.sleep(for: .milliseconds(400)); say(state(r, nav))
        case "pid": say("pid=\(getpid())")
        case "tb":
            if let win = mainWindow() {
                say("tb transparent=\(win.titlebarAppearsTransparent) titleVis=\(win.titleVisibility.rawValue) toolbar=\(win.toolbar.map { "visible=\($0.isVisible) items=\($0.items.count)" } ?? "nil") fullSize=\(win.styleMask.contains(.fullSizeContentView)) full=\(win.styleMask.contains(.fullScreen)) sep=\(win.titlebarSeparatorStyle.rawValue)")
            }
        case "quit": NSApp.terminate(nil)
        case "scroll":
            // scroll <y> [index]: the window's scroll views, biggest first; scroll the one picked to y
            if let win = mainWindow(), let root = win.contentView {
                var found: [NSScrollView] = []
                func walk(_ v: NSView) { if let s = v as? NSScrollView, !s.isHidden { found.append(s) }; v.subviews.forEach(walk) }
                walk(root)
                found.sort { $0.frame.width * $0.frame.height > $1.frame.width * $1.frame.height }
                for (i, s) in found.prefix(6).enumerated() { say("scrollview \(i) \(type(of: s)) frame=\(s.convert(s.bounds, to: nil)) doc=\(s.documentView?.frame.size ?? .zero) at=\(s.contentView.bounds.origin)") }
                let i = ms(w, 1, 0)
                if found.indices.contains(i) {
                    let s = found[i]
                    s.contentView.scroll(to: CGPoint(x: 0, y: Double(w[0]) ?? 0))
                    s.reflectScrolledClipView(s.contentView)
                }
            }
            try? await Task.sleep(for: .milliseconds(600)); say(state(r, nav))
        case "cgwheel":
            // cgwheel <dy> <dx> <n> <gapms> [x y from top left] [trackpad|mouse|post]: wheel events made as the window server makes them
            if let win = mainWindow() {
                let dy = Int32(ms(w, 0, -40)), dx = Int32(ms(w, 1, 0)), n = ms(w, 2, 10), gap = ms(w, 3, 16)
                let x = w.count > 4 ? Double(w[4]) ?? win.frame.width / 2 : win.frame.width / 2
                let yTop = w.count > 5 ? Double(w[5]) ?? win.frame.height / 2 : win.frame.height / 2
                let how = w.count > 6 ? w[6] : "trackpad"
                let screenH = NSScreen.screens.first?.frame.height ?? 0
                let gpt = CGPoint(x: win.frame.minX + x, y: screenH - (win.frame.maxY - yTop))
                var types: [String] = []
                for i in 0..<n {
                    let units: CGScrollEventUnit = how == "mouse" ? .line : .pixel
                    guard let cg = CGEvent(scrollWheelEvent2Source: nil, units: units, wheelCount: 2, wheel1: dy, wheel2: dx, wheel3: 0) else { break }
                    cg.location = gpt
                    if how != "mouse" {
                        cg.setIntegerValueField(.scrollWheelEventIsContinuous, value: 1)
                        cg.setIntegerValueField(.scrollWheelEventScrollPhase, value: Int64(i == 0 ? 1 : (i == n - 1 ? 4 : 2)))
                    }
                    cg.setIntegerValueField(CGEventField(rawValue: 91)!, value: Int64(win.windowNumber))
                    cg.setIntegerValueField(CGEventField(rawValue: 92)!, value: Int64(win.windowNumber))
                    if how == "post" { cg.postToPid(getpid()) }
                    else if let e = NSEvent(cgEvent: cg) { if i == 0 { types.append("\(e.type.rawValue)/win=\(e.windowNumber)/loc=\(e.locationInWindow)") }; NSApp.sendEvent(e) }
                    try? await Task.sleep(for: .milliseconds(gap))
                }
                say("cgwheel \(how) \(types)")
            }
            try? await Task.sleep(for: .milliseconds(500)); say(state(r, nav))
        case "gesture":
            // gesture <magnify|swipe|rotate> <value> <n> <gapms> [x y]: trackpad gestures through the private gesture fields
            if let win = mainWindow() {
                let kind = w.first ?? "magnify", value = Double(w.count > 1 ? w[1] : "0.05") ?? 0.05, n = ms(w, 2, 8), gap = ms(w, 3, 16)
                let x = w.count > 4 ? Double(w[4]) ?? win.frame.width / 2 : win.frame.width / 2
                let yTop = w.count > 5 ? Double(w[5]) ?? win.frame.height / 2 : win.frame.height / 2
                let screenH = NSScreen.screens.first?.frame.height ?? 0
                let gpt = CGPoint(x: win.frame.minX + x, y: screenH - (win.frame.maxY - yTop))
                var seen: [String] = []
                for i in 0..<n {
                    guard let cg = CGEvent(source: nil) else { break }
                    cg.type = CGEventType(rawValue: 29)!
                    cg.location = gpt
                    let hid: Int64 = kind == "magnify" ? 8 : (kind == "rotate" ? 5 : 16)
                    cg.setIntegerValueField(CGEventField(rawValue: 110)!, value: hid)
                    if kind == "magnify" { cg.setDoubleValueField(CGEventField(rawValue: 113)!, value: value) }
                    if kind == "rotate" { cg.setDoubleValueField(CGEventField(rawValue: 114)!, value: value) }
                    if kind == "swipe" { cg.setDoubleValueField(CGEventField(rawValue: 124)!, value: value); cg.setIntegerValueField(CGEventField(rawValue: 115)!, value: value < 0 ? 4 : 8) }
                    cg.setIntegerValueField(CGEventField(rawValue: 132)!, value: Int64(i == 0 ? 1 : (i == n - 1 ? 4 : 2)))
                    cg.setIntegerValueField(CGEventField(rawValue: 91)!, value: Int64(win.windowNumber))
                    cg.setIntegerValueField(CGEventField(rawValue: 92)!, value: Int64(win.windowNumber))
                    if let e = NSEvent(cgEvent: cg) {
                        if i < 2 { seen.append("type=\(e.type.rawValue) mag=\(e.type == .magnify ? e.magnification : -99)") }
                        NSApp.sendEvent(e)
                    }
                    try? await Task.sleep(for: .milliseconds(gap))
                }
                say("gesture \(kind) \(seen)")
            }
            try? await Task.sleep(for: .milliseconds(500)); say(state(r, nav))
        case "pkey":
            // pkey <name> [mods]: a key made as the window server makes it, posted to this process
            if let name = w.first, let (code, _) = codes[name] {
                var flags: CGEventFlags = []
                for m in w.dropFirst() { switch m { case "cmd": flags.insert(.maskCommand); case "shift": flags.insert(.maskShift); case "opt": flags.insert(.maskAlternate); case "ctrl": flags.insert(.maskControl); default: break } }
                for down in [true, false] {
                    if let cg = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down) { cg.flags = flags; cg.postToPid(getpid()) }
                }
            }
            try? await Task.sleep(for: .milliseconds(500)); say(state(r, nav))
        case "axfind", "press":
            // axfind <text…>: elements whose label, title or value holds the text; press <text…> [#n]: press the nth match
            if let win = mainWindow() {
                var words = w; var pick = 0
                if let last = words.last, last.hasPrefix("#"), let k = Int(last.dropFirst()) { pick = k; words.removeLast() }
                let needle = words.joined(separator: " ").lowercased()
                var hits: [AnyObject] = []
                func walk(_ o: AnyObject, _ d: Int) {
                    guard d < 60 else { return }
                    let label: String = axLabel(o) + "|" + axTitle(o) + "|" + axValue(o)
                    if label.lowercased().contains(needle) { hits.append(o) }
                    for c in (o.accessibilityChildren?() ?? nil) ?? [] { walk(c as AnyObject, d + 1) }
                }
                walk(win, 0)
                for (i, o) in hits.prefix(12).enumerated() {
                    let role: String = (o.accessibilityRole?() ?? nil)?.rawValue ?? "?"
                    let f: CGRect = o.accessibilityFrame?() ?? .zero
                    let en: Bool = o.isAccessibilityEnabled?() ?? true
                    let text: String = "ax#\(i) \(role) '\(axLabel(o))' t='\(axTitle(o))' v='\(axValue(o))'"
                    say(text + " \(Int(f.minX)),\(Int(f.minY)) \(Int(f.width))x\(Int(f.height)) enabled=\(en)")
                }
                if op == "press" {
                    if hits.indices.contains(pick) { let target: AnyObject = hits[pick]; let r1: Bool? = target.accessibilityPerformPress?(); let ok: Bool = r1 ?? false; say("pressed #\(pick) ok=\(ok)") } else { say("press: no match #\(pick) for '\(needle)'") }
                    try? await Task.sleep(for: .milliseconds(900)); say(state(r, nav))
                }
            }
        case "hover":
            // hover <x> <y>: move the real pointer is not ours; send a mouse-moved through the window server path instead
            if let win = mainWindow() {
                let screenH = NSScreen.screens.first?.frame.height ?? 0
                let gpt = CGPoint(x: win.frame.minX + (Double(w[0]) ?? 0), y: screenH - (win.frame.maxY - (Double(w[1]) ?? 0)))
                if let cg = CGEvent(mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: gpt, mouseButton: .left) {
                    cg.setIntegerValueField(CGEventField(rawValue: 91)!, value: Int64(win.windowNumber))
                    cg.setIntegerValueField(CGEventField(rawValue: 92)!, value: Int64(win.windowNumber))
                    if let e = NSEvent(cgEvent: cg) { NSApp.sendEvent(e) }
                }
            }
            try? await Task.sleep(for: .milliseconds(500)); say(state(r, nav))
        case "wsleep":
            // wsleep <secs>: the workspace's sleep and wake notifications, as a lid close sends them
            NSWorkspace.shared.notificationCenter.post(name: NSWorkspace.willSleepNotification, object: NSWorkspace.shared)
            NSWorkspace.shared.notificationCenter.post(name: NSWorkspace.screensDidSleepNotification, object: NSWorkspace.shared)
            try? await Task.sleep(for: .seconds(ms(w, 0, 5)))
            NSWorkspace.shared.notificationCenter.post(name: NSWorkspace.didWakeNotification, object: NSWorkspace.shared)
            NSWorkspace.shared.notificationCenter.post(name: NSWorkspace.screensDidWakeNotification, object: NSWorkspace.shared)
            try? await Task.sleep(for: .milliseconds(800)); say("woke " + state(r, nav))
        case "zoomstate":
            say("zoom \(String(describing: Mirror(reflecting: r).children.filter { ($0.label ?? "").lowercased().contains("zoom") }.map { "\($0.label ?? ""): \($0.value)" }))")
        case "medium":
            if let a = store.medium.asking { store.medium.answer(a, with: w.first == "read" ? .read : (w.first == "watch" ? .watch : .both)); say("medium answered") } else { say("medium: no ask") }
        case "clickw", "pclick":
            // clickw <x> <y> [n]: a click into the key window (sheets); pclick <x> <y> [n]: a click posted through the window server at a point of the main window
            let n = ms(w, 2, 1)
            if op == "clickw", let win = NSApp.keyWindow {
                let p = CGPoint(x: Double(w[0]) ?? 0, y: win.frame.height - (Double(w[1]) ?? 0))
                for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
                    if let e = NSEvent.mouseEvent(with: type, location: p, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                  windowNumber: win.windowNumber, context: nil, eventNumber: 0, clickCount: n, pressure: 1) { NSApp.sendEvent(e) }
                }
            } else if let win = mainWindow() {
                let screenH: CGFloat = NSScreen.screens.first?.frame.height ?? 0
                let gx: CGFloat = win.frame.minX + CGFloat(Double(w[0]) ?? 0)
                let gy: CGFloat = screenH - (win.frame.maxY - CGFloat(Double(w[1]) ?? 0))
                for type in [CGEventType.leftMouseDown, .leftMouseUp] {
                    if let cg = CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: CGPoint(x: gx, y: gy), mouseButton: .left) {
                        cg.setIntegerValueField(.mouseEventClickState, value: Int64(n))
                        cg.postToPid(getpid())
                    }
                }
            }
            try? await Task.sleep(for: .milliseconds(700)); say(state(r, nav))
        case "hwheel":
            // hwheel <dy> <dx> <n> <gapms> [x y from top left] [mouse|trackpad]: wheel events handed to the view under the point, as the window does after its hit test
            if let win = mainWindow(), let root = win.contentView {
                let dy = Int32(ms(w, 0, -3)), dx = Int32(ms(w, 1, 0)), n = ms(w, 2, 10), gap = ms(w, 3, 16)
                let x: CGFloat = w.count > 4 ? CGFloat(Double(w[4]) ?? 0) : win.frame.width / 2
                let yTop: CGFloat = w.count > 5 ? CGFloat(Double(w[5]) ?? 0) : win.frame.height / 2
                let mouse = (w.count > 6 ? w[6] : "mouse") == "mouse"
                let inWin = CGPoint(x: x, y: win.frame.height - yTop)
                let target: NSView? = root.hitTest(inWin)
                var seen = ""
                for i in 0..<n {
                    let units: CGScrollEventUnit = mouse ? .line : .pixel
                    guard let cg = CGEvent(scrollWheelEvent2Source: nil, units: units, wheelCount: 2, wheel1: dy, wheel2: dx, wheel3: 0) else { break }
                    if !mouse {
                        cg.setIntegerValueField(.scrollWheelEventIsContinuous, value: 1)
                        let phase: Int64 = i == 0 ? 1 : (i == n - 1 ? 4 : 2)
                        cg.setIntegerValueField(.scrollWheelEventScrollPhase, value: phase)
                    }
                    if let e = NSEvent(cgEvent: cg) {
                        if i == 0 { seen = "dy=\(e.scrollingDeltaY) precise=\(e.hasPreciseScrollingDeltas) phase=\(e.phase.rawValue)" }
                        target?.scrollWheel(with: e)
                    }
                    try? await Task.sleep(for: .milliseconds(gap))
                }
                let name: String = target.map { String(describing: type(of: $0)) } ?? "nil"
                say("hwheel target=\(name) \(seen)")
            }
            try? await Task.sleep(for: .milliseconds(500)); say(state(r, nav))
        case "qkey":
            // qkey <name> [mods]: a key posted to the app's event queue, so the main event loop hands it out as it does a real one
            if let name = w.first, let (code, chars) = codes[name], let win = NSApp.keyWindow ?? mainWindow() {
                var flags: NSEvent.ModifierFlags = []
                for m in w.dropFirst() {
                    switch m { case "cmd": flags.insert(.command); case "shift": flags.insert(.shift); case "opt": flags.insert(.option); case "ctrl": flags.insert(.control); default: break }
                }
                if ["left", "right", "up", "down", "pgdn", "pgup", "home", "end"].contains(name) { flags.insert([.function, .numericPad]) }
                let number = win.windowNumber
                for type in [NSEvent.EventType.keyDown, .keyUp] {
                    let e = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                             windowNumber: number, context: nil, characters: chars,
                                             charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code)
                    if let e { NSApp.postEvent(e, atStart: false) }
                }
            }
            try? await Task.sleep(for: .milliseconds(600)); say(state(r, nav))
        case "qmenu":
            // qmenu <title>: the menu item performed from the run loop rather than from inside this task
            let title = w.joined(separator: " ")
            RunLoop.main.perform { MainActor.assumeIsolated { menu(title) } }
            try? await Task.sleep(for: .milliseconds(800)); say(state(r, nav))
        case "dquit":
            // dquit: NSApp.terminate from the run loop, as the Quit menu item runs it
            RunLoop.main.perform { MainActor.assumeIsolated { NSApp.terminate(nil) } }
        case "lookkey": if let win = mainWindow() { pinned = win; lookKey(win) }; say("lookkey key=\(String(describing: NSApp.keyWindow?.title))")
        case "rset":
            // rset <key> <c|a|t> <value>: a reader setting, as the settings sheet sets it (r7)
            let keys: [String: ReaderSettingKey] = ["look": .look, "font": .font, "textSize": .textSize, "margins": .margins,
                "lineSpacing": .lineSpacing, "justify": .justify, "mode": .mode, "background": .background, "fit": .fit,
                "direction": .direction, "pageLayout": .pageLayout, "footValue": .footValue]
            if w.count >= 3, let k = keys[w[0]] {
                let v: ReaderValue = switch w[1] { case "a": .amount(value: Double(w[2]) ?? 1); case "t": .toggle(on: w[2] == "on"); default: .choice(name: w[2]) }
                r.set(k, v)
                try? await Task.sleep(for: .milliseconds(1500))
                say("rset \(w) " + state(r, nav))
            } else { say("rset? \(w)") }
        case "qpress":
            // qpress <text…> [#n]: press from the run loop, so a modal panel it opens doesn't hold the driver (r7)
            var words = w; var pick = 0
            if let last = words.last, last.hasPrefix("#"), let k = Int(last.dropFirst()) { pick = k; words.removeLast() }
            let needle = words.joined(separator: " ").lowercased()
            var hits: [AnyObject] = []
            func walk(_ o: AnyObject, _ d: Int) {
                guard d < 60 else { return }
                if (axLabel(o) + "|" + axTitle(o) + "|" + axValue(o)).lowercased().contains(needle) { hits.append(o) }
                for c in (o.accessibilityChildren?() ?? nil) ?? [] { walk(c as AnyObject, d + 1) }
            }
            for win in NSApp.windows where win.isVisible { walk(win, 0) }
            if hits.indices.contains(pick) {
                nonisolated(unsafe) let target = hits[pick]
                say("qpress #\(pick) of \(hits.count) '\(axLabel(target))' t='\(axTitle(target))'")
                RunLoop.main.perform { MainActor.assumeIsolated { _ = target.accessibilityPerformPress?() } }
            } else { say("qpress: no match #\(pick) for '\(needle)'") }
            try? await Task.sleep(for: .milliseconds(1500)); say(state(r, nav))
        case "rsettings":
            // rsettings: the reader's settings as the sheet shows them (r7)
            say("rsettings \(String(describing: r.settings))")
        case "wins":
            // wins: every window with its tabs, level and class (r7)
            for x in NSApp.windows {
                say("win '\(x.title)' cls=\(type(of: x)) id=\(x.identifier?.rawValue ?? "-") vis=\(x.isVisible) key=\(x.isKeyWindow) main=\(x.isMainWindow) tabbing=\(x.tabbingMode.rawValue) tabs=\(x.tabbedWindows?.count ?? 0) sheet=\(x.isSheet) attached=\(x.attachedSheet.map { String(describing: type(of: $0)) } ?? "-") frame=\(x.frame) fr=\(x.firstResponder.map { String(describing: type(of: $0)) } ?? "nil")")
            }
        case "panel":
            // panel [list|cancel|ok <dir>]: the open panel the app shows, if any (r7)
            let panels = NSApp.windows.compactMap { $0 as? NSOpenPanel }
            say("panels=\(panels.count) " + panels.map { "vis=\($0.isVisible) dir=\($0.directoryURL?.path ?? "-") dirs=\($0.canChooseDirectories) files=\($0.canChooseFiles) multi=\($0.allowsMultipleSelection)" }.joined(separator: "; "))
            if let p = panels.first(where: { $0.isVisible }) {
                switch w.first {
                case "cancel": p.cancel(nil)
                case "ok":
                    if w.count > 1 { p.directoryURL = URL(filePath: w[1], directoryHint: .isDirectory) }
                    try? await Task.sleep(for: .milliseconds(1200))
                    p.ok(nil)
                default: break
                }
                try? await Task.sleep(for: .milliseconds(1500))
                say("panel after \(w.first ?? "list") " + state(r, nav))
            }
        case "axw":
            // axw <title prefix> [depth]: the accessibility tree of another window (r7)
            let t = w.first ?? ""
            if let win = NSApp.windows.first(where: { $0.title.hasPrefix(t) && $0.isVisible }) { dumpAX(win, depth: 0, max: ms(w, 1, 40)) } else { say("no window '\(t)'") }
        case "qch":
            // qch <char> [mods]: any character as a key press through the event queue, for menu keys the code table lacks (r7)
            if let c = w.first, let win = NSApp.keyWindow ?? mainWindow() {
                var flags: NSEvent.ModifierFlags = []
                for m in w.dropFirst() {
                    switch m { case "cmd": flags.insert(.command); case "shift": flags.insert(.shift); case "opt": flags.insert(.option); case "ctrl": flags.insert(.control); default: break }
                }
                let chars = c == "SP" ? " " : c
                for type in [NSEvent.EventType.keyDown, .keyUp] {
                    if let e = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                                windowNumber: win.windowNumber, context: nil, characters: chars,
                                                charactersIgnoringModifiers: chars, isARepeat: false, keyCode: codes[chars]?.0 ?? 0) {
                        NSApp.postEvent(e, atStart: false)
                    }
                }
            }
            try? await Task.sleep(for: .milliseconds(600)); say(state(r, nav))
        #endif
        default: say("?? \(op)")
        }
    }

    #if os(macOS)
    static func axLabel(_ obj: AnyObject) -> String { (obj.accessibilityLabel?() ?? nil) ?? "" }
    static func axTitle(_ obj: AnyObject) -> String { (obj.accessibilityTitle?() ?? nil) ?? "" }
    static func axValue(_ obj: AnyObject) -> String { (obj.accessibilityValue?() ?? nil).map { "\($0)" } ?? "" }
    #endif

    static func dlText(_ d: ChapterDownloadState?) -> String {
        guard let d else { return "-" }
        switch d {
        case .queued: return "Q"
        case .waitsForWifi: return "W"
        case .downloading(let done, let total): return "\(done)/\(total)"
        case .done: return "D"
        case .failed(let f): return "F(\(f))"
        }
    }

    static func dumpChapters(_ store: AppStore, _ work: String, max: Int) {
        guard let c = store.reading.chapters[work] else { say("chapters \(work) not loaded"); return }
        let all = c.volumes.flatMap(\.chapters)
        say("chapters \(work) edition=\(c.edition.map { "\($0.sourceName)/\($0.chapters)" } ?? "nil") groups=\(c.volumes.count) rows=\(all.count) failure=\(c.failure.map { "\($0)" } ?? "-")")
        var n = 0
        for g in c.volumes {
            say(" group vol=\(g.volume ?? "-") title='\(g.title ?? "")' flat=\(g.flat) allRead=\(g.allRead) rows=\(g.chapters.count)")
            for row in g.chapters {
                n += 1
                if n > max { continue }
                let line = row.line.map(\.text).joined(separator: " · ")
                say("  \(row.id) num=\(row.number ?? "-") label='\(row.label)' title='\(row.title ?? "")' vol=\(row.volume ?? "-") st=\(row.state) dl=\(dlText(row.download)) parts=\(row.parts) other=\(row.otherSource ?? "-") ver=\(row.versionLabel ?? "-") line='\(line)'")
            }
        }
        if n > max { say("  ... \(n - max) more rows") }
    }

    #if os(macOS)
    /// AppKit asks whether the window and app are key and active; answer yes, so a locked screen
    /// still draws the window as key and routes keys to it (the dev harness's KeyLook, plus the
    /// app's keyWindow).
    static func lookKey(_ window: NSWindow) {
        let yes: @convention(block) (AnyObject) -> Bool = { _ in true }
        let imp = imp_implementationWithBlock(yes)
        for name in ["isKeyWindow", "isMainWindow", "_hasActiveAppearance", "_hasKeyAppearance",
                     "_hasMainAppearance", "_hasActiveControls", "_hasActiveAppearanceIgnoringKeyFocus"] {
            replace(name, in: type(of: window), with: imp)
        }
        for name in ["isActive", "_isActive"] { replace(name, in: NSApplication.self, with: imp) }
        let keyBlock: @convention(block) (AnyObject) -> NSWindow? = { _ in HuntDriver.pinned }
        let keyImp = imp_implementationWithBlock(keyBlock)
        for name in ["keyWindow", "mainWindow"] { replace(name, in: NSApplication.self, with: keyImp) }
        NotificationCenter.default.post(name: NSApplication.didBecomeActiveNotification, object: NSApp)
        NotificationCenter.default.post(name: NSWindow.didBecomeMainNotification, object: window)
        NotificationCenter.default.post(name: NSWindow.didBecomeKeyNotification, object: window)
        window.makeFirstResponder(window.contentView)
        window.contentView?.superview?.needsDisplay = true
        window.contentView?.superview?.needsLayout = true
        window.update()
    }

    nonisolated(unsafe) static weak var pinned: NSWindow?

    static func replace(_ name: String, in cls: AnyClass, with imp: IMP) {
        let selector = NSSelectorFromString(name)
        guard let method = class_getInstanceMethod(cls, selector) else { return }
        class_replaceMethod(cls, selector, imp, method_getTypeEncoding(method))
    }

    /// The window as the window server composites it (toolbar effects, full screen), through
    /// CGWindowListCreateImage looked up at run time; an app may capture its own windows.
    static func rshot(_ name: String) async {
        guard let dir, let win = mainWindow() else { say("no window"); return }
        try? await Task.sleep(for: .milliseconds(300))
        typealias Fn = @convention(c) (CGRect, UInt32, UInt32, UInt32) -> Unmanaged<CGImage>?
        guard let h = dlopen(nil, RTLD_NOW), let sym = dlsym(h, "CGWindowListCreateImage") else { say("rshot no symbol"); return }
        let fn = unsafeBitCast(sym, to: Fn.self)
        // kCGWindowListOptionIncludingWindow = 8, kCGWindowImageBoundsIgnoreFraming = 1, BestResolution = 8
        guard let img = fn(.null, 8, UInt32(win.windowNumber), 1 | 8)?.takeRetainedValue() else { say("rshot nil (n=\(win.windowNumber))"); return }
        let rep = NSBitmapImageRep(cgImage: img)
        let url = dir.appending(path: "\(name).png")
        try? rep.representation(using: .png, properties: [:])?.write(to: url)
        say("rshot \(url.lastPathComponent) \(img.width)x\(img.height) full=\(win.styleMask.contains(.fullScreen)) frame=\(win.frame)")
    }

    static func shot(_ name: String) async {
        guard let dir, let win = mainWindow(), let view = win.contentView?.superview ?? win.contentView else { say("no window"); return }
        var webs: [(WKWebView, CGImage)] = []
        var found: [WKWebView] = []
        func find(_ v: NSView) { if let web = v as? WKWebView { found.append(web) }; v.subviews.forEach(find) }
        find(view)
        for web in found where !web.isHidden && web.bounds.width > 1 {
            if let p = try? await web.takeSnapshot(configuration: WKSnapshotConfiguration()),
               let cg = p.cgImage(forProposedRect: nil, context: nil, hints: nil) { webs.append((web, cg)) }
        }
        view.layoutSubtreeIfNeeded()
        view.displayIfNeeded()
        guard let bitmap = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { return }
        view.cacheDisplay(in: view.bounds, to: bitmap)
        guard let base = bitmap.cgImage else { return }
        let scale = CGFloat(base.width) / view.bounds.width
        let ctx = CGContext(data: nil, width: base.width, height: base.height, bitsPerComponent: 8, bytesPerRow: 0,
                            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.draw(base, in: CGRect(x: 0, y: 0, width: base.width, height: base.height))
        ctx.setBlendMode(.multiply)
        for (web, cg) in webs {
            let f = web.convert(web.bounds, to: view)
            let y = view.isFlipped ? view.bounds.height - f.maxY : f.minY
            ctx.draw(cg, in: CGRect(x: f.minX * scale, y: y * scale, width: f.width * scale, height: f.height * scale))
        }
        guard let img = ctx.makeImage() else { return }
        let rep = NSBitmapImageRep(cgImage: img)
        let url = dir.appending(path: "\(name).png")
        try? rep.representation(using: .png, properties: [:])?.write(to: url)
        say("shot \(url.lastPathComponent) \(img.width)x\(img.height) webs=\(webs.count)")
    }
    #endif

    #if os(macOS)
    static func mainWindow() -> NSWindow? {
        NSApp.windows.first { $0.identifier?.rawValue.hasPrefix("main") == true } ?? NSApp.windows.first { $0.isVisible && $0.canBecomeMain }
    }

    static let codes: [String: (UInt16, String)] = [
        "space": (49, " "), "left": (123, "\u{F702}"), "right": (124, "\u{F703}"), "down": (125, "\u{F701}"),
        "up": (126, "\u{F700}"), "pgdn": (121, "\u{F72D}"), "pgup": (116, "\u{F72C}"), "home": (115, "\u{F729}"),
        "end": (119, "\u{F72B}"), "esc": (53, "\u{1B}"), "]": (30, "]"), "[": (33, "["), "w": (13, "w"), "g": (5, "g"),
        "l": (37, "l"), "t": (17, "t"), "=": (24, "="), "+": (24, "+"), "-": (27, "-"), "0": (29, "0"), "f": (3, "f"),
        "k": (40, "k"), "1": (18, "1"), "2": (19, "2"), "5": (23, "5"), "return": (36, "\r"), "tab": (48, "\t"),
        "m": (46, "m"), "q": (12, "q"), ",": (43, ","), "a": (0, "a"), "n": (45, "n"), "delete": (51, "\u{8}"), "j": (38, "j"),
        "r": (15, "r"), "b": (11, "b"), "o": (31, "o"), "d": (2, "d"), "e": (14, "e"), "3": (20, "3"), "9": (25, "9"), "s": (1, "s"), "p": (35, "p"),
    ]

    /// key <name> [cmd|shift|opt|ctrl…]: sent through NSApp, as a real key press is, to the main window.
    static func fields() -> String {
        var out: [String] = []
        func walk(_ v: NSView) { if let f = v as? NSTextField, f.isEditable { out.append("'\(f.stringValue)'") }; v.subviews.forEach(walk) }
        for win in NSApp.windows where win.isVisible { if let c = win.contentView { walk(c) } }
        return out.joined(separator: ",")
    }

    static func key(_ w: [String], target: NSWindow? = nil) {
        guard let name = w.first, let (code, chars) = codes[name], let win = target ?? mainWindow() else { say("no key \(w)"); return }
        var flags: NSEvent.ModifierFlags = []
        for m in w.dropFirst() {
            switch m { case "cmd": flags.insert(.command); case "shift": flags.insert(.shift); case "opt": flags.insert(.option); case "ctrl": flags.insert(.control); default: break }
        }
        if ["left", "right", "up", "down", "pgdn", "pgup", "home", "end"].contains(name) { flags.insert([.function, .numericPad]) }
        if target == nil, !win.isKeyWindow { win.makeKey() }
        for type in [NSEvent.EventType.keyDown, .keyUp] {
            if let e = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                                        windowNumber: win.windowNumber, context: nil, characters: chars,
                                        charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code) {
                NSApp.sendEvent(e)
            }
        }
    }

    static func find(_ title: String, in menu: NSMenu) -> (NSMenu, Int)? {
        for (i, item) in menu.items.enumerated() {
            if item.title == title { return (menu, i) }
            if let sub = item.submenu, let hit = find(title, in: sub) { return hit }
        }
        return nil
    }

    static func menu(_ title: String) {
        // r7: "Top>Item" looks only under that top menu.
        var root = NSApp.mainMenu
        var title = title
        if let gt = title.firstIndex(of: ">") {
            let top = String(title[..<gt]).trimmingCharacters(in: .whitespaces)
            title = String(title[title.index(after: gt)...]).trimmingCharacters(in: .whitespaces)
            root = NSApp.mainMenu?.items.first { $0.title == top }?.submenu
        }
        guard let main = root, let (m, i) = find(title, in: main) else { say("no menu item '\(title)'"); return }
        m.update()
        let item = m.items[i]
        say("menu '\(title)' enabled=\(item.isEnabled) key=\(item.keyEquivalent.debugDescription) mods=\(item.keyEquivalentModifierMask.rawValue)")
        if item.isEnabled { m.performActionForItem(at: i) }
    }

    static func dumpMenus() {
        guard let main = NSApp.mainMenu else { return }
        for top in main.items {
            guard let sub = top.submenu else { continue }
            sub.update()
            for item in sub.items where !item.isSeparatorItem {
                say("menu \(top.title) > \(item.title) key=\(item.keyEquivalent.debugDescription) mods=\(item.keyEquivalentModifierMask.rawValue) on=\(item.isEnabled) hidden=\(item.isHidden)")
            }
        }
    }

    static func window(_ w: [String]) async {
        guard let win = mainWindow() else { say("no window"); return }
        switch w.first {
        case "size":
            let width = Double(w[1]) ?? 1280, height = Double(w[2]) ?? 800
            var f = win.frame; f.size = CGSize(width: width, height: height)
            win.setFrame(f, display: true, animate: false)
        case "steps":
            // win steps <from> <to> <step> <ms>: resize the width in steps, as a drag does
            let a = Int(w[1]) ?? 1280, b = Int(w[2]) ?? 960, step = Int(w[3]) ?? 20, gap = Int(w[4]) ?? 16
            for x in stride(from: a, through: b, by: a < b ? step : -step) {
                var f = win.frame; f.size.width = CGFloat(x)
                win.setFrame(f, display: true, animate: false)
                try? await Task.sleep(for: .milliseconds(gap))
            }
        case "full": win.toggleFullScreen(nil)
        case "mini": win.miniaturize(nil)
        case "demini": win.deminiaturize(nil)
        case "close": win.performClose(nil)
        case "front": win.orderFront(nil)
        case "back": win.orderBack(nil)
        default: break
        }
        try? await Task.sleep(for: .milliseconds(600))
        for x in NSApp.windows {
            say("window '\(x.title)' id=\(x.identifier?.rawValue ?? "-") n=\(x.windowNumber) frame=\(x.frame) visible=\(x.isVisible) key=\(x.isKeyWindow) full=\(x.styleMask.contains(.fullScreen)) occ=\(x.occlusionState.contains(.visible)) min=\(x.minSize)")
        }
    }

    static func dumpAX(_ e: Any, depth: Int, max: Int) {
        guard depth < max else { return }
        let el = e as? NSAccessibilityProtocol
        let obj = e as AnyObject
        let role = (obj.accessibilityRole?() ?? nil)?.rawValue ?? "?"
        let label = (obj.accessibilityLabel?() ?? nil) ?? ""
        let value = (obj.accessibilityValue?() ?? nil).map { "\($0)" } ?? ""
        let title = (obj.accessibilityTitle?() ?? nil) ?? ""
        let frame = obj.accessibilityFrame?() ?? .zero
        if role != "?" || !label.isEmpty {
            say(String(repeating: " ", count: depth) + "\(role) '\(label)' t='\(title)' v='\(value)' \(Int(frame.origin.x)),\(Int(frame.origin.y)) \(Int(frame.width))x\(Int(frame.height))")
        }
        _ = el
        for child in (obj.accessibilityChildren?() ?? nil) ?? [] {
            dumpAX(child, depth: depth + 1, max: max)
        }
    }
    #else
    static func keyWindowIOS() -> UIWindow? {
        UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.flatMap(\.windows).first { $0.isKeyWindow }
    }

    static func dumpAXIOS(_ e: NSObject, depth: Int, max: Int) {
        guard depth < max else { return }
        if e.isAccessibilityElement {
            let f = e.accessibilityFrame
            say(String(repeating: " ", count: depth) + "'\(e.accessibilityLabel ?? "")' v='\(e.accessibilityValue ?? "")' traits=\(e.accessibilityTraits.rawValue) \(si(f.origin.x)),\(si(f.origin.y)) \(si(f.width))x\(si(f.height))")
            return
        }
        if let els = e.accessibilityElements as? [NSObject], !els.isEmpty {
            for c in els { dumpAXIOS(c, depth: depth + 1, max: max) }
            return
        }
        let n = e.accessibilityElementCount()
        if n > 0, n != NSNotFound {
            for i in 0..<n { if let c = e.accessibilityElement(at: i) as? NSObject { dumpAXIOS(c, depth: depth + 1, max: max) } }
            return
        }
        if let v = e as? UIView, !v.isHidden, v.alpha > 0 {
            for c in v.subviews { dumpAXIOS(c, depth: depth + 1, max: max) }
        }
    }
    #endif
}
