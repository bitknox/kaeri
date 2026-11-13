#if os(macOS)
import AppKit
import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import SwiftUI
import Testing
import WebKit

/// The novel reader's web view over a reading session: one made for the reader, kept while ten
/// chapters open and close and the screen is drawn again, and let go with the reader. A tap on a
/// page's side turns it in the page, which says so once. And two columns capped in a wide view
/// keep their usual gap and sit in the middle, on the first page and the last, with scroll bars
/// that take room as well. A turn past a chapter's end goes on into the next one, with the page
/// put away while the end page shows.
@MainActor
@Suite(.serialized)
struct NovelWebLifecycleTests {
    /// A made-up chapter, long enough for several pages.
    private static let page = """
    <?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>Lanterns</title></head><body>
    \(String(repeating: "<p>The lantern keeper climbed the stair and counted the lamps along the harbour wall, one by one.</p>", count: 120))
    </body></html>
    """

    private final class Sent { var commands: [Command] = [] }

    private func model(_ sent: Sent) -> ReaderModel {
        let model = ReaderModel()
        model.send = { sent.commands.append($0) }
        let page = Self.page
        model.bookResource = { _ in
            BookResource(status: 200, mime: "application/xhtml+xml", headers: [], body: .bytes(data: Data(page.utf8)))
        }
        return model
    }

    private func opened(_ model: ReaderModel, chapter n: Int, rtl: Bool = false) {
        let chapter = ChapterId("c\(n)")
        let row = ChapterRow(id: chapter, label: "\(n)", state: .unread)
        let next = ChapterRow(id: ChapterId("c\(n + 1)"), label: "\(n + 1)", state: .unread)
        let work = WorkId("a:900302")
        model.apply(.opened(ReaderView(
            session: 1, work: work, title: "The Glass Orchard", medium: .novel, chapter: row, mode: .paged,
            direction: rtl ? .rightToLeft : .leftToRight, start: ReadPlace(work: work, chapter: chapter), next: next)))
        let url = "kaeri-book://1/c\(n)/text.xhtml"
        model.apply(.novelOpen(NovelOpenView(
            session: 1, chapter: chapter, entry: url, writingMode: .horizontal, progression: rtl ? .rightToLeft : .leftToRight,
            support: .full, spine: [NovelSpineItem(url: url, linear: true)])))
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

    private func window(_ model: ReaderModel, size: CGSize = CGSize(width: 800, height: 600)) -> NSWindow {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        // The end page reads the app's store and navigator.
        window.contentView = NSHostingView(rootView: ReaderScreen(model: model) {}.frame(width: size.width, height: size.height)
            .environment(AppStore()).environment(Navigator()))
        window.setFrameOrigin(NSPoint(x: -20_000, y: -20_000))
        window.orderFrontRegardless()
        return window
    }

    @Test func tenChaptersKeepOneWebViewAndClosingLetsItGo() async throws {
        let sent = Sent()
        let model = model(sent)
        let made = NovelWebController.made
        model.open(work: "a:900302")
        let window = window(model)
        await spin(0.2)
        weak var first: WKWebView?
        for n in 1...10 {
            opened(model, chapter: n)
            #expect(await until(10) { model.position.count > 1 }, "chapter \(n) laid out")
            // The screen drawn again, as the chrome coming and going does.
            model.chromeShown.toggle()
            await spin(0.1)
            model.chromeShown.toggle()
            await spin(0.1)
            let view = try #require(window.contentView.flatMap(Self.webView(in:)))
            if first == nil { first = view }
            #expect(view === first, "chapter \(n) is in the same web view")
            #expect(NovelWebController.live == 1)
            if n < 10 {
                model.openChapter(ChapterId("c\(n + 1)"))
                model.position = (0, 1)
                await spin(0.1)
            }
        }
        #expect(NovelWebController.made - made == 1, "one web view for the whole reading")

        // The book closes before its window does: what the page held goes already.
        model.close()
        #expect(await until(5) { NovelWebController.live == 0 }, "the controller goes with the book")
        #expect(await until(5) { first == nil }, "and the web view with it")
        window.contentView = nil
        window.close()
    }

    @Test func aTapOnTheSideTurnsInThePageWithOneMessage() async throws {
        let sent = Sent()
        let model = model(sent)
        model.open(work: "a:900302")
        let window = window(model)
        defer {
            model.close()
            window.contentView = nil
            window.close()
        }
        await spin(0.2)
        opened(model, chapter: 1)
        #expect(await until(10) { model.position.count > 2 })
        let view = try #require(window.contentView.flatMap(Self.webView(in:)))
        let controller = try #require(view.navigationDelegate as? NovelWebController)
        // The page has heard which taps it turns on.
        _ = try await view.evaluateJavaScript("window.kaeriReader.input()")
        await spin(0.3)
        let heard = controller.heard
        let turns = sent.commands.count { if case .reader(.novelAt(_, _, _, _, _, .turn)) = $0 { true } else { false } }
        _ = try await view.evaluateJavaScript(
            "document.body.dispatchEvent(new MouseEvent('click', {clientX: 760, clientY: 300, bubbles: true})); 0")
        #expect(await until(5) { model.position.index == 1 }, "the next page")
        await spin(0.3)
        #expect(controller.heard - heard == 1, "one message: the page")
        let after = sent.commands.count { if case .reader(.novelAt(_, _, _, _, _, .turn)) = $0 { true } else { false } }
        #expect(after - turns == 1)
        #expect(!model.chromeShown)
        // The middle is the reader's: the chrome comes up, and the page stays.
        _ = try await view.evaluateJavaScript(
            "document.body.dispatchEvent(new MouseEvent('click', {clientX: 400, clientY: 300, bubbles: true})); 0")
        #expect(await until(5) { model.chromeShown })
        #expect(model.position.index == 1)
    }

    /// The chapters asked to open.
    private static func opens(_ sent: Sent) -> [String] {
        sent.commands.compactMap { if case .reader(.openChapter(_, let chapter)) = $0 { chapter } else { nil } }
    }

    private static func key(_ name: String) -> String {
        "document.dispatchEvent(new KeyboardEvent('keydown', {key: '\(name)', bubbles: true})); 0"
    }

    /// Past the last page a turn shows the end page, and one more goes on into the next chapter:
    /// a key the page turns on, a key while the end page holds the page, the reader's own turn.
    /// A turn back from the end page puts it away.
    @Test func aTurnPastTheLastPageGoesOnIntoTheNextChapter() async throws {
        let sent = Sent()
        let model = model(sent)
        model.open(work: "a:900302")
        let window = window(model)
        defer {
            model.close()
            window.contentView = nil
            window.close()
        }
        await spin(0.2)
        opened(model, chapter: 1)
        #expect(await until(10) { model.position.count > 2 })
        let view = try #require(window.contentView.flatMap(Self.webView(in:)))
        let controller = try #require(view.navigationDelegate as? NovelWebController)
        _ = try await view.evaluateJavaScript("window.kaeriReader.goTo(window.kaeriReader.state().pages - 1, 'jump'); 0")
        #expect(await until(5) { model.position.index == model.position.count - 1 }, "the last page")

        _ = try await view.evaluateJavaScript(Self.key("ArrowRight"))
        #expect(await until(5) { controller.shown.end }, "the end page")
        // The web view draws over the end page on the Mac: the page goes while it shows.
        #expect(await until(5) { view.alphaValue == 0 }, "the page under the end page: \(view.alphaValue)")
        model.requestTurn(forward: false)
        #expect(await until(5) { !controller.shown.end }, "back to the last page")
        #expect(await until(5) { view.alphaValue == 1 }, "the last page shown again: \(view.alphaValue)")
        model.requestTurn(forward: true)
        #expect(await until(5) { controller.shown.end }, "the end page again")
        #expect(Self.opens(sent).isEmpty)
        _ = try await view.evaluateJavaScript(Self.key("PageDown"))
        #expect(await until(5) { Self.opens(sent) == ["c2"] }, "on into the next chapter: \(Self.opens(sent))")
    }

    /// A scrolled chapter read right to left: a turn at its foot shows the end page, and a tap
    /// on the end page's forward side, the left, goes on.
    @Test func aScrolledChapterGoesOnFromItsFoot() async throws {
        let sent = Sent()
        let model = model(sent)
        model.open(work: "a:900302")
        let window = window(model)
        defer {
            model.close()
            window.contentView = nil
            window.close()
        }
        await spin(0.2)
        opened(model, chapter: 1, rtl: true)
        #expect(await until(10) { model.position.count > 2 })
        let view = try #require(window.contentView.flatMap(Self.webView(in:)))
        let controller = try #require(view.navigationDelegate as? NovelWebController)
        _ = try await view.evaluateJavaScript("window.kaeriReader.configure({mode: 'scrolled'}); 0")
        await spin(0.3)
        let atFoot = "(function () { var s = document.scrollingElement; s.scrollTop = s.scrollHeight; return s.scrollTop > 0; })()"
        #expect(try await view.evaluateJavaScript(atFoot) as? Bool == true, "the chapter scrolls")
        await spin(0.4)
        _ = try await view.evaluateJavaScript(Self.key("ArrowLeft"))
        #expect(await until(5) { controller.shown.end }, "the end page from the foot")
        controller.tapped(0.9)
        #expect(await until(5) { !controller.shown.end }, "the back side puts it away")
        model.requestTurn(forward: true)
        #expect(await until(5) { controller.shown.end })
        controller.tapped(0.1)
        #expect(await until(5) { Self.opens(sent) == ["c2"] }, "on into the next chapter: \(Self.opens(sent))")
    }

    /// The text's columns on the page shown: the left one's edges and the right one's.
    private static let columns = """
    (function () {
      var vw = document.documentElement.clientWidth, rs = [], r = document.createRange(), n;
      var w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
      while ((n = w.nextNode())) {
        r.selectNodeContents(n);
        var bs = r.getClientRects();
        for (var i = 0; i < bs.length; i++) {
          var b = bs[i];
          if (!(b.width > 0 && b.left >= 0 && b.right <= vw)) continue;
          // Only what shows: a neighbouring page's line under the outer margin does not.
          var e = document.elementFromPoint((b.left + b.right) / 2, (b.top + b.bottom) / 2);
          if (e && e !== document.documentElement && (e.contains(n.parentElement) || n.parentElement.contains(e))) rs.push(b);
        }
      }
      var a = rs.filter(function (b) { return b.left < vw / 2; }), b = rs.filter(function (b) { return b.left >= vw / 2; });
      var lo = function (xs) { return Math.min.apply(null, xs.map(function (x) { return x.left; })); };
      var hi = function (xs) { return Math.max.apply(null, xs.map(function (x) { return x.right; })); };
      return [vw, lo(a), hi(a), lo(b), hi(b)];
    })()
    """

    /// Scroll bars that take room, as a Mac with a mouse and Windows draw them, whatever the
    /// machine running the tests has set. Before the paginator's first layout a long chapter is
    /// one tall column, so the view has a vertical scroll bar until the page hides the overflow.
    private static let scrollBars = """
    (function () {
      var s = document.createElementNS('http://www.w3.org/1999/xhtml', 'style');
      s.textContent = '::-webkit-scrollbar { width: 17px; height: 17px; background: #888; }';
      (document.head || document.documentElement).appendChild(s);
      // What the scroll bar takes from the view before the paginator lays the page out.
      window.kaeriTestScrollBar = window.innerWidth - document.documentElement.clientWidth;
    })();
    """

    @Test func twoCappedColumnsKeepTheirGapAndSitInTheMiddle() async throws {
        try await twoCappedColumns(scrollBars: false)
    }

    @Test func twoCappedColumnsSitInTheMiddleWithScrollBarsThatTakeRoom() async throws {
        try await twoCappedColumns(scrollBars: true)
    }

    private func twoCappedColumns(scrollBars: Bool) async throws {
        let model = model(Sent())
        model.open(work: "a:900302")
        let window = window(model, size: CGSize(width: 1920, height: 1080))
        defer {
            model.close()
            window.contentView = nil
            window.close()
        }
        await spin(0.2)
        opened(model, chapter: 1)
        #expect(await until(10) { model.position.count > 2 })
        let view = try #require(window.contentView.flatMap(Self.webView(in:)))
        if scrollBars {
            // The chapter again, laid out from the start with the scroll bars in.
            view.configuration.userContentController.addUserScript(
                WKUserScript(source: Self.scrollBars, injectionTime: .atDocumentEnd, forMainFrameOnly: true))
            model.position = (0, 1)
            view.reload()
            #expect(await until(10) { model.position.count > 2 }, "laid out again")
            let bar = try await view.evaluateJavaScript("window.kaeriTestScrollBar") as? Double
            #expect(bar == 17, "the scroll bar took room before the layout: \(bar ?? -1)")
        }
        await spin(0.5)
        let first = try #require(try await view.evaluateJavaScript(Self.columns) as? [Double])
        let (width, left, leftEnd, right, rightEnd) = (first[0], first[1], first[2], first[3], first[4])
        #expect(width > 1500, "a wide view: \(first)")
        // The paginator's default margin is 28 px, so the gap between columns is 56.
        #expect(abs((right - leftEnd) - 56) <= 4, "the usual gap: \(first)")
        #expect(abs(left - (width - rightEnd)) <= 4, "in the middle: \(first)")
        #expect(left > 100, "the spare width in the outer margins: \(first)")
        // The last page is reached exactly, its columns where the first page's are.
        _ = try await view.evaluateJavaScript("window.kaeriReader.goTo(window.kaeriReader.state().pages - 1, 'jump'); 0")
        await spin(0.5)
        let last = try #require(try await view.evaluateJavaScript(Self.columns) as? [Double])
        #expect(abs(last[1] - left) <= 4, "the last page lines up: \(last)")
        #expect(abs(last[1] - (last[0] - last[4])) <= 4, "the last page in the middle: \(last)")
    }

    private static func webView(in view: NSView) -> WKWebView? {
        if let web = view as? WKWebView { return web }
        for child in view.subviews {
            if let web = webView(in: child) { return web }
        }
        return nil
    }
}
#endif
