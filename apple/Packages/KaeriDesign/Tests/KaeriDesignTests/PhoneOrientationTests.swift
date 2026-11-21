// The iPhone runs in portrait; only the player and the reader turn it (native-ux, iPhone).

import KaeriDesign
import Testing

struct PhoneOrientationTests {
    @Test func theAppIsPortraitWithNothingOverIt() {
        #expect(PhoneOrientation().allowed == .portrait)
    }

    @Test func thePlayerAndTheReaderTurnItAndGiveItBackInPortrait() {
        var now = PhoneOrientation()
        now.player = true
        #expect(now.allowed == .landscape)
        now.player = false
        #expect(now.allowed == .portrait)
        now.reader = true
        #expect(now.allowed == .either)
        now.player = true
        #expect(now.allowed == .landscape)
        now.player = false
        #expect(now.allowed == .either)
        now.reader = false
        #expect(now.allowed == .portrait)
    }
}
