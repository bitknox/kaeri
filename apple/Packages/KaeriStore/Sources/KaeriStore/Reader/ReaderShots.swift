// The reader staged for a screenshot: the Fixtures app and the dev harness open the reading
// scene's work, wait for its pages, and put the chrome or a sheet up as the arguments say.

#if FIXTURES
import CoreGraphics
import Foundation
import ImageIO
import KaeriKit
import KaeriStrings
import UniformTypeIdentifiers

extension ReaderModel {
    /// `-KaeriReader chrome|settings|chapters|chapterlist|ask|end|endskip|scrub|plate|opening|error`, and with
    /// `-KaeriCredits` `creditline|creditchrome|creditshown|creditshownchrome|creditkept|parts|partscrub|partmissingscrub`,
    /// after a reader scene.
    public static let argument = "-KaeriReader"

    /// The work a reader scene reads.
    public static func shotWork(arguments: [String]) -> String? {
        guard let at = arguments.firstIndex(where: { $0 == "-KaeriScene" || $0 == "--scene" }), arguments.indices.contains(at + 1) else { return nil }
        switch arguments[at + 1].lowercased() {
        case "readermangartl", "readermoved": return "a:900301"
        // The long series opens in the reader only when a reader shot asks; otherwise its page is shot.
        case "readerlong": return arguments.contains(argument) ? "a:900301" : nil
        case "readernovel": return "a:900302"
        case "readerstrip": return "a:900303"
        // The reading demo's comic, for its reader and its end (prelaunch=demo-reader, demo-end).
        case "readingdemo":
            guard let scene = PrelaunchScene.named(in: arguments), scene == .demoReader || scene == .demoEnd else { return nil }
            return readingDemoInfo().works.first { !$0.novel }?.work
        default: return nil
        }
    }

    /// Open the scene's work and wait until it has pages to show. False when it never did.
    public func stage(arguments: [String] = ProcessInfo.processInfo.arguments) async -> Bool {
        guard let work = Self.shotWork(arguments: arguments) else { return true }
        if let at = arguments.firstIndex(of: Self.argument), arguments.indices.contains(at + 1), arguments[at + 1] == "plate" {
            holdPages = true
        }
        open(work: work)
        let drawn = arguments.contains("-KaeriReaderDrawn")
        let deadline = ContinuousClock.now + .seconds(drawn ? 6 : 25)
        while ContinuousClock.now < deadline {
            if phase == .open, novel != nil || !pages.isEmpty { break }
            try? await Task.sleep(for: .milliseconds(100))
        }
        // `-KaeriReaderDrawn`: the engine's answer played from pages drawn for the shot, for an
        // engine without a book source.
        if drawn, phase != .open || (novel == nil && pages.isEmpty) {
            playFixture(work: work, scene: arguments)
            try? await Task.sleep(for: .milliseconds(800))
        }
        // `-KaeriReaderSet look=paper,paper=on,layout=double,at=first`: settings as the settings
        // sheet would set them, once the book is open and its medium known, and at=first the
        // chapter's first page, as Home in the Go menu.
        if let at = arguments.firstIndex(of: "-KaeriReaderSet"), arguments.indices.contains(at + 1) {
            for pair in arguments[at + 1].split(separator: ",") {
                let parts = pair.split(separator: "=", maxSplits: 1).map(String.init)
                guard parts.count == 2 else { continue }
                switch parts[0] {
                case "look": set(.look, .choice(name: parts[1]))
                case "layout": set(.pageLayout, .choice(name: parts[1]))
                case "paper": set(.paperMode, .toggle(on: parts[1] == "on"))
                case "at" where parts[1] == "first": requestJump(0)
                default: continue
                }
            }
            try? await Task.sleep(for: .milliseconds(600))
        }
        // A novel is shot once its paginator has put a page on screen.
        while novel != nil, !novelPainted, ContinuousClock.now < deadline {
            try? await Task.sleep(for: .milliseconds(100))
        }
        try? await Task.sleep(for: .milliseconds(novel != nil ? 2500 : 1500))
        let shown = phase == .open
        // `-KaeriCredits start|end|both|parts|partsmissing`: credit pages the engine skipped, and parts, for
        // their shots.
        if let at = arguments.firstIndex(of: "-KaeriCredits"), arguments.indices.contains(at + 1) {
            stageCreditShot(arguments[at + 1])
        }
        if let at = arguments.firstIndex(of: Self.argument), arguments.indices.contains(at + 1) {
            switch arguments[at + 1] {
            // Page 1 with its line, alone or with the chrome; a strip scrolled to its top.
            case "creditline", "creditchrome":
                notice = nil
                requestJump(0)
                try? await Task.sleep(for: .milliseconds(600))
                if view?.mode == .strip {
                    requestTurn(forward: false)
                } else {
                    stageCreditLine()
                    if arguments[at + 1] == "creditchrome" { chromeShown = true }
                }
            // The credit page shown, then kept with its Undo; in a strip, shown in place.
            case "creditshown", "creditshownchrome", "creditkept":
                notice = nil
                requestJump(0)
                try? await Task.sleep(for: .milliseconds(600))
                if view?.mode == .strip, let chapter = view?.chapter.id {
                    reveal(startSkipped, chapter: chapter)
                    try? await Task.sleep(for: .milliseconds(300))
                    requestTurn(forward: false)
                } else {
                    showStartCredits()
                    try? await Task.sleep(for: .milliseconds(400))
                    if arguments[at + 1] == "creditkept" { keepCredit() }
                    if arguments[at + 1] != "creditshown" { chromeShown = true }
                }
            // The scrubber held on the first later part's tick.
            case "partscrub":
                notice = nil
                try? await Task.sleep(for: .milliseconds(400))
                chromeShown = true
                shotScrub = partTicks.first?.leaf ?? max(leaves.count / 2, 1)
            // The scrubber held on the tick of a part whose part before it is missing.
            case "partmissingscrub":
                notice = nil
                try? await Task.sleep(for: .milliseconds(400))
                chromeShown = true
                shotScrub = partTicks.first { $0.missing }?.leaf ?? max(leaves.count / 2, 1)
            // Past the first later part, so its tick sits behind the thumb on the track.
            case "parts":
                notice = nil
                requestJump(partTicks.first.map { min($0.leaf + 2, leaves.count - 1) } ?? 0)
                try? await Task.sleep(for: .milliseconds(600))
                chromeShown = true
            case "chrome": chromeShown = true
            case "settings": chromeShown = true; sheet = .settings
            case "chapters", "chapterlist": chromeShown = true; sheet = .chapters
            case "ask": chromeShown = true; askingPage = true
            // The end page is reached by reading on, by which time the resume notice has gone.
            // A strip has no end page: it scrolls on past its last image to the marker into the
            // next chapter.
            case "end", "endskip":
                notice = nil
                requestJump(max(leaves.count - 1, 0))
                if view?.mode == .strip {
                    try? await Task.sleep(for: .milliseconds(700))
                    requestTurn(forward: true)
                }
                if arguments[at + 1] == "endskip" {
                    try? await Task.sleep(for: .milliseconds(700))
                    stageSkipExtra()
                }
            case "scrub": chromeShown = true; shotScrub = max(leaves.count / 4, 1)
            case "pill": chromeShown = true; backToPage = 9
            case "opening": hold(.opening)
            case "error": hold(.failed(.offline))
            // The chapter failing with the group up, as Continue on an end page can leave it.
            case "errorchrome": chromeShown = true; hold(.failed(.offline))
            default: break
            }
            try? await Task.sleep(for: .milliseconds(900))
        }
        // `-KaeriAutoTurn <n>`: n turns forward, 0.8 s apart, for a recording of the turns or
        // of a strip flowing on into the next chapter.
        if let at = arguments.firstIndex(of: "-KaeriAutoTurn"), arguments.indices.contains(at + 1),
           let turns = Int(arguments[at + 1]) {
            for _ in 0..<turns {
                try? await Task.sleep(for: .milliseconds(800))
                requestTurn(forward: true)
            }
            try? await Task.sleep(for: .milliseconds(900))
        }
        // `-KaeriContinueAfter <ms>`: Continue on the end page that many milliseconds after the
        // scene is ready, for a recording of the move into the next chapter. With
        // `-KaeriSlowPages <ms>` the next chapter's pages take that long to arrive.
        if let at = arguments.firstIndex(of: "-KaeriContinueAfter"), arguments.indices.contains(at + 1),
           let wait = Int(arguments[at + 1]) {
            let slow = arguments.firstIndex(of: "-KaeriSlowPages").flatMap { at in
                arguments.indices.contains(at + 1) ? Int(arguments[at + 1]) : nil
            }
            Task { @MainActor [weak self] in
                try? await Task.sleep(for: .milliseconds(wait))
                guard let self, let next = end?.next ?? view?.next else { return }
                if slow != nil { holdPages = true }
                openChapter(next.id)
                if let slow {
                    try? await Task.sleep(for: .milliseconds(slow))
                    holdPages = false
                }
            }
        }
        if Self.demoAsked { await resumeForDemo(work: work) }
        _ = shown
        if Self.demoAsked { playBackToDemo() }
        return true
    }

    /// `-KaeriReaderDemo`: Back to page's moments for a recording, with the chrome up for each.
    /// The reader opens again at a page kept from the first open, then moves one page, jumps
    /// ten (or to the last page of a shorter chapter), turns once and, in a novel, lays out again
    /// at another text size.
    public static var demoAsked: Bool { ProcessInfo.processInfo.arguments.contains("-KaeriReaderDemo") }

    /// A place to resume at: the sixth page shown, or nearer the start of a short chapter, then
    /// the reader closed and opened again. Pages show one at a time, so a leaf is a page.
    private func resumeForDemo(work: String) async {
        if novel != nil {
            // Large text, so the scene's short first document runs to enough pages for a jump.
            set(.textSize, .amount(value: 2.4))
            try? await Task.sleep(for: .milliseconds(1200))
        } else if view?.mode != .strip {
            set(.pageLayout, .choice(name: "single"))
            try? await Task.sleep(for: .milliseconds(800))
        }
        requestJump(min(5, position.count / 4))
        try? await Task.sleep(for: .milliseconds(1500))
        close()
        try? await Task.sleep(for: .milliseconds(600))
        open(work: work)
        let deadline = ContinuousClock.now + .seconds(10)
        while ContinuousClock.now < deadline {
            if phase == .open, novel != nil ? novelPainted : !pages.isEmpty { break }
            try? await Task.sleep(for: .milliseconds(100))
        }
        try? await Task.sleep(for: .milliseconds(1200))
        notice = nil
    }

    private func playBackToDemo() {
        let steps: [(ms: Int, act: @MainActor (ReaderModel) -> Void)] = [
            (1000, { $0.chromeShown = true }),
            (1500, { $0.requestJump($0.position.index + 1) }),
            (300, { $0.chromeShown = true }),
            // A strip's turn scrolls on within its last image: the jump stops one short of it.
            (1700, { $0.requestJump(min($0.position.index + 10, $0.position.count - ($0.view?.mode == .strip ? 2 : 1))) }),
            (300, { $0.chromeShown = true }),
            (2700, { $0.requestTurn(forward: true) }),
            (500, { $0.chromeShown = true }),
            // A strip's turn can stay within a tall image; it goes once the scroll is on the next.
            (800, { if $0.view?.mode == .strip { $0.requestTurn(forward: true) } }),
            (800, { if $0.view?.mode == .strip { $0.requestTurn(forward: true) } }),
            (400, { $0.chromeShown = true }),
            (1000, { if $0.novel != nil { $0.set(.textSize, .amount(value: 1.8)) } }),
            (1200, { $0.chromeShown = true }),
        ]
        Task { @MainActor [weak self] in
            for step in steps {
                try? await Task.sleep(for: .milliseconds(step.ms))
                guard let self else { return }
                step.act(self)
            }
        }
    }
}

extension ReaderModel {
    /// Credit pages and parts for the open chapter of a drawn scene: a page skipped at the start,
    /// at the end, both, or a chapter in two parts.
    func stageCreditShot(_ what: String) {
        guard view != nil else { return }
        let count = UInt32(pageCount)
        if what == "parts" {
            let second = max(count / 2, 2) + 1
            stageCredits([], parts: ReaderParts(
                parts: [ChapterPart(number: 1, firstPage: 1, group: nil, otherGroup: false),
                        ChapterPart(number: 2, firstPage: second, group: nil, otherGroup: false)],
                total: 2, missing: []))
            return
        }
        // Parts 1, 2 and 4, with 3 missing: "Page 14 · part 4 · part 3 missing" on the scrubber.
        if what == "partsmissing" {
            let second = max(count / 4, 2) + 1, fourth = max(count * 2 / 3, second + 1) + 1
            stageCredits([], parts: ReaderParts(
                parts: [ChapterPart(number: 1, firstPage: 1, group: nil, otherGroup: false),
                        ChapterPart(number: 2, firstPage: second, group: nil, otherGroup: false),
                        ChapterPart(number: 4, firstPage: fourth, group: nil, otherGroup: false)],
                total: 4, missing: [3]))
            return
        }
        let dir = FileManager.default.temporaryDirectory.appending(path: "kaeri-reader-shot", directoryHint: .isDirectory)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let strip = view?.mode == .strip
        let url = dir.appending(path: strip ? "credit-strip.png" : "credit.png")
        drawCredit(strip: strip, to: url)
        let file = PageFile(
            path: url.path, encoding: .original(mime: "image/png"), width: 800, height: strip ? 1000 : 1200,
            crop: nil, colour: true, bytes: 1)
        // One at an end, or two at each end for `both` (the board's Edges: "2 credit pages
        // skipped", "credit page 1 of 2", and the end page naming only its own).
        let each: UInt64 = what == "both" ? 2 : 1
        var pages: [CreditPage] = []
        if what == "start" || what == "both" {
            for n in 0..<each {
                pages.append(CreditPage(page: UInt32(n), atEnd: false, file: file, fingerprint: 0xC0FFEE + n * 16))
            }
        }
        if what == "end" || what == "both" {
            for n in 0..<each {
                pages.append(CreditPage(page: count + UInt32(pages.count), atEnd: true, file: file, fingerprint: 0xC0FFEF + n * 16))
            }
        }
        stageCredits(pages)
    }

    /// A made-up credit page: a lantern drawn in gold lines on a deep blue ground, in a frame.
    private func drawCredit(strip: Bool, to url: URL) {
        let width = 800, height = strip ? 1000 : 1200
        guard let context = CGContext(
            data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return }
        let w = CGFloat(width), h = CGFloat(height)
        context.setFillColor(CGColor(red: 0.17, green: 0.18, blue: 0.35, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: w, height: h))
        let gold = CGColor(red: 0.95, green: 0.78, blue: 0.42, alpha: 1)
        context.setStrokeColor(gold)
        context.setLineWidth(4)
        context.stroke(CGRect(x: 40, y: 40, width: w - 80, height: h - 80))
        // The lantern: a cord, a cap, a round body with ribs, and a foot.
        let cx = w / 2, cy = h * 0.6
        context.move(to: CGPoint(x: cx, y: h - 40))
        context.addLine(to: CGPoint(x: cx, y: cy + 190))
        context.strokePath()
        context.setFillColor(gold)
        context.fill(CGRect(x: cx - 60, y: cy + 160, width: 120, height: 26))
        context.fill(CGRect(x: cx - 50, y: cy - 186, width: 100, height: 22))
        context.strokeEllipse(in: CGRect(x: cx - 150, y: cy - 160, width: 300, height: 320))
        context.strokeEllipse(in: CGRect(x: cx - 80, y: cy - 160, width: 160, height: 320))
        context.setFillColor(CGColor(red: 0.85, green: 0.7, blue: 0.4, alpha: 1))
        context.fillEllipse(in: CGRect(x: cx - 50, y: cy - 80, width: 100, height: 160))
        // Lines where the credits would be written.
        context.setFillColor(gold)
        for row in 0..<4 {
            let y = h * 0.22 - CGFloat(row) * 34
            context.fill(CGRect(x: cx - 150, y: y, width: 110, height: 10))
            context.fill(CGRect(x: cx + 10, y: y, width: 70, height: 10))
        }
        context.fill(CGRect(x: cx - 180, y: h * 0.3, width: 360, height: 26))
        guard let image = context.makeImage(),
            let target = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)
        else { return }
        CGImageDestinationAddImage(target, image, nil)
        CGImageDestinationFinalize(target)
    }

    /// What the engine would send for the scene's work, with pages of made-up art.
    func playFixture(work: String, scene arguments: [String]) {
        guard let at = arguments.firstIndex(where: { $0 == "-KaeriScene" || $0 == "--scene" }) else { return }
        let scene = arguments[at + 1].lowercased()
        let strip = scene == "readerstrip"
        guard scene != "readernovel" else { return }
        let dir = FileManager.default.temporaryDirectory.appending(path: "kaeri-reader-shot", directoryHint: .isDirectory)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let count = strip ? 8 : 20
        let chapter = ChapterId(strip ? "2" : (scene == "readermoved" ? "14" : "13"))
        let session: UInt64 = 1
        let row = { (id: String, label: String, pages: UInt32?) in
            ChapterRow(
                id: ChapterId(id), label: label, state: .unread, number: nil, title: nil, volume: nil, published: nil,
                pages: pages, newChapter: false, download: nil, versions: 1, bookmarked: false)
        }
        let title = strip ? "Paper Kites Over Juniper Row" : "The Lantern Ferry"
        let here = row(chapter, "Chapter \(chapter)", UInt32(count))
        let place = ReadPlace(
            work: WorkId(work), chapter: chapter, page: strip ? nil : (scene == "readermoved" ? 4 : 8),
            strip: strip ? StripPos(image: 4, offset: 0.35) : nil, novel: nil)
        let rows: [ReaderSettingRow] = [
            ReaderSettingRow(key: .mode, value: .choice(name: strip ? "strip" : "paged"), scope: .series, detected: false, overridden: strip, choices: [.choice(name: "paged"), .choice(name: "strip")]),
            ReaderSettingRow(key: .direction, value: .choice(name: "rtl"), scope: .series, detected: true, overridden: false, choices: [.choice(name: "rtl"), .choice(name: "ltr")]),
            ReaderSettingRow(key: .look, value: .choice(name: "light"), scope: .medium, detected: false, overridden: false, choices: []),
            ReaderSettingRow(key: .fit, value: .choice(name: "screen"), scope: .medium, detected: false, overridden: false, choices: []),
            ReaderSettingRow(key: .cropBorders, value: .toggle(on: true), scope: .medium, detected: false, overridden: false, choices: []),
            ReaderSettingRow(key: .tapLayout, value: .choice(name: "edges"), scope: .device, detected: false, overridden: false, choices: []),
            ReaderSettingRow(key: .pageLayout, value: .choice(name: "auto"), scope: .medium, detected: false, overridden: false, choices: []),
            ReaderSettingRow(key: .paperMode, value: .toggle(on: false), scope: .device, detected: false, overridden: false, choices: []),
            ReaderSettingRow(key: .greyscalePages, value: .toggle(on: false), scope: .device, detected: false, overridden: false, choices: []),
            ReaderSettingRow(key: .stripWidth, value: .amount(value: 0.6), scope: .device, detected: false, overridden: false, choices: []),
        ]
        let settings = ReaderSettingsView(medium: .manga, work: WorkId(work), rows: rows)
        let opened = ReaderView(
            session: session, work: WorkId(work), title: title, medium: .manga, chapter: here,
            mode: strip ? .strip : .paged, direction: strip ? .topToBottom : .rightToLeft, start: place,
            next: row("next", "Chapter \((Int(String(chapter)) ?? 1) + 1)", 22), previous: nil, settings: settings)
        apply(.opened(opened))
        var leaves: [Leaf] = []
        for index in 0..<count {
            let url = dir.appending(path: "\(strip ? "s" : "p")\(index).png")
            drawPage(index: index, strip: strip, to: url)
            let file = PageFile(
                path: url.path, encoding: .original(mime: "image/png"), width: 800, height: strip ? 1400 : 1200,
                crop: nil, colour: true, bytes: 1)
            let part = LeafPart(page: UInt32(index), kind: .story, file: file, place: UnitRect(x: 0, y: 0, width: 1, height: 1), rotate: false, half: nil)
            leaves.append(Leaf(index: UInt32(index), parts: [part], endPage: false))
        }
        leaves.append(Leaf(index: UInt32(count), parts: [], endPage: true))
        apply(.leaves(session: session, chapter: chapter, leaves: leaves))
        for leaf in leaves where !leaf.endPage {
            if let part = leaf.parts.first, let file = part.file {
                apply(.pageReady(session: session, chapter: chapter, page: part.page, half: nil, file: file))
            }
        }
        if !strip { current = Int(place.page ?? 0) }
        position = (current, leaves.count)
        if scene == "readermoved" {
            let line = ReaderNoticeLine(id: bump(), text: L.readerMoved(chapter: "14", page: 5, device: "Kim's iPad"), action: .goBack)
            notice = line
        }
    }

    /// A page of made-up art: panels of flat colour on a pale ground, and its number.
    private func drawPage(index: Int, strip: Bool, to url: URL) {
        let width = 800, height = strip ? 1400 : 1200
        guard let context = CGContext(
            data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return }
        let hue = CGFloat((index * 37) % 100) / 100
        context.setFillColor(CGColor(red: 0.96, green: 0.94, blue: 0.9, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        let panels = strip ? 4 : 3
        let gap: CGFloat = 24
        let each = (CGFloat(height) - gap * CGFloat(panels + 1)) / CGFloat(panels)
        for panel in 0..<panels {
            let shade = hue + CGFloat(panel) * 0.11
            let r = 0.35 + 0.4 * abs(sin(shade * 6.28)), g = 0.45 + 0.35 * abs(sin(shade * 4.1)), b = 0.55 + 0.3 * abs(cos(shade * 5.3))
            context.setFillColor(CGColor(red: r, green: g, blue: b, alpha: 1))
            let rect = CGRect(x: gap, y: gap + CGFloat(panel) * (each + gap), width: CGFloat(width) - gap * 2, height: each)
            context.fill(rect)
            context.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 0.35))
            context.fillEllipse(in: CGRect(x: rect.midX - each * 0.25 + CGFloat(panel * 60), y: rect.midY - each * 0.25, width: each * 0.5, height: each * 0.5))
        }
        guard let image = context.makeImage(),
            let target = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)
        else { return }
        CGImageDestinationAddImage(target, image, nil)
        CGImageDestinationFinalize(target)
    }
}
#endif
