// What the reading screens ask again after the reader: the engine sends nothing for a place kept
// or a chapter read, so the series page, Read Home and Updates would keep the place they showed
// when the reader opened.

import KaeriKit

/// Follows the reader's events for the screens under it.
@MainActor
final class ReadingAfterReader {
    /// The book the reader has open.
    private var work: String?

    func observe(_ event: Event, reading: ReadingModel) {
        guard case .reader(let event) = event else { return }
        switch event {
        case .opened(let view):
            work = view.work
        case .chapterRead(let work, _, _):
            // Counted as read on the end page: the series page under it offers the next chapter.
            reading.readerMoved(work, home: false)
        case .closed:
            // The last place is written by the time the engine says it closed.
            if let work { reading.readerMoved(work, home: true) }
            work = nil
        default:
            break
        }
    }
}

extension ReadingModel {
    /// The series page, Read Home's Continue and Updates again, where they are on screen.
    func readerMoved(_ work: String, home: Bool) {
        if series[work] != nil || chapters[work] != nil {
            send?(.reading(.loadSeries(work: work)))
            send?(.reading(.loadChapters(work: work, edition: chapters[work]?.edition?.sourceId)))
        }
        if home, self.home != nil { send?(.reading(.loadHome(refresh: false))) }
        if updates != nil { send?(.reading(.loadUpdates)) }
    }
}
