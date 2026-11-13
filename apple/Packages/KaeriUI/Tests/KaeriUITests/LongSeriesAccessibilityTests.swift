#if os(macOS)
// A series of thousands of chapters builds only the rows in view, for VoiceOver as for the screen
// (Mac R8-M9: the first accessibility walk of a 3245-chapter page held the main thread for 109 s
// and grew the app past 1 GB, every row built for it). The rows that appear are counted, since a
// test process gives no accessibility tree; the build time and footprint are printed for the notes.

import AppKit
import Darwin
import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import SwiftUI
import Testing

extension WindowEventTests {
    @MainActor
    @Suite(.serialized)
    struct LongSeriesAccessibilityTests {
        private let size = CGSize(width: 1000, height: 800)
        private let work = "a:900307"
        private let count = 3245

        private func store() -> AppStore {
            let store = AppStore()
            store.reading.send = { _ in }
            store.reading.apply(.series(SeriesView(
                work: WorkId(work), medium: .novel, titles: WorkTitles(display: "The Glass Orchard"),
                action: .read(chapter: ChapterId("c1"), label: "Read chapter 1"))))
            let rows = (1...count).map { n in
                ChapterRow(id: ChapterId("c\(n)"), label: "Chapter \(n)", state: .unread, number: "\(n)", title: "The lamp at the stair \(n)")
            }
            store.reading.apply(.chapters(
                work: WorkId(work), edition: nil, volumes: [VolumeGroup(chapters: rows, flat: true)], failure: nil))
            return store
        }

        /// The app's physical footprint, in MB.
        private static func footprint() -> Double {
            var info = task_vm_info_data_t()
            var size = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<integer_t>.size)
            let result = withUnsafeMutablePointer(to: &info) {
                $0.withMemoryRebound(to: integer_t.self, capacity: Int(size)) {
                    task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &size)
                }
            }
            return result == KERN_SUCCESS ? Double(info.phys_footprint) / 1_048_576 : -1
        }

        @Test func aLongSeriesBuildsOnlyTheRowsInView() async throws {
            let store = store()
            ChapterRowsShown.count = 0
            let before = Self.footprint()
            let start = Date()
            let window = TapWindow(SeriesPage(work: work).environment(store).environment(Navigator()), size: size)
            defer { window.close() }
            try await window.settle()
            let built = Date().timeIntervalSince(start)
            let rows = ChapterRowsShown.count
            print("measure: page built in \(String(format: "%.2f", built)) s, \(rows) chapter rows, footprint \(Int(before)) MB to \(Int(Self.footprint())) MB") // i18n-skip
            #expect(rows > 0, "the rows in view are built")
            #expect(rows < 200, "every one of \(count) rows was built: \(rows)")
        }
    }
}
#endif
