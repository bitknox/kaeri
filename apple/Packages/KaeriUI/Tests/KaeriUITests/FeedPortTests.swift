// What the Mac's feed port field writes when it loses focus.

#if os(macOS)
@testable import KaeriUI
import Testing

struct FeedPortTests {
    @Test func onlyListeningPortsAreKept() {
        #expect(FeedPort.kept("8484") == "8484")
        #expect(FeedPort.kept(" 9 000 ") == "9000")
        #expect(FeedPort.kept("80") == "", "below 1024")
        #expect(FeedPort.kept("70000") == "", "past 65535")
        #expect(FeedPort.kept("") == "")
    }

    @Test func leavingTheFieldUnchangedWritesNothing() {
        #expect(FeedPort.write("", saved: "") == nil, "an empty field over no port")
        #expect(FeedPort.write("9000", saved: "9000") == nil)
        #expect(FeedPort.write("9001", saved: "9000") == "9001")
        #expect(FeedPort.write("", saved: "9000") == "", "emptied on purpose")
    }
}
#endif
