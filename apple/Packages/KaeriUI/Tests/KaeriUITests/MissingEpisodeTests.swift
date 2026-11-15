// What a title page says while its next episode has aired and no source lists it: the status's
// head and tail, the row's line, and the source picker's lines.

@testable import KaeriUI
import Foundation
import KaeriKit
import KaeriStore
import Testing

struct MissingEpisodeTests {
    /// Saturday 7 November 2026, 17:55 UTC.
    private let now = Date(timeIntervalSince1970: 1_793_987_700)

    private func missing(
        airedAgo seconds: Int64?, phase: MissingPhase, previous: UInt32? = 5
    ) -> MissingEpisode {
        MissingEpisode(
            episode: 6, airedAt: seconds.map { MissingWords.unix(now) - $0 }, phase: phase,
            previous: previous, everyMinutes: 15, forHours: 6
        )
    }

    private func row(airedAgo seconds: Int64?, unsourced: Bool = true) -> EpisodeRow {
        var row = EpisodeRow(
            number: "6", title: "The Lantern Road", thumbnail: nil, description: nil, durationSecs: 1440,
            watched: 0, completed: false, kind: nil, skippable: false,
            airsAt: seconds.map { MissingWords.unix(now) - $0 }
        )
        row.unsourced = unsourced
        return row
    }

    @Test func theHeadSaysWhenItAiredInMinutesHoursOrADate() {
        #expect(MissingWords.head(missing(airedAgo: 25 * 60, phase: .quiet), now: now) == "Episode 6 aired 25 min ago.")
        #expect(MissingWords.head(missing(airedAgo: 3 * 3600 + 60, phase: .quiet), now: now) == "Episode 6 aired 3 h ago.")
        let dated = MissingWords.head(missing(airedAgo: 2 * 86_400, phase: .after), now: now)
        #expect(dated.hasPrefix("Episode 6 aired on ") && dated.hasSuffix("."))
        #expect(MissingWords.head(missing(airedAgo: nil, phase: .undated), now: now) == "No source has this episode")
    }

    @Test func theTailFollowsThePhaseTheAnswerAndTheNetwork() {
        let quiet = missing(airedAgo: 25 * 60, phase: .quiet)
        #expect(MissingWords.tail(quiet, answer: nil, offline: false)
            == "Your sources don't have it yet. Kaeri looks again every 15 minutes.")
        #expect(MissingWords.tail(quiet, answer: .still, offline: false)
            == "Your sources still don't have it. Kaeri looks again every 15 minutes.")
        #expect(MissingWords.tail(quiet, answer: .noAnswer, offline: false)
            == "Your sources didn't answer. Kaeri looks again every 15 minutes.")
        #expect(MissingWords.tail(quiet, answer: nil, offline: true)
            == "Your sources don't have it yet. Kaeri looks for it when you're back online.")
        let after = missing(airedAgo: 2 * 86_400, phase: .after)
        #expect(MissingWords.tail(after, answer: nil, offline: false) == "Your sources still don't have it.")
        #expect(MissingWords.tail(after, answer: nil, offline: true) == "Your sources still don't have it.",
                "past the quiet window nothing looks by itself, so offline keeps still")
        #expect(MissingWords.tail(missing(airedAgo: 8 * 86_400, phase: .longAgo), answer: nil, offline: false)
            == "None of your sources has it.")
        #expect(MissingWords.tail(missing(airedAgo: nil, phase: .undated), answer: nil, offline: false).isEmpty)
    }

    @Test func checkAgainsHintSaysKaeriLooksByItselfOnlyWhileItDoes() {
        #expect(MissingWords.checkHint(missing(airedAgo: 25 * 60, phase: .quiet))
            == "Kaeri also looks by itself every 15 minutes for 6 hours after an episode airs.")
        #expect(MissingWords.checkHint(missing(airedAgo: 2 * 86_400, phase: .after)) == nil)
    }

    @Test func aScreenReaderHearsTheTimeInWholeWords() {
        #expect(MissingWords.spokenHead(missing(airedAgo: 25 * 60, phase: .quiet), now: now)
            == "Episode 6 aired 25 minutes ago.")
        #expect(MissingWords.spoken(.found(episode: 6)) == "Episode 6 is in your sources now.")
        #expect(MissingWords.spoken(.still) == "Your sources still don't have it.")
    }

    @Test func theRowSaysHowLongAgoThenNotYetThenNoSource() {
        func line(_ row: EpisodeRow) -> String {
            EpisodeFacts.line(row, current: true, minutesLeft: nil, leadsWithNumber: false, now: now)
        }
        #expect(line(row(airedAgo: 25 * 60)) == "24 min · Aired 25 min ago · Not in your sources yet")
        let week = line(row(airedAgo: 2 * 86_400))
        #expect(week.hasPrefix("24 min · Aired ") && week.hasSuffix(" · Not in your sources yet"))
        #expect(line(row(airedAgo: 8 * 86_400)).hasSuffix(" · No source has this episode"))
        #expect(!line(row(airedAgo: 25 * 60, unsourced: false)).contains("ago"), "a sourced row keeps its date")
    }

    @Test func aSourcesLineSaysWhichEpisodesItHasAndLacks() {
        func option(covered: UInt32, of: UInt32 = 6, last: UInt32, lacking: [UInt32], since: Int64? = nil) -> SourceOption {
            SourceOption(
                providerId: "a", name: "A", covered: covered, of: of, pinned: false, note: nil, releases: [],
                pinnedRelease: nil, first: 1, last: last, lacking: lacking,
                lackingSince: since.map { MissingWords.unix(now) - $0 }
            )
        }
        #expect(SourcePickerWords.detail(option(covered: 6, last: 6, lacking: []), now: now) == "Episodes 1 to 6")
        #expect(SourcePickerWords.detail(option(covered: 5, last: 5, lacking: [6], since: 1500), now: now)
            == "Episodes 1 to 5. Not 6 yet")
        #expect(SourcePickerWords.detail(option(covered: 4, last: 4, lacking: [5, 6], since: 1500), now: now)
            == "Episodes 1 to 4. Not 5 or 6 yet")
    }
}
