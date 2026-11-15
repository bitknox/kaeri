// A jump from a novel's end page puts the page away: Home, End, Go to page and the scrubber land
// on a page that shows and turns, as a turn back from the end page does. In the web view suite,
// which runs one at a time, since it counts the controllers alive.

#if os(macOS)
import KaeriKit
@testable import KaeriUI
import Testing

extension NovelWebLifecycleTests {
    @Test func aJumpPutsTheEndPageAway() {
        let controller = NovelWebController()
        controller.shown.end = true
        controller.goTo(0)
        #expect(!controller.shown.end)
    }

    @Test func aTurnBackStillPutsTheEndPageAway() {
        let controller = NovelWebController()
        controller.shown.end = true
        controller.turn(forward: false, how: "key")
        #expect(!controller.shown.end)
    }
}
#endif
