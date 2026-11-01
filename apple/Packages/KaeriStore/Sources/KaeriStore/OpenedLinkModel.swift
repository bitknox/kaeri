// A magnet link or torrent file the person opened: what the engine read from it, its guess at the
// title and episode, and the titles Change lists. The sheet asks "Is this ...?" from this.

import KaeriKit
import Foundation
import Observation

@MainActor @Observable
public final class OpenedLinkModel {
    /// What the engine read from the link, `nil` while no link is open.
    public private(set) var link: OpenedLink?
    /// Whether the person is choosing another title: the Change list is showing.
    public var changing = false
    /// The titles Change lists: the engine's candidates, or what a search of other words found.
    public private(set) var choices: [Entry] = []
    /// The words of the last search of other titles, and whether it is out.
    public private(set) var searching = false
    public private(set) var searchFailed: String?
    /// The paste field's text, kept while the sheet reads it.
    public var input = ""
    /// Whether the engine has been asked to read a link and has not answered yet.
    public private(set) var reading = false

    public init() {}

    var isOpen: Bool { link != nil }

    func asked() {
        reading = true
    }

    func opened(_ link: OpenedLink) {
        reading = false
        self.link = link
        choices = link.candidates
        changing = false
        searching = false
        searchFailed = nil
    }

    func searchStarted() {
        searching = true
        searchFailed = nil
    }

    func found(link: String, candidates: [Entry], failed: String?) {
        guard self.link?.link == link else { return }
        searching = false
        choices = candidates
        searchFailed = failed
    }

    public func close() {
        link = nil
        changing = false
        choices = []
        reading = false
        searching = false
        searchFailed = nil
        input = ""
    }
}
