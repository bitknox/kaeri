// A novel in a web view. The engine serves the book's documents on the kaeri-book scheme, the
// paginator lays them out in pages and reports the place, and the host moves between documents.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI
import WebKit

struct NovelReader: View {
    @Bindable var model: ReaderModel
    let theme: ReaderTheme
    let mac: Bool
    let size: CGSize
    let rtl: Bool
    let onTap: (Double) -> Void
    @State private var controller = NovelWebController()
    @State private var endShown = false
    @State private var note: String?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var forwardIsLeft: Bool { model.novel?.progression == .rightToLeft }

    var body: some View {
        ZStack {
            NovelWebView(controller: controller)
                .frame(width: size.width, height: size.height)
            if endShown {
                ReaderEndPage(model: model, theme: theme, size: size).transition(.opacity)
            }
            if let note {
                VStack {
                    Spacer()
                    Text(note).font(.ui(15)).foregroundStyle(theme.silk).lineLimit(8)
                        .padding(16).frame(maxWidth: 520, alignment: .leading)
                        .glassEffect(.regular, in: .rect(cornerRadius: 20))
                        .padding(.bottom, 120).padding(.horizontal, 16)
                        .onTapGesture { self.note = nil }
                }
                .transition(.opacity)
            }
        }
        .animation(reduceMotion ? nil : .fadeIn, value: endShown)
        .animation(reduceMotion ? nil : .fadeIn, value: note)
        .onAppear { wire(); open() }
        .onChange(of: model.novel?.chapter) { _, _ in open() }
        .onChange(of: model.novelStyle) { _, style in controller.configure(style) }
        .onChange(of: model.jump?.tick) { _, _ in
            if let index = model.jump?.index { controller.goTo(index) }
        }
        .onChange(of: model.turnRequest) { _, request in
            guard let request else { return }
            if endShown, !request.forward { endShown = false; return }
            controller.turn(forward: request.forward, how: "key")
        }
        .onChange(of: model.choice(.mode)) { _, mode in controller.scrolled(mode == "scrolled") }
        .onDisappear { controller.stop() }
    }

    private func wire() {
        controller.model = model
        controller.onTap = { x in
            let layout = model.choice(.tapLayout) ?? "edges"
            if endShown { endShown = false; return }
            if model.chromeShown || layout == "off" { onTap(x); return }
            let edge = layout == "sides" ? 0.5 : 0.33
            if x < edge || x > 1 - edge || (layout == "sides") {
                controller.turn(forward: (x < 0.5) == forwardIsLeft, how: "turn")
            } else {
                onTap(x)
            }
        }
        controller.onEnd = { endShown = true }
        controller.onNote = { note = $0 }
    }

    private func open() {
        guard let book = model.novel else { return }
        endShown = false
        controller.open(book, scrolled: model.choice(.mode) == "scrolled")
    }
}

// MARK: Controller

@MainActor
final class NovelWebController: NSObject, WKNavigationDelegate, WKScriptMessageHandler {
    let webView: WKWebView
    weak var model: ReaderModel?
    var onTap: (Double) -> Void = { _ in }
    var onEnd: () -> Void = {}
    var onNote: (String) -> Void = { _ in }
    private var book: NovelOpenView?
    private var openedChapter: String?
    private let handler = BookSchemeHandler()

    override init() {
        let config = WKWebViewConfiguration()
        let assets = novelAssets()
        let script = WKUserScript(source: assets.script, injectionTime: .atDocumentStart, forMainFrameOnly: true)
        config.userContentController.addUserScript(script)
        config.setURLSchemeHandler(handler, forURLScheme: "kaeri-book")
        webView = WKWebView(frame: .zero, configuration: config)
        super.init()
        config.userContentController.add(ScriptProxy(self), name: "kaeri")
        webView.navigationDelegate = self
        #if os(iOS)
        webView.isOpaque = false
        webView.backgroundColor = .clear
        webView.scrollView.backgroundColor = .clear
        webView.scrollView.contentInsetAdjustmentBehavior = .never
        webView.scrollView.isScrollEnabled = false
        webView.allowsLinkPreview = false
        for direction in [UISwipeGestureRecognizer.Direction.left, .right] {
            let swipe = UISwipeGestureRecognizer(target: self, action: #selector(swiped(_:)))
            swipe.direction = direction
            webView.addGestureRecognizer(swipe)
        }
        #else
        webView.setValue(false, forKey: "drawsBackground")
        #endif
        webView.underPageBackgroundColor = .clear
    }

    func stop() {
        webView.stopLoading()
    }

    func open(_ book: NovelOpenView, scrolled: Bool) {
        guard openedChapter != book.chapter else { return }
        openedChapter = book.chapter
        self.book = book
        handler.resource = model?.bookResource
        self.scrolled(scrolled)
        var url = book.entry
        if let at = book.restore, book.spine.indices.contains(Int(at.spine)) {
            url = book.spine[Int(at.spine)].url + "#kaeri:pos=" + Self.text(at)
        }
        if let target = URL(string: url) { webView.load(URLRequest(url: target)) }
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

    #if os(iOS)
    @objc private func swiped(_ gesture: UISwipeGestureRecognizer) {
        // A swipe to the left moves forward, unless the book runs right to left.
        let left = gesture.direction == .left
        let rtl = book?.progression == .rightToLeft
        turn(forward: left != rtl, how: "swipe")
    }
    #endif

    // MARK: Messages

    nonisolated func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let text = message.body as? String else { return }
        Task { @MainActor in self.receive(text) }
    }

    private func receive(_ text: String) {
        guard let data = text.data(using: .utf8),
            let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let type = json["type"] as? String
        else { return }
        switch type {
        case "ready", "page": page(json, ready: type == "ready")
        case "layout":
            if let page = json["page"] as? Int, let pages = json["pages"] as? Int { model?.position = (page, max(pages, 1)) }
        case "end": next()
        case "start": previous()
        case "tap": if let x = json["x"] as? Double { onTap(x) }
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

    /// Past the last page of a document: the next one, or the end page.
    private func next() {
        guard let book, let current = currentSpine else { return }
        if let index = book.spine.indices.dropFirst(current + 1).first(where: { book.spine[$0].linear }) {
            load(book.spine[index].url, fragment: nil)
        } else {
            onEnd()
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
        if let note = json["note"] as? String, !note.isEmpty { onNote(note); return }
        // Only links inside the book are followed; everything else stays where it is.
        if json["internal"] as? Bool == true, let href = json["href"] as? String, let target = URL(string: href) {
            webView.load(URLRequest(url: target))
        }
    }

    private func key(_ key: String) {
        let rtl = book?.progression == .rightToLeft
        switch key {
        case "ArrowRight": turn(forward: !rtl, how: "key")
        case "ArrowLeft": turn(forward: rtl, how: "key")
        case "ArrowDown", "PageDown", " ": turn(forward: true, how: "key")
        case "ArrowUp", "PageUp": turn(forward: false, how: "key")
        case "Home": goTo(0)
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
}

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

#if os(iOS)
struct NovelWebView: UIViewRepresentable {
    let controller: NovelWebController
    func makeUIView(context: Context) -> WKWebView { controller.webView }
    func updateUIView(_ view: WKWebView, context: Context) {}
}
#else
struct NovelWebView: NSViewRepresentable {
    let controller: NovelWebController
    func makeNSView(context: Context) -> WKWebView { controller.webView }
    func updateNSView(_ view: WKWebView, context: Context) {}
}
#endif
