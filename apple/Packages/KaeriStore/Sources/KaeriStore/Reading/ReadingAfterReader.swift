// What the reading screens ask again after the reader: the engine sends nothing for a place kept
// or a chapter read, so the series page, Read Home and Updates would keep the place they showed
// when the reader opened. ReadingModel hears the reader's events and asks through here, once.

import KaeriKit

extension ReadingModel {
    /// The series page, Read Home's Continue and Updates again, where they are on screen.
    func readerMoved(_ work: String, home: Bool) {
        if series[work] != nil || chapters[work] != nil {
            // The series lists the edition it reads from again with itself: only another edition
            // the page shows needs an ask of its own, so a close asks its source once.
            send?(.reading(.loadSeries(work: work)))
            if let edition = chapters[work]?.edition, !edition.inUse {
                send?(.reading(.loadChapters(work: work, edition: edition.sourceId)))
            }
        }
        if home, self.home != nil { send?(.reading(.loadHome(refresh: false))) }
        if updates != nil { send?(.reading(.loadUpdates)) }
    }
}
