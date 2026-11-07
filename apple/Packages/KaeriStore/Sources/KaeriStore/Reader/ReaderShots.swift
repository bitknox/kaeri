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
    /// `-KaeriReader chrome|settings|chapters|ask|end|scrub|plate|opening|error` after a reader scene.
    public static let argument = "-KaeriReader"

    /// The work a reader scene reads.
    public static func shotWork(arguments: [String]) -> String? {
        guard let at = arguments.firstIndex(where: { $0 == "-KaeriScene" || $0 == "--scene" }), arguments.indices.contains(at + 1) else { return nil }
        switch arguments[at + 1].lowercased() {
        case "readermangartl", "readermoved": return "a:900301"
        case "readernovel": return "a:900302"
        case "readerstrip": return "a:900303"
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
        // A novel is shot once its paginator has put a page on screen.
        while novel != nil, !novelPainted, ContinuousClock.now < deadline {
            try? await Task.sleep(for: .milliseconds(100))
        }
        try? await Task.sleep(for: .milliseconds(novel != nil ? 2500 : 1500))
        let shown = phase == .open
        if let at = arguments.firstIndex(of: Self.argument), arguments.indices.contains(at + 1) {
            switch arguments[at + 1] {
            case "chrome": chromeShown = true
            case "settings": chromeShown = true; sheet = .settings
            case "chapters": chromeShown = true; sheet = .chapters
            case "ask": chromeShown = true; askingPage = true
            case "end": requestJump(max(leaves.count - 1, 0))
            case "scrub": chromeShown = true; shotScrub = max(leaves.count / 4, 1)
            case "pill": backTo = ReaderNoticeLine(id: bump(), text: L.readerBackToPage(page: 9), action: .backTo)
            case "opening": hold(.opening)
            case "error": hold(.failed(.offline))
            default: break
            }
            try? await Task.sleep(for: .milliseconds(900))
        }
        _ = shown
        return true
    }
}

extension ReaderModel {
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
            let back = ReaderNoticeLine(id: bump(), text: L.readerBackToPage(page: 9), action: .backTo)
            backTo = back
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
