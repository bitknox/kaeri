// For shots, Search's sources' part in the discovery board's states, with made-up sources and
// series.

import KaeriKit
import Foundation

#if FIXTURES
/// The board's Search states, staged over the testing engine's AniList results by
/// `-KaeriSheet sources-<name>` (ReadingModel.stageSourceSearchShot):
///
/// - `mid`: Home library and Comics folder answered, Northwind still asking, Demo found nothing;
///   Home library joins AniList's first result.
/// - `failed`: as `mid`, with Northwind out of time.
/// - `only`: Your sources only, every source in.
/// - `nothing`: nothing from AniList or any source.
/// - `sourcehits`: AniList found nothing, Home library has one series.
/// - `anilistfailed`: AniList didn't answer; the sources were still asked.
enum SourceSearchShots {
    static let mid = "mid", failed = "failed", sourcesOnly = "only", nothing = "nothing"
    static let sourceHits = "sourcehits", anilistFailed = "anilistfailed"
    static let names: Set<String> = [mid, failed, sourcesOnly, nothing, sourceHits, anilistFailed]

    // Made-up sources. i18n-skip: fixture names
    private static let library: (String, String, ReadSourceKind?) = ("fixture-home-library", "Home library", .server) // i18n-skip
    private static let comics: (String, String, ReadSourceKind?) = ("fixture-comics-folder", "Comics folder", .folder) // i18n-skip
    private static let northwind: (String, String, ReadSourceKind?) = ("fixture-northwind", "Northwind", .plugin) // i18n-skip
    private static let demo: (String, String, ReadSourceKind?) = ("demo", "Demo", nil) // i18n-skip

    static func make(_ name: String, query: ReadSearchQuery, results: [WorkTile]) -> ReadSourceSearch {
        let only = name == sourcesOnly
        let anilist: ReadAnilistState = switch name {
        case sourcesOnly: .skipped
        case anilistFailed: .failed
        default: .answered
        }
        // With AniList's results to join, Home library's own volumes of the first one join it.
        let joinable = anilist == .answered && name != nothing && name != sourceHits
        let joins = joinable ? results.prefix(1).map {
            ReadSourceJoin(work: $0.work, sourceIds: [library.0], sourceNames: [library.1])
        } : []
        var groups: [ReadSourceGroup] = []
        switch name {
        case nothing:
            groups = [library, comics, northwind, demo].map { group($0, .nothing, []) }
        case sourceHits:
            groups = [
                group(library, .hits, [tile("kiln-extras", "Kiln Street Almanac, Extras", cover: 900309, .novel, .volumes(1))]), // i18n-skip
                group(comics, .nothing, []),
                group(northwind, .nothing, []),
                group(demo, .nothing, []),
            ]
        default:
            var library = libraryTiles
            if only || !joinable { library.insert(tile("lsn", "Lantern Street Notes", cover: 900302, .novel, .volumes(4)), at: 0) } // i18n-skip
            let northwindState: ReadSourceState = switch name {
            case mid: .asking
            case failed: .timedOut(seconds: 15)
            default: .hits
            }
            groups = [
                group(self.library, .hits, library, joined: joinable ? 1 : 0),
                group(comics, .hits, [tile("festival", "Lantern Festival Sketchbook", cover: 900307, .manga, .files(1, "CBZ"))]), // i18n-skip
                group(northwind, northwindState, northwindState == .hits ? northwindTiles : []),
                group(demo, .nothing, []),
            ]
        }
        return ReadSourceSearch(query: query, anilist: anilist, groups: groups, joins: joins, done: name != mid)
    }

    private static var libraryTiles: [WorkTile] {
        [
            tile("lsn-side", "Lantern Street Notes, Side Stories", cover: 900302, .novel, .volumes(2)), // i18n-skip
            tile("recipes", "Paper Lantern Recipes", cover: 900309, .manga, .volumes(3)), // i18n-skip
        ]
    }

    private static var northwindTiles: [WorkTile] {
        [
            tile("couriers", "Lantern Hill Couriers", cover: 900304, .manga, .chapters(18)), // i18n-skip
            tile("moth", "The Lantern Moth", cover: 900303, .manga, .episodes(40)), // i18n-skip
            tile("timetables", "Lantern Ferry Timetables", cover: nil, .novel, .parts(6)), // i18n-skip
            tile("diner", "Lanternfish Diner", cover: 900308, .manga, .episodes(22)), // i18n-skip
            tile("bridge", "Under the Lantern Bridge", cover: 900310, .manga, .chapters(9)), // i18n-skip
        ]
    }

    private static func group(
        _ source: (String, String, ReadSourceKind?), _ state: ReadSourceState, _ tiles: [WorkTile], joined: UInt32 = 0
    ) -> ReadSourceGroup {
        ReadSourceGroup(sourceId: source.0, sourceName: source.1, kind: source.2, state: state, tiles: tiles, joined: joined)
    }

    /// How much a source has of a series, as the source's row says it.
    private enum Amount {
        case volumes(Int64), chapters(Int64), episodes(Int64), parts(Int64), files(Int64, String)

        var msg: Msg {
            switch self {
            case .volumes(let n): .reading(.readingVolumes(count: n))
            case .chapters(let n): .reading(.readHitChapters(last: n))
            case .episodes(let n): .reading(.readHitEpisodes(last: n))
            case .parts(let n): .reading(.readHitParts(count: n))
            case .files(let n, let format): .reading(.readHitFilesFormat(count: n, format: format))
            }
        }
    }

    private static func tile(
        _ key: String, _ title: String, cover: Int?, _ medium: Medium, _ amount: Amount
    ) -> WorkTile {
        WorkTile(
            work: "local:fixture:\(key)", title: title, medium: medium, // i18n-skip
            cover: cover.map { "https://img.example/cover/\($0).jpg" }, // i18n-skip
            formatLabel: nil, progress: nil, next: nil, newChapter: false, adult: false,
            facts: [], demo: false, notInSources: false, origin: nil, amount: amount.msg)
    }
}
#endif
