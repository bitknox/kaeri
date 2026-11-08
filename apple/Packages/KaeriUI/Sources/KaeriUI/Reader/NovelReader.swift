// A novel in a web view. The engine serves the book's documents on the kaeri-book scheme, the
// paginator lays them out in pages and reports the place, and the host moves between documents.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import Synchronization
import SwiftUI
import WebKit

struct NovelReader: View {
    @Bindable var model: ReaderModel
    let theme: ReaderTheme
    let mac: Bool
    let size: CGSize
    let rtl: Bool
    /// The reader's web view, which the reader's screen keeps from chapter to chapter.
    let web: NovelWebSession
    /// A tap the page did not turn on. It must not hold this view or its screen: the web view
    /// keeps it for as long as the reader is open.
    let onTap: (Double) -> Void
    @Environment(\.readerPaper) private var paper

    var body: some View {
        let controller = web.controller
        let shown = controller.shown
        ZStack {
            NovelWebView(controller: controller)
                .frame(width: size.width, height: size.height)
            if !shown.end, model.notice == nil, model.choice(.footValue) != "none", model.choice(.mode) != "scrolled" {
                pagesLeft
            }
            if shown.end {
                ReaderEndPage(model: model, theme: theme, size: size).transition(.opacity)
            }
            if let note = shown.note {
                VStack {
                    Spacer()
                    Text(note).font(.ui(15)).foregroundStyle(theme.silk).lineLimit(8)
                        .padding(16).frame(maxWidth: 520, alignment: .leading)
                        .glassEffect(.regular, in: .rect(cornerRadius: 20))
                        .padding(.bottom, 120).padding(.horizontal, 16)
                        .onTapGesture { shown.note = nil }
                }
                .transition(.opacity)
            }
        }
        // Reduce Motion keeps the fades, which move nothing; Paper mode has none.
        .animation(paper ? nil : .fadeIn, value: shown.end)
        .animation(paper ? nil : .fadeIn, value: shown.note)
        .animation(paper ? nil : .fadeIn, value: model.notice == nil)
        .onAppear {
            controller.attach(model, onTap: onTap)
            open()
        }
        .onChange(of: theme.page, initial: true) { _, page in controller.pageColor = page }
        .onChange(of: paper, initial: true) { _, paper in controller.paper = paper }
        .onChange(of: model.novel?.chapter) { _, _ in open() }
        .onChange(of: model.novelStyle) { _, style in controller.configure(style) }
        .onChange(of: model.jump?.tick) { _, _ in
            if let index = model.jump?.index { controller.goTo(index) }
        }
        .onChange(of: model.turnRequest) { _, request in
            guard let request else { return }
            if shown.end, !request.forward { shown.end = false; return }
            controller.turn(forward: request.forward, how: "key")
        }
        .onChange(of: model.choice(.mode)) { _, mode in controller.scrolled(mode == "scrolled") }
        // What the page turns on by itself follows the chrome and the tap setting.
        .onChange(of: model.chromeShown) { _, _ in controller.sendInput() }
        .onChange(of: model.choice(.tapLayout)) { _, _ in controller.sendInput() }
        .onDisappear { controller.stop() }
    }

    /// "6 pages left in this chapter", the one line at rest, in the bottom margin. A notice takes
    /// its place while it shows.
    private var pagesLeft: some View {
        let left = max(model.position.count - model.position.index - 1, 0)
        return VStack {
            Spacer()
            Text(L.readerPagesLeft(count: left))
                .font(.ui(12)).monospacedDigit()
                .foregroundStyle(theme.silk.opacity(0.6))
                .lineLimit(1).minimumScaleFactor(0.8)
                .padding(.bottom, mac ? 48 : (size.width >= 700 ? 30 : 28))
        }
        .frame(width: size.width)
        .allowsHitTesting(false)
        .opacity(model.chromeShown ? 0 : 1)
        .transition(.opacity)
        .accessibilityHidden(true)
    }

    private func open() {
        guard let book = model.novel else { return }
        web.controller.open(book, scrolled: model.choice(.mode) == "scrolled")
    }
}

// MARK: Session

/// The novel web view of one open reader: made the first time a novel shows, kept while
/// chapters come and go, and let go with everything it held when the reader closes. The
/// reader's screen keeps it in its state, which SwiftUI may build again on any redraw, so
/// making one costs nothing until a novel asks for its view.
@MainActor
final class NovelWebSession {
    private var made: NovelWebController?

    nonisolated init() {}

    var controller: NovelWebController {
        if let made { return made }
        let controller = NovelWebController()
        made = controller
        return controller
    }

    /// The reader closed: the page, its process and what it kept go.
    func close() {
        made?.close()
        made = nil
    }
}

/// What the page shows over itself: the end page and a footnote.
@MainActor @Observable
final class NovelOverlay {
    var end = false {
        didSet { if end != oldValue { changed() } }
    }
    var note: String?
    @ObservationIgnored var changed: () -> Void = {}
}

// MARK: Controller

@MainActor
final class NovelWebController: NSObject, WKNavigationDelegate, WKScriptMessageHandler {
    /// Controllers alive now and made in this run, for the test that a reader keeps one from
    /// chapter to chapter and lets it go when it closes.
    static var live: Int { counter.withLock { $0.live } }
    static var made: Int { counter.withLock { $0.made } }
    private nonisolated static let counter = Mutex((live: 0, made: 0))
    /// Messages heard from the page, for the test that a turn is one.
    private(set) var heard = 0

    let webView: WKWebView
    let shown = NovelOverlay()
    private weak var model: ReaderModel?
    private var onTap: (Double) -> Void = { _ in }
    private var book: NovelOpenView?
    private var openedChapter: String?
    /// The place the page last said, to open again at after WebKit ended the page's process.
    private var lastPlace: (spine: Int, pos: NovelPos)?
    /// What the page last heard of input(): sent again with each new document.
    private var sentInput: String?
    private let handler = BookSchemeHandler()
    /// The page's colour, behind the page a drag carries away.
    var pageColor: Color = .clear
    /// Paper mode: a swipe turns at once, with nothing following the finger.
    var paper = false
    #if os(iOS)
    /// The page being dragged away, over the one the turn showed under it.
    private var carried: UIView?
    private var carriedForward = true
    #endif

    override init() {
        let config = WKWebViewConfiguration()
        // A book's pages keep nothing on disk; the engine serves every file, and only the web
        // view's memory cache keeps a book's fonts and stylesheets while it is open.
        config.websiteDataStore = .nonPersistent()
        let assets = novelAssets()
        let script = WKUserScript(source: assets.script, injectionTime: .atDocumentStart, forMainFrameOnly: true)
        config.userContentController.addUserScript(script)
        config.setURLSchemeHandler(handler, forURLScheme: "kaeri-book")
        webView = WKWebView(frame: .zero, configuration: config)
        super.init()
        Self.counter.withLock {
            $0.live += 1
            $0.made += 1
        }
        config.userContentController.add(ScriptProxy(self), name: "kaeri")
        webView.navigationDelegate = self
        shown.changed = { [weak self] in self?.sendInput() }
        #if os(iOS)
        webView.isOpaque = false
        webView.backgroundColor = .clear
        webView.scrollView.backgroundColor = .clear
        webView.scrollView.contentInsetAdjustmentBehavior = .never
        webView.scrollView.isScrollEnabled = false
        webView.allowsLinkPreview = false
        let pan = UIPanGestureRecognizer(target: self, action: #selector(panned(_:)))
        pan.delegate = self
        webView.addGestureRecognizer(pan)
        #else
        webView.setValue(false, forKey: "drawsBackground")
        #endif
        webView.underPageBackgroundColor = .clear
    }

    deinit {
        Self.counter.withLock { $0.live -= 1 }
    }

    /// The reader and what a tap off the turning sides does: kept weakly and replaced, so
    /// nothing here holds a view.
    func attach(_ model: ReaderModel, onTap: @escaping (Double) -> Void) {
        self.model = model
        self.onTap = onTap
    }

    func stop() {
        webView.stopLoading()
    }

    /// Let go of everything: the page, its scripts' bridge, the book's server and the reader.
    func close() {
        webView.stopLoading()
        webView.navigationDelegate = nil
        webView.configuration.userContentController.removeAllScriptMessageHandlers()
        webView.configuration.userContentController.removeAllUserScripts()
        webView.removeFromSuperview()
        handler.resource = nil
        onTap = { _ in }
        shown.changed = {}
        model = nil
        book = nil
    }

    func open(_ book: NovelOpenView, scrolled: Bool) {
        guard openedChapter != book.chapter else { return }
        // A view that showed another chapter stays hidden until the new one is laid out, so
        // the last page of the old one never shows under the new one.
        if openedChapter != nil { setHidden(true) }
        openedChapter = book.chapter
        self.book = book
        lastPlace = nil
        shown.end = false
        shown.note = nil
        handler.resource = model?.bookResource
        self.scrolled(scrolled)
        var url = book.entry
        if let at = book.restore, book.spine.indices.contains(Int(at.spine)) {
            url = book.spine[Int(at.spine)].url + "#kaeri:pos=" + Self.text(at)
        }
        if let target = URL(string: url) { webView.load(URLRequest(url: target)) }
    }

    private func setHidden(_ hidden: Bool) {
        #if os(iOS)
        webView.alpha = hidden ? 0 : 1
        #else
        webView.alphaValue = hidden ? 0 : 1
        #endif
    }

    func scrolled(_ on: Bool) {
        #if os(iOS)
        webView.scrollView.isScrollEnabled = on
        #endif
    }

    static func text(_ pos: NovelPos) -> String {
        "\(pos.spine)/\(pos.path.map(String.init).joined(separator: "."))/\(pos.offset)"
    }

    // MARK: Calling the paginator

    private func call(_ script: String) { webView.evaluateJavaScript(script, completionHandler: nil) }

    func configure(_ style: String) {
        guard !style.isEmpty else { return }
        call("window.kaeriReader && window.kaeriReader.configure(\(style))")
    }

    func turn(forward: Bool, how: String) {
        call("window.kaeriReader && window.kaeriReader.\(forward ? "next" : "prev")('\(how)')")
    }

    func goTo(_ page: Int) { call("window.kaeriReader && window.kaeriReader.goTo(\(max(page, 0)), 'jump')") }

    /// Which taps and keys the page turns on by itself, so a turn is one message from the page:
    /// the tap setting's sides, forward on the side the book reads towards, and none while the
    /// chrome or the end page is up, when a tap is the reader's.
    private var input: String {
        let layout = model?.choice(.tapLayout) ?? "edges"
        let edge = layout == "off" ? 0 : (layout == "sides" ? 0.5 : 0.33)
        let forward = book?.progression == .rightToLeft ? "left" : "right"
        let hold = shown.end || (model?.chromeShown ?? false)
        return "{edge:\(edge),forward:'\(forward)',keys:true,hold:\(hold)}"
    }

    /// Tell the page, when what it turns on changed or a new document came up.
    func sendInput() {
        let now = input
        guard now != sentInput else { return }
        sentInput = now
        call("window.kaeriReader && window.kaeriReader.input && window.kaeriReader.input(\(now))")
    }

    #if os(iOS)
    /// A paged drag, as the manga reader has: the page follows the finger, the next one is
    /// already under it, and letting go short of a quarter of the width brings it back. A drag
    /// to the left moves forward, unless the book runs right to left.
    @objc private func panned(_ gesture: UIPanGestureRecognizer) {
        let dx = gesture.translation(in: webView).x
        let width = max(webView.bounds.width, 1)
        let rtl = book?.progression == .rightToLeft
        switch gesture.state {
        case .began, .changed:
            if carried == nil {
                guard abs(dx) > 6 else { return }
                let forward = (dx < 0) != rtl
                if paper {
                    // Paper mode turns at once; the rest of this drag is dropped.
                    gesture.isEnabled = false
                    gesture.isEnabled = true
                    turn(forward: forward, how: "swipe")
                    return
                }
                guard let shot = webView.snapshotView(afterScreenUpdates: false) else { return }
                let page = UIView(frame: webView.bounds)
                page.backgroundColor = UIColor(pageColor)
                page.isUserInteractionEnabled = false
                shot.frame = page.bounds
                page.addSubview(shot)
                webView.addSubview(page)
                carried = page
                carriedForward = forward
                turn(forward: forward, how: "swipe")
            }
            // The page moves only the way it is going.
            let away: CGFloat = (carriedForward != rtl) ? -1 : 1
            carried?.transform = CGAffineTransform(translationX: away * max(dx * away, 0), y: 0)
        case .ended, .cancelled, .failed:
            guard let page = carried else { return }
            let away: CGFloat = (carriedForward != rtl) ? -1 : 1
            let velocity = gesture.velocity(in: webView).x * away
            let commit = gesture.state == .ended && (dx * away > width * 0.25 || velocity > 450)
            carried = nil
            let forward = carriedForward
            UIView.animate(withDuration: 0.25, delay: 0, options: [.curveEaseOut, .allowUserInteraction]) {
                page.transform = commit ? CGAffineTransform(translationX: away * width, y: 0) : .identity
            } completion: { [weak self] _ in
                if commit {
                    page.removeFromSuperview()
                    return
                }
                // Back where it was: the turn under it is undone before the page lifts.
                guard let self else { page.removeFromSuperview(); return }
                self.webView.evaluateJavaScript(
                    "window.kaeriReader && window.kaeriReader.\(forward ? "prev" : "next")('swipe')"
                ) { _, _ in
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { page.removeFromSuperview() }
                }
            }
        default:
            break
        }
    }
    #endif

    // MARK: Messages

    nonisolated func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let text = message.body as? String else { return }
        Task { @MainActor in self.receive(text) }
    }

    private func receive(_ text: String) {
        heard += 1
        guard let data = text.data(using: .utf8),
            let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let type = json["type"] as? String
        else { return }
        switch type {
        case "ready":
            // A new document: it knows nothing of the host's input until told.
            sentInput = nil
            sendInput()
            setHidden(false)
            page(json, ready: true)
        case "failed": setHidden(false)
        case "page": page(json, ready: false)
        case "layout":
            if let page = json["page"] as? Int, let pages = json["pages"] as? Int { model?.position = (page, max(pages, 1)) }
        case "end": next()
        case "start": previous()
        case "tap": if let x = json["x"] as? Double { tapped(x) }
        case "link": link(json)
        case "key": key(json["key"] as? String ?? "")
        default: break
        }
    }

    private func page(_ json: [String: Any], ready: Bool) {
        guard let page = json["page"] as? Int, let pages = json["pages"] as? Int, let pos = json["pos"] as? [String: Any]
        else { return }
        model?.position = (page, max(pages, 1))
        let place = NovelPos(
            spine: UInt32(pos["spine"] as? Int ?? 0), path: (pos["path"] as? [Int] ?? []).map(UInt32.init),
            offset: UInt32(pos["offset"] as? Int ?? 0))
        if let spine = currentSpine { lastPlace = (spine, place) }
        model?.novelAt(place, page: page, pages: pages, how: Self.how(json["how"] as? String))
    }

    static func how(_ text: String?) -> TurnHow {
        switch text {
        case "swipe": .swipe
        case "key": .key
        case "scroll": .scroll
        case "turn": .turn
        default: .jump
        }
    }

    /// A tap the page did not turn on: while the end page or the chrome is up, in the middle,
    /// or before the page heard which taps to turn on.
    private func tapped(_ x: Double) {
        if shown.end { shown.end = false; return }
        guard let model else { return }
        let layout = model.choice(.tapLayout) ?? "edges"
        if model.chromeShown || layout == "off" { onTap(x); return }
        let edge = layout == "sides" ? 0.5 : 0.33
        if x < edge || x > 1 - edge || layout == "sides" {
            turn(forward: (x < 0.5) == (book?.progression == .rightToLeft), how: "turn")
        } else {
            onTap(x)
        }
    }

    /// Past the last page of a document: the next one, or the end page.
    private func next() {
        guard let book, let current = currentSpine else { return }
        if let index = book.spine.indices.dropFirst(current + 1).first(where: { book.spine[$0].linear }) {
            load(book.spine[index].url, fragment: nil)
        } else {
            shown.end = true
        }
    }

    private func previous() {
        guard let book, let current = currentSpine else { return }
        if let index = book.spine.indices.prefix(current).last(where: { book.spine[$0].linear }) {
            load(book.spine[index].url, fragment: "kaeri:end")
        }
    }

    private var currentSpine: Int? {
        guard let book, let url = webView.url else { return nil }
        let path = url.absoluteString.split(separator: "#").first.map(String.init) ?? ""
        return book.spine.firstIndex { $0.url == path }
    }

    private func load(_ url: String, fragment: String?) {
        guard var parts = URLComponents(string: url) else { return }
        parts.fragment = fragment
        if let target = parts.url { webView.load(URLRequest(url: target)) }
    }

    private func link(_ json: [String: Any]) {
        if let note = json["note"] as? String, !note.isEmpty { shown.note = note; return }
        // Only links inside the book are followed; everything else stays where it is.
        if json["internal"] as? Bool == true, let href = json["href"] as? String, let target = URL(string: href) {
            webView.load(URLRequest(url: target))
        }
    }

    /// A key the page did not turn on, while the chrome or the end page is up.
    private func key(_ key: String) {
        let rtl = book?.progression == .rightToLeft
        let forward: Bool? = switch key {
        case "ArrowRight": !rtl
        case "ArrowLeft": rtl
        case "ArrowDown", "PageDown", " ": true
        case "ArrowUp", "PageUp": false
        default: nil
        }
        if shown.end, forward == false { shown.end = false; return }
        switch key {
        case "ArrowRight": turn(forward: !rtl, how: "key")
        case "ArrowLeft": turn(forward: rtl, how: "key")
        case "ArrowDown", "PageDown", " ": turn(forward: true, how: "key")
        case "ArrowUp", "PageUp": turn(forward: false, how: "key")
        case "Home": goTo(0)
        case "End": goTo(max((model?.position.count ?? 1) - 1, 0))
        default: break
        }
    }

    // MARK: Navigation

    /// Nothing leaves the book: every navigation off the book scheme is refused.
    nonisolated func webView(
        _ webView: WKWebView, decidePolicyFor action: WKNavigationAction,
        decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void
    ) {
        let ok = action.request.url?.scheme == "kaeri-book" || action.request.url?.absoluteString == "about:blank"
        Task { @MainActor in decisionHandler(ok ? .allow : .cancel) }
    }

    /// WebKit ended the page's process, for memory or a crash: the page comes back where it was.
    nonisolated func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        Task { @MainActor in self.reopen() }
    }

    private func reopen() {
        guard let book else { return }
        sentInput = nil
        if let place = lastPlace, book.spine.indices.contains(place.spine) {
            load(book.spine[place.spine].url, fragment: "kaeri:pos=" + Self.text(place.pos))
        } else if let url = URL(string: book.entry) {
            webView.load(URLRequest(url: url))
        }
    }
}

#if os(iOS)
extension NovelWebController: UIGestureRecognizerDelegate {
    /// Only a sideways drag on a paged book turns; scrolling and text selection keep theirs.
    func gestureRecognizerShouldBegin(_ gesture: UIGestureRecognizer) -> Bool {
        guard let pan = gesture as? UIPanGestureRecognizer, !webView.scrollView.isScrollEnabled else { return false }
        let v = pan.velocity(in: webView)
        return abs(v.x) > abs(v.y)
    }
}
#endif

/// The script handler holds the controller weakly, so the web view does not keep it alive.
private final class ScriptProxy: NSObject, WKScriptMessageHandler {
    weak var target: NovelWebController?

    init(_ target: NovelWebController) { self.target = target }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        target?.userContentController(controller, didReceive: message)
    }
}

/// Answers kaeri-book requests from the engine, off the main thread.
final class BookSchemeHandler: NSObject, WKURLSchemeHandler, @unchecked Sendable {
    var resource: (@Sendable (String) -> BookResource?)?
    private let lock = NSLock()
    private var stopped = Set<ObjectIdentifier>()

    func webView(_ webView: WKWebView, start task: any WKURLSchemeTask) {
        guard let url = task.request.url else { return }
        let box = TaskBox(task: task)
        let id = ObjectIdentifier(task as AnyObject)
        let lookup = resource
        DispatchQueue.global(qos: .userInitiated).async { [self] in
            let answer = lookup?(url.absoluteString)
            DispatchQueue.main.async {
                self.lock.lock()
                let gone = self.stopped.remove(id) != nil
                self.lock.unlock()
                if gone { return }
                self.respond(box.task, url: url, answer: answer)
            }
        }
    }

    func webView(_ webView: WKWebView, stop task: any WKURLSchemeTask) {
        lock.lock()
        stopped.insert(ObjectIdentifier(task as AnyObject))
        lock.unlock()
    }

    private func respond(_ task: any WKURLSchemeTask, url: URL, answer: BookResource?) {
        guard let answer else {
            task.didFailWithError(URLError(.fileDoesNotExist))
            return
        }
        var fields = ["Content-Type": answer.mime]
        for header in answer.headers { fields[header.name] = header.value }
        let data: Data
        switch answer.body {
        case .bytes(let bytes): data = Data(bytes)
        case .file(let path): data = (try? Data(contentsOf: URL(fileURLWithPath: path), options: .mappedIfSafe)) ?? Data()
        }
        fields["Content-Length"] = String(data.count)
        let response = HTTPURLResponse(url: url, statusCode: Int(answer.status), httpVersion: "HTTP/1.1", headerFields: fields)
        task.didReceive(response ?? URLResponse(url: url, mimeType: answer.mime, expectedContentLength: data.count, textEncodingName: nil))
        task.didReceive(data)
        task.didFinish()
    }
}

private struct TaskBox: @unchecked Sendable { let task: any WKURLSchemeTask }

/// The reader's web view inside a holder of this place's own: the same view comes back in a new
/// holder after a chapter's opening screen, and is taken out of the old one when it goes.
#if os(iOS)
struct NovelWebView: UIViewRepresentable {
    let controller: NovelWebController

    func makeUIView(context: Context) -> UIView {
        let holder = UIView()
        holder.backgroundColor = .clear
        adopt(into: holder)
        return holder
    }

    func updateUIView(_ holder: UIView, context: Context) { adopt(into: holder) }

    static func dismantleUIView(_ holder: UIView, coordinator: ()) {
        holder.subviews.forEach { $0.removeFromSuperview() }
    }

    private func adopt(into holder: UIView) {
        let view = controller.webView
        guard view.superview !== holder else { return }
        view.removeFromSuperview()
        view.frame = holder.bounds
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        holder.addSubview(view)
    }
}
#else
struct NovelWebView: NSViewRepresentable {
    let controller: NovelWebController

    func makeNSView(context: Context) -> NSView {
        let holder = NSView()
        adopt(into: holder)
        return holder
    }

    func updateNSView(_ holder: NSView, context: Context) { adopt(into: holder) }

    static func dismantleNSView(_ holder: NSView, coordinator: ()) {
        holder.subviews.forEach { $0.removeFromSuperview() }
    }

    private func adopt(into holder: NSView) {
        let view = controller.webView
        guard view.superview !== holder else { return }
        view.removeFromSuperview()
        view.frame = holder.bounds
        view.autoresizingMask = [.width, .height]
        holder.addSubview(view)
    }
}
#endif
