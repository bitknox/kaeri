// Which titles the calendar's My list filter keeps.

import KaeriKit
import KaeriStore
@testable import KaeriUI
import Testing

@MainActor
struct CalendarMineTests {
    private func title(status: LibrarySegment?, progress: TitleProgress?) -> Entry {
        entry {
            $0.title = "Lantern Harbour"
            $0.listStatus = status
            $0.progress = progress
        }
    }

    @Test func aDroppedTitleIsNotMineWhateverItsProgress() {
        #expect(!AppStore.isMine(title(status: .dropped, progress: TitleProgress(done: 3, next: 4))))
        #expect(!AppStore.isMine(title(status: .dropped, progress: nil)))
    }

    @Test func watchingAndPlanningAreMine() {
        #expect(AppStore.isMine(title(status: .watching, progress: nil)))
        #expect(AppStore.isMine(title(status: .planning, progress: nil)))
    }

    @Test func progressWithNoStatusIsMine() {
        #expect(AppStore.isMine(title(status: nil, progress: TitleProgress(done: 3, next: 4))))
        #expect(!AppStore.isMine(title(status: nil, progress: nil)))
    }
}
