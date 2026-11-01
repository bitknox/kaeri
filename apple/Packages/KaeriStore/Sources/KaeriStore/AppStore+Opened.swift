// Opening a magnet link or torrent file: the commands the open sheet and the confirm step send, and
// the engine's answers to them.

import KaeriKit
import Foundation

extension AppStore {
    /// Read a magnet link or info hash the person typed or pasted.
    public func openLink(_ input: String) {
        let input = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !input.isEmpty else { return }
        opened.asked()
        send(.opened(.openLink(input: input)))
    }

    /// Read a torrent file's bytes, as a document picker or the system's open hands them over.
    public func openTorrentFile(_ bytes: Data) {
        opened.asked()
        send(.opened(.openTorrentFile(bytes: bytes)))
    }

    /// Open what the system handed the app: a `magnet:` link or a `.torrent` file. Only these
    /// two kinds are read; anything else is left alone. Returns whether it was one of them.
    @discardableResult
    public func openFromSystem(_ url: URL) -> Bool {
        if url.scheme?.lowercased() == "magnet" {
            openLink(url.absoluteString)
            return true
        }
        guard url.isFileURL, url.pathExtension.lowercased() == "torrent" else { return false }
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        guard let bytes = try? Data(contentsOf: url) else { return false }
        openTorrentFile(bytes)
        return true
    }

    /// Change: search the catalogue for other words.
    public func findOpened(_ query: String) {
        let query = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let link = opened.link?.link, !query.isEmpty else { return }
        opened.searchStarted()
        send(.opened(.find(link: link, query: query)))
    }

    /// Play it as `episode` of `entry`, as the person confirmed.
    public func playOpened(_ entry: Entry, episode: String, title: String) {
        guard let link = opened.link?.link else { return }
        send(.opened(.play(link: link, id: entry.id, episode: episode, title: title)))
        opened.close()
    }

    /// Play it with nothing recorded.
    public func playOpenedUntracked() {
        guard let link = opened.link else { return }
        send(.opened(.playUntracked(link: link.link, name: link.name ?? "")))
        opened.close()
    }

    /// Queue it for offline as `episode` of `entry`.
    public func downloadOpened(_ entry: Entry, episode: String) {
        guard let link = opened.link?.link else { return }
        send(.opened(.download(link: link, id: entry.id, episode: episode)))
        opened.close()
    }

    func applyOpened(_ event: Event) {
        switch event {
        case .linkOpened(let link):
            opened.opened(link)
        case .openedChoices(let link, _, let candidates, let failed):
            opened.found(link: link, candidates: candidates, failed: failed?.text)
        default:
            break
        }
    }
}
