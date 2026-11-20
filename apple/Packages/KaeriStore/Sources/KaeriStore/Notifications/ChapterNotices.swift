// What the app notifies of new chapters, worked out from what the engine's check says: one
// notification per series, as the Android app posts them. Pure, so it is tested without the
// system's notification centre.

import KaeriKit
import KaeriStrings
import Foundation

/// One series' new chapters, to post now.
public struct ChapterNotice: Equatable, Sendable {
    /// `ch.<work>`: a series said again replaces its notification rather than adding a second.
    public var identifier: String
    public var work: WorkId
    /// The series' title: the notification's title.
    public var title: String
    /// "Chapter 12 is new." for one chapter, "3 new chapters" for several.
    public var body: String
    /// The chapter Read opens: the one the person is up to, or the new one when caught up.
    public var opens: ChapterId
    /// The picture: the series' banner where there is one, otherwise its cover.
    public var artURL: String?

    public init(identifier: String, work: WorkId, title: String, body: String, opens: ChapterId, artURL: String?) {
        self.identifier = identifier
        self.work = work
        self.title = title
        self.body = body
        self.opens = opens
        self.artURL = artURL
    }
}

public enum ChapterNotices {
    /// Every new-chapter notification's thread, which the system groups them by.
    public static let thread = "new-chapters"
    /// What every new-chapter notification's identifier starts with.
    public static let prefix = "ch."
    /// The category, whose action is Read.
    public static let category = "new-chapter"
    /// What a notification says when previews are hidden.
    public static var hiddenPreview: String { L.notifChapterHidden }

    /// The notification for `work`.
    public static func identifier(_ work: WorkId) -> String {
        "\(prefix)\(work)"
    }

    /// What the check found, one notification per series in the order the series first come:
    /// worded by its newest chapter, the one the source dates last.
    public static func plan(_ found: [NewChapter]) -> [ChapterNotice] {
        var order: [WorkId] = []
        var chapters: [WorkId: [NewChapter]] = [:]
        for chapter in found {
            if chapters[chapter.work] == nil { order.append(chapter.work) }
            chapters[chapter.work, default: []].append(chapter)
        }
        return order.compactMap { work in
            guard let all = chapters[work], let newest = newest(all) else { return nil }
            let body = all.count == 1 ? L.notifChapterNew(chapter: newest.label) : L.notifChaptersNew(count: all.count)
            return ChapterNotice(
                identifier: identifier(work), work: work, title: newest.title, body: body,
                opens: newest.nextForYou ?? newest.chapter, artURL: newest.banner ?? newest.cover)
        }
    }

    /// The chapter of `chapters` the source dates last; the first of them when none is dated.
    static func newest(_ chapters: [NewChapter]) -> NewChapter? {
        // `max(by:)` keeps the first of equals.
        chapters.max { ($0.published ?? 0) < ($1.published ?? 0) }
    }
}
