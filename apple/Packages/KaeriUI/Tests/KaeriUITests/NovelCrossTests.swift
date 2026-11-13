#if os(macOS)
import AppKit
import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import Synchronization
import SwiftUI
import Testing
import WebKit

extension NovelWebLifecycleTests {
    /// A long book's turn from the last page of one part into the next, as the engine serves it:
    /// only the open session's addresses answer, and each part carries its spine index in its
    /// settings. The same book opened again in a new session crosses into its own next part, a part
    /// that does not come leaves the page and its keys where they were and keeps no place, and
    /// parts with spaces in their names cross both ways. The last page of a part sits on the page
    /// grid like the others.
    ///
    /// It sits in the novel lifecycle suite, which is serialized, so that no two novel web views
    /// are open at once: the lifecycle tests count the controllers made and live.
    @MainActor
    @Suite
    struct NovelCrossTests {
        /// A made-up book of `parts` long parts, served for one session at a time, as the engine
        /// does: any other session's address, and a part listed as missing, answer 404 with an
        /// empty body.
        final class Book: Sendable {
            let parts: Int
            let spaces: Bool
            /// Notes in a file of their own outside the spine, linked from the first part.
            let notes: Bool
            /// Top to bottom and right to left, in Japanese.
            let vertical: Bool
            /// Paragraphs in each part, 60 where it says none.
            let paragraphs: [Int]
            private let state = Mutex((session: UInt64(1), missing: Set<Int>()))

            init(parts: Int, spaces: Bool = false, notes: Bool = false, vertical: Bool = false, paragraphs: [Int] = [],
                 missing: Set<Int> = []) {
                self.parts = parts
                self.spaces = spaces
                self.notes = notes
                self.vertical = vertical
                self.paragraphs = paragraphs
                state.withLock { $0.missing = missing }
            }

            var session: UInt64 {
                get { state.withLock { $0.session } }
                set { state.withLock { $0.session = newValue } }
            }

            func name(_ part: Int) -> String {
                spaces ? "Text/Part%20\(part + 1)%20of%20the%20Glass%20Orchard.xhtml" : "OEBPS/ch\(part + 1).xhtml"
            }

            func url(_ part: Int, session: UInt64) -> String { "kaeri-book://s\(session)/\(name(part))" }

            func notesURL(session: UInt64) -> String { "kaeri-book://s\(session)/OEBPS/notes.xhtml" }

            func spine(session: UInt64) -> [NovelSpineItem] {
                (0..<parts).map { NovelSpineItem(url: url($0, session: session), linear: true) }
            }

            /// The document an address names, without its fragment and its escapes.
            static func document(_ address: String) -> String {
                let path = String(address.split(separator: "#", maxSplits: 1, omittingEmptySubsequences: false).first ?? "")
                return path.removingPercentEncoding ?? path
            }

            func resource(_ address: String) -> BookResource {
                let (session, missing) = state.withLock { ($0.session, $0.missing) }
                let path = Self.document(address)
                if notes, path == Self.document(notesURL(session: session)) {
                    // The engine gives a file outside the spine the first index in its settings.
                    let text = """
                    <?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head>\
                    <meta name="kaeri-setup" content="{&quot;spine&quot;:0}"/><title>Notes</title></head><body>\
                    <p id="n1">1. The bell was cast from the harbour's old kettles.</p></body></html>
                    """
                    return BookResource(status: 200, mime: "application/xhtml+xml", headers: [], body: .bytes(data: Data(text.utf8)))
                }
                guard let part = (0..<parts).first(where: { Self.document(url($0, session: session)) == path }),
                      !missing.contains(part)
                else {
                    return BookResource(status: 404, mime: "text/plain", headers: [], body: .bytes(data: Data()))
                }
                let setup = "{&quot;spine&quot;:\(part),&quot;last&quot;:\(part == parts - 1)}"
                let words = vertical ? "渡し守は銅の鐘を鳴らし、静かな港の壁に沿って灯りを数えた。"
                    : "The ferry keeper rang the copper bell and counted the lanterns along the quiet harbour wall."
                let lang = vertical ? " xml:lang=\"ja\" lang=\"ja\"" : ""
                let body = vertical ? " style=\"writing-mode: vertical-rl\"" : ""
                let count = part < paragraphs.count ? paragraphs[part] : 60
                let text = """
                <?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml"\(lang)><head>\
                <meta name="kaeri-setup" content="\(setup)"/><title>Part \(part + 1)</title></head><body\(body)>\
                <h1>Part \(part + 1)</h1>\(notes && part == 0 ? "<p><a id=\"ref\" href=\"notes.xhtml#n1\">1</a></p>" : "")\(String(repeating: "<p>\(words) \(words)</p>", count: count))</body></html>
                """
                return BookResource(status: 200, mime: "application/xhtml+xml", headers: [], body: .bytes(data: Data(text.utf8)))
            }
        }

        private final class Sent { var commands: [Command] = [] }

        private static let workId = "x:900417"
        private static let work = WorkId(workId)
        private static let chapter = ChapterId("x/-/the glass orchard")

        private func model(_ sent: Sent, _ book: Book) -> ReaderModel {
            let model = ReaderModel()
            model.send = { sent.commands.append($0) }
            model.bookResource = { book.resource($0) }
            return model
        }

        /// The engine opened the book in `session`, at its first part.
        private func opened(_ model: ReaderModel, _ book: Book, session: UInt64) {
            let row = ChapterRow(id: Self.chapter, label: "1", state: .unread)
            let next = ChapterRow(id: ChapterId("x/-/the glass orchard 2"), label: "2", state: .unread)
            model.apply(.opened(ReaderView(
                session: session, work: Self.work, title: "The Glass Orchard", medium: .novel, chapter: row, mode: .paged,
                direction: .leftToRight, start: ReadPlace(work: Self.work, chapter: Self.chapter), next: next)))
            model.apply(.novelOpen(NovelOpenView(
                session: session, chapter: Self.chapter, entry: book.url(0, session: session),
                writingMode: book.vertical ? .verticalRightToLeft : .horizontal, progression: book.vertical ? .rightToLeft : .leftToRight, support: .full, spine: book.spine(session: session))))
        }

        /// The places the reader reported to the engine.
        private static func places(_ sent: Sent) -> [(spine: UInt32, page: UInt32, pages: UInt32)] {
            sent.commands.compactMap {
                if case .reader(.novelAt(_, _, let pos, let page, let pages, _)) = $0 { (pos.spine, page, pages) } else { nil }
            }
        }

        private func spin(_ seconds: Double) async {
            let end = Date().addingTimeInterval(seconds)
            while Date() < end {
                Self.pump()
                try? await Task.sleep(for: .milliseconds(1))
            }
        }

        /// The main run loop turned for a moment, for the web view's callbacks.
        private static func pump() { RunLoop.main.run(until: Date().addingTimeInterval(0.01)) }

        private func until(_ seconds: Double, _ done: () -> Bool) async -> Bool {
            let end = Date().addingTimeInterval(seconds)
            while !done(), Date() < end { await spin(0.05) }
            return done()
        }

        private func window(_ model: ReaderModel, _ size: CGSize = CGSize(width: 800, height: 600)) -> NSWindow {
            _ = NSApplication.shared
            let window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.borderless], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            window.contentView = NSHostingView(rootView: ReaderScreen(model: model) {}.frame(width: size.width, height: size.height)
                .environment(AppStore()).environment(Navigator()))
            window.setFrameOrigin(NSPoint(x: -20_000, y: -20_000))
            window.orderFrontRegardless()
            return window
        }

        private static func webView(in view: NSView) -> WKWebView? {
            if let web = view as? WKWebView { return web }
            for child in view.subviews {
                if let web = webView(in: child) { return web }
            }
            return nil
        }

        private func shown(_ window: NSWindow) -> String? {
            window.contentView.flatMap(Self.webView(in:))?.url?.absoluteString
        }

        /// The document the page has laid out and its pages, read from the page itself.
        private func laidOut(_ window: NSWindow) async -> (address: String, pages: Int)? {
            guard let view = window.contentView.flatMap(Self.webView(in:)) else { return nil }
            let value = try? await view.evaluateJavaScript(
                "window.kaeriReader ? window.kaeriReader.count() + ' ' + location.href : ''")
            guard let text = value as? String, let space = text.firstIndex(of: " "), let pages = Int(text[..<space])
            else { return nil }
            return (String(text[text.index(after: space)...]), pages)
        }

        /// End, then a turn forward, as the Mac's keys ask.
        private func endThenRight(_ model: ReaderModel) async {
            model.requestJump(model.position.count - 1)
            #expect(await until(5) { model.position.index == model.position.count - 1 }, "the part's last page")
            await spin(0.3)
            model.requestTurn(forward: true)
        }

        /// The book read from the start while it was open: the engine's new session serves the
        /// book at new addresses and no longer answers the old ones. The turn from the last page of
        /// part 1 shows the first page of part 2, the keys go on turning, and no place is kept from
        /// anything but the book's own pages.
        @Test func aBookOpenedAgainCrossesIntoItsNextPart() async throws {
            let book = Book(parts: 40)
            let sent = Sent()
            let model = model(sent, book)
            model.open(work: Self.workId)
            let window = window(model)
            defer {
                model.close()
                window.contentView = nil
                window.close()
            }
            await spin(0.2)
            opened(model, book, session: 1)
            #expect(await until(10) { model.position.count > 2 }, "part 1 laid out")

            // Read from the start, over the open book.
            model.open(work: Self.workId, at: .start)
            model.apply(.closed(session: 1))
            book.session = 2
            await spin(0.2)
            model.position = (0, 1)
            opened(model, book, session: 2)
            // The page itself says which document it laid out: until the new session's has, a
            // count and a turn may still come from the old page.
            let entry = book.url(0, session: 2)
            var page = await laidOut(window)
            let deadline = Date().addingTimeInterval(10)
            while !(page?.address.hasPrefix(entry) == true && (page?.pages ?? 0) > 2), Date() < deadline {
                await spin(0.05)
                page = await laidOut(window)
            }
            #expect(page?.address.hasPrefix(entry) == true && (page?.pages ?? 0) > 2,
                    "the book laid out again: \(String(describing: page))")
            #expect(await until(5) { model.position.count > 2 }, "and its pages counted")
            await spin(0.3)

            let before = Self.places(sent).count
            await endThenRight(model)
            let second = book.url(1, session: 2)
            #expect(await until(10) { shown(window) == second && model.position.count > 2 && model.position.index == 0 },
                    "part 2's first page: \(shown(window) ?? "-") at \(model.position)")
            model.requestTurn(forward: true)
            #expect(await until(5) { model.position.index == 1 }, "the keys turn on: \(model.position)")
            let after = Self.places(sent).dropFirst(before)
            #expect(after.allSatisfy { $0.pages > 1 }, "no place from an empty page: \(after)")
            #expect(after.last?.spine == 1, "part 2 kept: \(after)")
        }

        /// Part 2 does not come: the last page of part 1 stays with its keys, and no place is kept
        /// from the empty answer.
        @Test func aPartThatDoesNotComeLeavesThePageAndKeepsNoPlace() async throws {
            let book = Book(parts: 40, missing: [1])
            let sent = Sent()
            let model = model(sent, book)
            model.open(work: Self.workId)
            let window = window(model)
            defer {
                model.close()
                window.contentView = nil
                window.close()
            }
            await spin(0.2)
            opened(model, book, session: 1)
            #expect(await until(10) { model.position.count > 2 }, "part 1 laid out")

            let before = Self.places(sent).count
            await endThenRight(model)
            await spin(3)
            let first = book.url(0, session: 1)
            let count = model.position.count
            #expect(shown(window)?.hasPrefix(first) == true, "still part 1: \(shown(window) ?? "-")")
            #expect(count > 2 && model.position.index == count - 1, "its last page: \(model.position)")
            let after = Self.places(sent).dropFirst(before)
            #expect(after.allSatisfy { $0.spine == 0 && $0.pages > 1 }, "no place from the empty answer: \(after)")

            model.requestTurn(forward: false)
            #expect(await until(5) { model.position.index == count - 2 }, "the keys still turn: \(model.position)")
        }

        /// A note in a file outside the spine: its link opens it, it keeps no reading place, and a
        /// turn back from it shows the page the link was followed from.
        @Test func aNoteOutsideTheSpineOpensAndTurnsBackToThePlace() async throws {
            let book = Book(parts: 3, notes: true)
            let sent = Sent()
            let model = model(sent, book)
            model.open(work: Self.workId)
            let window = window(model)
            defer {
                model.close()
                window.contentView = nil
                window.close()
            }
            await spin(0.2)
            opened(model, book, session: 1)
            #expect(await until(10) { model.position.count > 2 }, "part 1 laid out")
            model.requestTurn(forward: true)
            #expect(await until(5) { model.position.index == 1 }, "page 2")
            await spin(0.3)
            let count = model.position.count
            let before = Self.places(sent).count

            let view = try #require(window.contentView.flatMap(Self.webView(in:)))
            _ = try await view.evaluateJavaScript("document.getElementById('ref').click(); 0")
            let notes = book.notesURL(session: 1)
            #expect(await until(10) { shown(window)?.hasPrefix(notes) == true }, "the notes: \(shown(window) ?? "-")")
            await spin(1)
            #expect(Self.places(sent).count == before, "no place kept from the notes: \(Self.places(sent).dropFirst(before))")

            model.requestTurn(forward: false)
            let first = book.url(0, session: 1)
            #expect(await until(10) {
                shown(window)?.hasPrefix(first) == true && model.position.count == count && model.position.index == 1
            }, "back on page 2 of part 1: \(shown(window) ?? "-") at \(model.position)")
            let after = Self.places(sent).dropFirst(before)
            #expect(after.allSatisfy { $0.spine == 0 && $0.pages > 1 }, "places from part 1 only: \(after)")
        }

        /// Parts with spaces in their names, escaped as the engine writes them: a turn crosses into
        /// part 2, and a turn back from its first page shows part 1's last page.
        @Test func partsWithSpacesInTheirNamesCrossBothWays() async throws {
            let book = Book(parts: 40, spaces: true)
            let sent = Sent()
            let model = model(sent, book)
            model.open(work: Self.workId)
            let window = window(model)
            defer {
                model.close()
                window.contentView = nil
                window.close()
            }
            await spin(0.2)
            opened(model, book, session: 1)
            #expect(await until(10) { model.position.count > 2 }, "part 1 laid out")

            await endThenRight(model)
            let second = book.url(1, session: 1)
            #expect(await until(10) { shown(window) == second && model.position.count > 2 && model.position.index == 0 },
                    "part 2's first page: \(shown(window) ?? "-") at \(model.position)")
            await spin(0.3)
            model.requestTurn(forward: false)
            let first = book.url(0, session: 1)
            #expect(await until(10) {
                shown(window)?.hasPrefix(first) == true && model.position.count > 2
                    && model.position.index == model.position.count - 1
            }, "part 1's last page: \(shown(window) ?? "-") at \(model.position)")
            #expect(Self.places(sent).allSatisfy { $0.pages > 1 }, "every place from a laid-out page")
        }

        /// Where the page stands against where its number puts it, along the reading axis.
        private struct Grid: Decodable {
            let page: Int
            let pages: Int
            /// The scroll along the reading axis, and the page's length along it.
            let at: Double
            let pitch: Double
            var off: Double { at - Double(page) * pitch }
        }

        private func grid(_ view: WKWebView) async throws -> Grid {
            let js = """
            (function () {
              var s = kaeriReader.state(), h = document.documentElement, v = h.classList.contains('k-v');
              return JSON.stringify({ page: s.page, pages: s.pages, at: v ? window.scrollY : Math.abs(window.scrollX),
                pitch: v ? h.clientHeight : parseFloat(getComputedStyle(h).width) });
            })()
            """
            let value = try await view.evaluateJavaScript(js)
            let text = try #require(value as? String)
            return try JSONDecoder().decode(Grid.self, from: Data(text.utf8))
        }

        /// Each part's last page scrolled to where its number puts it, as every other page is, at the
        /// size of a Mac's window: parts of different lengths end on a full spread or on one
        /// column, and a page short of its place would show the spread before it again.
        private func lastPagesOnTheGrid(_ book: Book, _ size: CGSize) async throws {
            let sent = Sent()
            let model = model(sent, book)
            model.open(work: Self.workId)
            let window = window(model, size)
            defer {
                model.close()
                window.contentView = nil
                window.close()
            }
            await spin(0.2)
            opened(model, book, session: 1)
            for part in 0..<book.parts {
                let address = book.url(part, session: 1)
                #expect(await until(10) { shown(window)?.hasPrefix(address) == true && model.position.count > 1 },
                        "part \(part + 1) laid out: \(shown(window) ?? "-") at \(model.position)")
                await spin(0.3)
                let view = try #require(window.contentView.flatMap(Self.webView(in:)))
                let middle = try await grid(view)
                #expect(abs(middle.off) < 1, "part \(part + 1)'s first page on the grid: \(middle)")
                model.requestJump(model.position.count - 1)
                #expect(await until(5) { model.position.index == model.position.count - 1 }, "part \(part + 1)'s last page")
                await spin(0.5)
                let last = try await grid(view)
                #expect(last.page == last.pages - 1 && abs(last.off) < 1, "part \(part + 1)'s last page on the grid: \(last)")
                if part < book.parts - 1 { model.requestTurn(forward: true) }
            }
        }

        @Test func theLastPageOfAPartSitsOnTheGrid() async throws {
            try await lastPagesOnTheGrid(Book(parts: 4, paragraphs: [30, 33, 36, 39]), CGSize(width: 1280, height: 794))
        }

        @Test func theLastPageOfAVerticalPartSitsOnTheGrid() async throws {
            try await lastPagesOnTheGrid(Book(parts: 3, vertical: true, paragraphs: [24, 27, 30]), CGSize(width: 1280, height: 794))
        }
    }
}
#endif
