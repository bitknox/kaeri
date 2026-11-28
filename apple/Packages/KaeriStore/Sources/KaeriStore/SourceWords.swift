// A source's words, wherever its releases are listed: the player's sources, Choose a source,
// and the room's own list.

import KaeriKit
import KaeriStrings

public enum SourceWords {
    /// A source's second line: "Sample Source · 1080p · 1.2 GiB · lantern-archive", the source by
    /// the name it goes by.
    public static func detail(_ candidate: SourceCandidate) -> String {
        var parts = [sourceName(candidate)]
        if let quality = candidate.quality { parts.append("\(quality)p") }
        if let size = candidate.size, !size.isEmpty { parts.append(size) }
        if let label = candidate.label { parts.append(label) }
        return parts.joined(separator: " · ")
    }

    /// The name a release's source goes by, and its id when the engine gave none.
    public static func sourceName(_ candidate: SourceCandidate) -> String {
        candidate.providerName.isEmpty ? candidate.providerId : candidate.providerName
    }

    /// Why one source listed no release for `episode`: its name once, as it goes by.
    /// The line over the reasons when no source lists the episode: that none lists it only when
    /// every source answered, and that none that answered lists it when any failed, took too long
    /// or wasn't asked.
    public static func noneLists(_ missed: [MissedListing], episode: String) -> String {
        missed.contains { unreached($0.miss) } ? L.sourcePickerNoneAnsweredLists(episode: episode) : L.sourcePickerNoneLists(episode: episode)
    }

    public static func missed(_ missed: MissedListing, episode: String) -> String {
        let name = missed.name
        switch missed.miss {
        case .noMatch: return L.playerStartMissNoMatch(name: name)
        case .ambiguous: return L.playerStartMissAmbiguous(name: name)
        case .noEpisode: return L.playerStartMissNoEpisode(name: name, episode: episode)
        case .failed: return L.playerStartMissFailed(name: name)
        case .timedOut: return L.playerStartMissTimedOut(name: name)
        case .heldBack(let why):
            switch why {
            case .torrentsOff: return L.sourcePickerHeldTorrentsOff(name: name)
            case .torrentsStarting: return L.sourcePickerHeldTorrentsStarting(name: name)
            case .torrentsFailed: return L.sourcePickerHeldTorrentsFailed(name: name)
            case .proxyCheck: return L.sourcePickerHeldProxy(name: name)
            case .cellular: return L.sourcePickerHeldCellular(name: name)
            case .other: return L.sourcePickerHeldOther(name: name)
            }
        }
    }

    /// Whether a source gave no real answer (it failed, took too long or was held back), so Try
    /// again may reach it.
    public static func unreached(_ miss: ListingMiss) -> Bool {
        switch miss {
        case .failed, .timedOut, .heldBack: true
        case .noMatch, .ambiguous, .noEpisode: false
        }
    }
}
