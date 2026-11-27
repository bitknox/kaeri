// A source's words: why a source listed nothing, the line over the reasons, a release's line.

@testable import KaeriStore
import KaeriKit
import Testing

struct SourceWordsTests {
    @Test func eachSourceThatListedNothingSaysWhyOnceByName() {
        #expect(SourceWords.missed(MissedListing(name: "Alpha", miss: .failed), episode: "7") == "Alpha didn't answer")
        #expect(SourceWords.missed(MissedListing(name: "Alpha", miss: .timedOut), episode: "7") == "Alpha took too long")
        #expect(SourceWords.missed(MissedListing(name: "Brook", miss: .noEpisode), episode: "7") == "Brook doesn't have episode 7")
        #expect(SourceWords.missed(MissedListing(name: "Harbor", miss: .heldBack(why: .torrentsOff)), episode: "7")
            == "Harbor isn't asked while torrent playback is off")
        #expect(SourceWords.missed(MissedListing(name: "Harbor", miss: .heldBack(why: .torrentsStarting)), episode: "7")
            == "Harbor isn't asked until torrent playback has started")
        #expect(SourceWords.unreached(.failed) && SourceWords.unreached(.heldBack(why: .proxyCheck)))
        #expect(!SourceWords.unreached(.noEpisode))
    }

    @Test func theLineOverTheReasonsClaimsOnlyWhatTheSourcesThatAnsweredSaid() {
        let answered = [MissedListing(name: "Brook", miss: .noEpisode), MissedListing(name: "Kumo", miss: .noMatch)]
        #expect(SourceWords.noneLists(answered, episode: "7") == "No source lists episode 7.")
        let failed = [MissedListing(name: "Alpha", miss: .failed), MissedListing(name: "Brook", miss: .noEpisode)]
        #expect(SourceWords.noneLists(failed, episode: "7") == "No source that answered lists episode 7.")
        let held = [MissedListing(name: "Harbor", miss: .heldBack(why: .torrentsOff))]
        #expect(SourceWords.noneLists(held, episode: "7") == "No source that answered lists episode 7.")
    }

    @Test func aReleaseNamesItsSource() {
        let one = SourceCandidate(
            id: "a", providerId: "sample-source", providerName: "Sample Source", title: "Stardust Wayfarer 07", quality: 1080, // i18n-skip
            label: nil, size: "1.2 GiB", dualAudio: false, dubbed: false, autoPick: true)
        #expect(SourceWords.detail(one) == "Sample Source · 1080p · 1.2 GiB")
    }
}
