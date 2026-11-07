// The demo's parts that more than one screen shows: the credit block the licence asks for, and
// the About the demo detail with its links, the hosts it reaches and Remove demo.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The credit block, shown wherever the work shows as a title.
struct DemoCredit: View {
    @Environment(AppStore.self) private var store
    var size: CGFloat = 12

    var body: some View {
        let info = store.demo.info
        VStack(alignment: .leading, spacing: 4) {
            Text(L.sourcesDemoCredit)
                .font(.plat(mac: size, touch: size + 2))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 14) {
                link(L.sourcesDemoLinkLicence, info.licenceUrl)
                link(L.sourcesDemoLinkCommons, info.filePageUrl)
                link(L.sourcesDemoLinkMaker, info.makerUrl)
            }
        }
    }

    @ViewBuilder
    private func link(_ title: String, _ url: String) -> some View {
        if let url = URL(string: url) {
            Link(title, destination: url)
                .font(.plat(mac: size, touch: size + 2))
                .foregroundStyle(.water)
        }
    }
}

/// About the demo: what it is, the credit, the only hosts it reaches, and taking it out.
struct DemoAbout: View {
    @Environment(AppStore.self) private var store
    var size: CGFloat = 13

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(L.sourcesDemoAbout)
                .groupHeaderStyle()
                .accessibilityAddTraits(.isHeader)
            DemoCredit(size: size)
            Text(L.sourcesDemoHosts)
                .font(.plat(mac: size, touch: size + 2))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
            Text(L.sourcesDemoPrivate)
                .font(.plat(mac: size, touch: size + 2))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
            Button(L.sourcesDemoRemove, role: .destructive) { store.removeDemo() }
                .buttonStyle(SecondaryCapsuleStyle(height: ScreenFailed.buttonHeight, fontSize: ScreenFailed.buttonFont))
        }
    }
}

// MARK: - The reading demo

/// "Demo · Comic · 3 pages" for the comic, the story's maker and licence for the story: over the
/// title on its series page, and as its row's line under Also in the demo.
func demoFacts(_ work: DemoWorkInfo) -> String {
    work.novel ? L.demoStoryRow(author: work.author, licence: work.licence ?? "") : L.demoSeriesFacts(pages: Int(work.pages))
}

/// The credit under a demo work's title and in About the demo, worded from the engine's fields.
func demoCreditLine(_ work: DemoWorkInfo) -> String {
    work.novel
        ? L.demoCreditStory(title: work.series, author: work.author)
        : L.demoCreditBook(title: work.series, author: work.author, licence: work.licence ?? "")
}

/// The demo work's series page head: Demo, its kind and pages in small capitals, the title, and
/// the credit with a Licence link. No synopsis and no trackers.
struct DemoSeriesTitle: View {
    let series: SeriesView
    let demo: DemoWorkInfo
    let compact: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(demoFacts(demo).uppercased()) // i18n-skip
                .font(.ui(11, weight: .medium))
                .tracking(0.88)
                .foregroundStyle(.haze)
                .lineLimit(1)
                .accessibilityLabel(Text(demoFacts(demo)))
            Text(series.titles.display)
                .font(.showTitle(size: compact ? 22 : (Device.isMac ? 30 : 34), relativeTo: .largeTitle))
                .foregroundStyle(.silk)
                .lineLimit(3)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityAddTraits(.isHeader)
                .padding(.top, 8)
            (Text(demoCreditLine(demo)).foregroundStyle(.haze)
                + Text(" ")
                + licenceLink)
                .font(.plat(mac: 13, touch: 15))
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: 460, alignment: .leading)
                .padding(.top, 10)
        }
    }

    private var licenceLink: Text {
        guard let url = demo.licenceUrl, let link = URL(string: url) else { return Text("") }
        var text = AttributedString(L.sourcesDemoLinkLicence)
        text.link = link
        text.underlineStyle = .single
        return Text(text)
    }
}

/// Also in the demo: the other work, on its own row.
struct DemoAlso: View {
    @Environment(Navigator.self) private var navigator
    let id: String
    let info: DemoWorkInfo

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(L.demoAlso)
                .groupHeaderStyle()
                .accessibilityAddTraits(.isHeader)
            Button { navigator.openSeries(id) } label: {
                HStack(spacing: 12) {
                    BookCover(url: info.cover, width: Device.isMac ? 26 : 36)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(info.series).font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk)
                        Text(demoFacts(info))
                            .font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze)
                    }
                    Spacer(minLength: 8)
                    Image(systemName: "chevron.right").font(.ui(Device.isMac ? 11 : 13, weight: .semibold)).foregroundStyle(.haze)
                }
                .padding(.horizontal, 12).padding(.vertical, 10)
                .background(.raised, in: .rect(cornerRadius: Device.isMac ? 10 : 26))
                .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .pointerHover()
        }
    }
}

/// About the demo for the reading demo: what it is, the credits for each work, the hosts, what it
/// never does, and Remove demo.
struct DemoBooksAbout: View {
    @Environment(AppStore.self) private var store
    var size: CGFloat = 13

    var body: some View {
        let reading = store.demo.reading
        VStack(alignment: .leading, spacing: 12) {
            Text(L.sourcesDemoAbout).groupHeaderStyle().accessibilityAddTraits(.isHeader)
            line(L.sourcesDemoBooksWhat)
            Text(L.demoCredits).groupHeaderStyle().accessibilityAddTraits(.isHeader)
            ForEach(reading.works, id: \.id) { work in
                VStack(alignment: .leading, spacing: 4) {
                    line(demoCreditLine(work))
                    if let cover = work.coverCredit {
                        line(L.demoCreditCover(title: cover.title, artist: cover.artist, year: String(cover.year)))
                    }
                    HStack(spacing: 14) {
                        if let url = work.licenceUrl, let link = URL(string: url) {
                            Link(L.sourcesDemoLinkLicence, destination: link)
                        }
                        if let url = work.filePageUrl, let link = URL(string: url) {
                            Link(L.sourcesDemoLinkCommons, destination: link)
                        }
                    }
                    .font(.plat(mac: size, touch: size + 2))
                    .foregroundStyle(.water)
                }
            }
            line(L.sourcesDemoHosts)
            line(L.sourcesDemoBooksPrivate)
            Button(L.sourcesDemoRemove, role: .destructive) { store.removeDemo() }
                .buttonStyle(SecondaryCapsuleStyle(height: ScreenFailed.buttonHeight, fontSize: ScreenFailed.buttonFont))
        }
    }

    private func line(_ text: String) -> some View {
        Text(text)
            .font(.plat(mac: size, touch: size + 2))
            .foregroundStyle(.haze)
            .fixedSize(horizontal: false, vertical: true)
    }
}

/// Home's spotlight while the demo is the only thing to read: the comic's first panel across,
/// darkened from the foot, with Demo, free to share, its title and its line over it, and Read,
/// the one amber action. A banner of 300 on the Mac and iPad, 230 on the iPhone, as the board has
/// it, and dark in both appearances, as the art is.
struct DemoHomeSpotlight: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.homeLayout) private var layout

    var body: some View {
        if let comic = store.demo.comic {
            let height: CGFloat = layout.wide ? 300 : 230
            let inset: CGFloat = layout.wide ? 28 : 18
            ZStack(alignment: .bottomLeading) {
                Button { navigator.openSeries(comic.work) } label: {
                    panel(comic)
                        .overlay {
                            LinearGradient(stops: [
                                .init(color: Palette.ground.opacity(0.15), location: 0),
                                .init(color: Palette.ground.opacity(0.55), location: 0.4),
                                .init(color: Palette.ground.opacity(0.92), location: 1),
                            ], startPoint: .top, endPoint: .bottom)
                        }
                        .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .pointerHover()
                .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 0) {
                    Text(L.homeSpotlightDemo.uppercased()) // i18n-skip: the label's capitals
                        .font(.ui(11, weight: .medium))
                        .tracking(0.9)
                        .foregroundStyle(Palette.silk.opacity(0.8))
                        .accessibilityLabel(Text(L.homeSpotlightDemo))
                    Text(comic.series)
                        .font(.showTitle(size: layout.wide ? 30 : 24, relativeTo: .largeTitle))
                        .foregroundStyle(Palette.silk)
                        .lineLimit(2)
                        .accessibilityAddTraits(.isHeader)
                        .padding(.top, 4)
                    if !comic.synopsis.isEmpty {
                        Text(comic.synopsis)
                            .font(.plat(mac: 13, touch: 15))
                            .foregroundStyle(Palette.silk.opacity(0.85))
                            .lineLimit(2)
                            .padding(.top, 6)
                    }
                    Button(L.homeSpotlightRead) { store.reader.open(work: comic.work, at: .start) }
                        .buttonStyle(PrimaryCapsuleStyle(height: Device.isMac ? 28 : 44, fontSize: Device.isMac ? 13 : 17))
                        .fixedSize()
                        .padding(.top, 14)
                }
                .frame(maxWidth: 520, alignment: .leading)
                .padding(.leading, inset)
                .padding(.trailing, inset)
                .padding(.bottom, layout.wide ? 26 : 18)
            }
            .frame(maxWidth: .infinity, minHeight: height, maxHeight: height)
            .clipShape(.rect(cornerRadius: layout.wide ? 14 : 20))
            .accessibilityElement(children: .contain)
            .transition(.opacity)
        }
    }

    /// The cover is the comic's first page, three panels on white paper. The first panel alone
    /// fills the banner, so no paper margin or gutter shows.
    private func panel(_ comic: DemoWorkInfo) -> some View {
        GeometryReader { geo in
            let page = max(geo.size.width / 0.95, geo.size.height / 0.44)
            ArtworkView(url: comic.cover, maxPixelSize: Int(min(page * 2, 2_400)))
                .frame(width: page, height: page * 1.5)
                .position(x: geo.size.width / 2, y: geo.size.height / 2 - page * 0.28 + page * 0.75)
        }
        .clipped()
    }
}

/// The demo work's one chapter under Chapters: its number and title, its pages, and the reader
/// on a tap.
struct DemoChapter: View {
    @Environment(AppStore.self) private var store
    let work: String
    let demo: DemoWorkInfo
    let chapters: ReadChapters?

    var body: some View {
        let first = chapters?.volumes.flatMap(\.chapters).first
        VStack(alignment: .leading, spacing: 8) {
            Text(L.demoChapters)
                .groupHeaderStyle()
                .accessibilityAddTraits(.isHeader)
            Button { store.reader.open(work: work, chapter: first?.id) } label: {
                HStack(spacing: 12) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(L.readingChapterTitled(chapter: first?.number ?? "1", title: first?.title ?? demo.chapter)) // i18n-skip: the first chapter's number
                            .font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk)
                        if demo.pages > 0 {
                            Text(L.demoPages(pages: Int(demo.pages))).font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze)
                        }
                    }
                    Spacer(minLength: 8)
                    Image(systemName: "chevron.right").font(.ui(Device.isMac ? 11 : 13, weight: .semibold)).foregroundStyle(.haze)
                }
                .padding(.horizontal, 12).padding(.vertical, 10)
                .background(.raised, in: .rect(cornerRadius: Device.isMac ? 10 : 26))
                .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .pointerHover()
            .disabled(first == nil)
        }
        .frame(maxWidth: 720, alignment: .leading)
    }
}
