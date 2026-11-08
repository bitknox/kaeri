// Did my server get it: the page between chapters' line and the server row's line follow the
// engine's facts with the waiting rule (nothing for the wait, then Sending for at least the hold).

@testable import KaeriStore
import KaeriKit
import Testing

@MainActor
@Suite(.serialized)
struct ServerProgressTests {
    private let host = "books.home.example"

    init() {
        ProgressWaiting.wait = .milliseconds(120)
        ProgressWaiting.hold = .milliseconds(120)
    }

    private func model(sent: @escaping (Command) -> Void = { _ in }) -> ServerProgressModel {
        let model = ServerProgressModel()
        model.hostFor = { [host] _ in host }
        model.send = sent
        return model
    }

    private func row(pending: UInt32 = 0, sending: Bool = false, saved: Int64? = 1_000, problem: WriteBackProblem? = nil,
                     problemAt: Int64? = nil, report: Bool = true, kind: BookServerKind = .kavita) -> BookServerRow {
        BookServerRow(id: "s1", kind: kind, name: host, address: "https://\(host)", series: 212, reportProgress: report,
                      off: false, signedOut: false, libraries: 2, empty: false, rechecksAt: nil, keyLast4: nil,
                      progressPending: pending, progressSending: sending, progressSavedAt: saved,
                      progressProblem: problem, progressProblemAt: problemAt)
    }

    private func settle() async { try? await Task.sleep(for: .milliseconds(400)) }

    /// Waits for `condition` rather than for a fixed time, so a busy machine that runs the
    /// waiting line's timer late doesn't fail a test about what the line shows.
    private func eventually(_ condition: () -> Bool) async -> Bool {
        let deadline = ContinuousClock.now + .seconds(3)
        while !condition() {
            if ContinuousClock.now > deadline { return false }
            try? await Task.sleep(for: .milliseconds(10))
        }
        return true
    }

    @Test func aQuickAnswerIsTheFirstThingSeen() async {
        let model = model()
        model.apply(.reader(.chapterRead(work: "w", chapter: "12", trackers: ["AniList"])))
        #expect(model.holds(work: "w", chapter: "12"))
        #expect(model.lines["12"] == nil)
        model.apply(.reader(.progressSent(work: "w", chapter: "12", server: "s1", host: host, result: .saved)))
        #expect(model.lines["12"] == .saved(host: host))
        await settle()
        #expect(model.lines["12"] == .saved(host: host))
    }

    @Test func aSlowAnswerShowsSendingThenWaitsForTheHold() async {
        let model = model()
        model.apply(.reader(.chapterRead(work: "w", chapter: "12", trackers: [])))
        #expect(model.lines["12"] == nil)
        #expect(await eventually { model.lines["12"] == .sending(host: host) })
        model.apply(.reader(.progressSent(work: "w", chapter: "12", server: "s1", host: host, result: .saved)))
        // Sending stays until it has shown for the hold.
        #expect(model.lines["12"] == .sending(host: host))
        #expect(await eventually { model.lines["12"] == .saved(host: host) })
    }

    @Test func eachResultSaysItsLine() {
        let model = model()
        let cases: [(WriteBackResult, ServerLine?)] = [
            (.waiting(problem: .offline), .offline(host: host)),
            (.waiting(problem: .noAnswer), .retrying(host: host)),
            (.waiting(problem: .serverError), .retrying(host: host)),
            (.notSaved(problem: .refusedKey), .failed(host: host, why: .key)),
            (.notSaved(problem: .refusedPassword), .failed(host: host, why: .password)),
            (.notSaved(problem: .seriesGone), .failed(host: host, why: .gone)),
            (.notSaved(problem: .noAnswerForADay), .failed(host: host, why: .gaveUp)),
            // Not set to receive was a choice: no line.
            (.notSaved(problem: .notSetToReceive), nil),
        ]
        for (index, (result, line)) in cases.enumerated() {
            let chapter = "\(index)"
            model.apply(.reader(.progressSent(work: "w", chapter: chapter, server: "s1", host: host, result: result)))
            #expect(model.lines[chapter] == line, "\(result)")
        }
    }

    @Test func sendingOnCloseHasNoLine() {
        let model = model()
        model.quiet = { true }
        model.apply(.reader(.chapterRead(work: "w", chapter: "12", trackers: [])))
        model.apply(.reader(.progressSent(work: "w", chapter: "12", server: "s1", host: host, result: .saved)))
        #expect(model.lines["12"] == nil)
        #expect(!model.holds(work: "w", chapter: "12"))
    }

    @Test func aChapterFromNoServerHoldsNoRoom() {
        let model = ServerProgressModel()
        model.apply(.reader(.chapterRead(work: "w", chapter: "12", trackers: [])))
        #expect(!model.holds(work: "w", chapter: "12"))
    }

    @Test func anOfflineLineFollowsTheRowBackOnline() async {
        let model = model()
        model.apply(.bookSources(.view(BookSourcesView(servers: [row(pending: 1, problem: .offline)]))))
        model.apply(.reader(.progressSent(work: "w", chapter: "12", server: "s1", host: host, result: .waiting(problem: .offline))))
        #expect(model.lines["12"] == .offline(host: host))
        model.apply(.bookSources(.view(BookSourcesView(servers: [row(pending: 0, saved: 2_000)]))))
        #expect(await eventually { model.lines["12"] == .saved(host: host) })
    }

    @Test func theRowSaysWhereTheQueueStands() async {
        let model = model()
        let cases: [(BookServerRow, ServerRowLine?)] = [
            (row(saved: nil), nil),
            (row(saved: 1_000), .saved(at: 1_000)),
            (row(pending: 3, problem: .offline), .offline(count: 3)),
            (row(pending: 3, problem: .noAnswer, problemAt: 5), .retrying(count: 3)),
            (row(pending: 3, problem: .refusedKey, problemAt: 5), .failed(count: 3, why: .key)),
            (row(pending: 3, problem: .noAnswerForADay, problemAt: 5), .failed(count: 3, why: .gaveUp)),
            (row(saved: nil, problem: .notSetToReceive, report: false), .off),
            // An OPDS catalogue takes no progress: no line at all.
            (row(saved: nil, problem: .notSetToReceive, report: false, kind: .opds), nil),
        ]
        for (row, line) in cases {
            model.apply(.bookSources(.view(BookSourcesView(servers: [row]))))
            await settle()
            #expect(model.rowLines["s1"] == line, "\(String(describing: row.progressProblem))")
        }
    }

    @Test func tryAgainDimsThenSendsThenAnswers() async {
        var sent: [Command] = []
        let model = model { sent.append($0) }
        let failed = row(pending: 3, problem: .refusedKey, problemAt: 5)
        model.apply(.bookSources(.view(BookSourcesView(servers: [failed]))))
        #expect(model.rowLines["s1"] == .failed(count: 3, why: .key))
        model.retryRow("s1")
        #expect(model.retrying.contains("s1"))
        #expect(sent.count == 1)
        // The engine publishes the old facts at once: the press is not answered by them.
        model.apply(.bookSources(.view(BookSourcesView(servers: [failed]))))
        #expect(await eventually { model.rowLines["s1"] == .sending(count: 3) })
        model.apply(.bookSources(.view(BookSourcesView(servers: [row(pending: 3, sending: true, problem: .refusedKey, problemAt: 5)]))))
        model.apply(.bookSources(.view(BookSourcesView(servers: [row(pending: 0, saved: 3_000)]))))
        #expect(await eventually { model.rowLines["s1"] == .saved(at: 3_000) })
        #expect(!model.retrying.contains("s1"))
    }

    @Test func aQuickQueueNeverShowsSending() async {
        let model = model()
        model.apply(.bookSources(.view(BookSourcesView(servers: [row(saved: 1_000)]))))
        model.apply(.bookSources(.view(BookSourcesView(servers: [row(pending: 1, sending: true, saved: 1_000)]))))
        #expect(model.rowLines["s1"] == .saved(at: 1_000))
        model.apply(.bookSources(.view(BookSourcesView(servers: [row(saved: 2_000)]))))
        #expect(model.rowLines["s1"] == .saved(at: 2_000))
    }
}
