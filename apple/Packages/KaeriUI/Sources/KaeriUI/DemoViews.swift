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
            Text(L.demoSeriesFacts(pages: Int(demo.pages)).uppercased()) // i18n-skip
                .font(.ui(11, weight: .medium))
                .tracking(0.88)
                .foregroundStyle(.haze)
                .lineLimit(1)
                .accessibilityLabel(Text(L.demoSeriesFacts(pages: Int(demo.pages))))
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
                    VStack(alignment: .leading, spacing: 2) {
                        Text(info.series).font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk)
                        Text(L.demoStoryRow(author: info.author, licence: info.licence ?? ""))
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
                        if let link = URL(string: work.makerUrl) { Link(L.sourcesDemoLinkMaker, destination: link) }
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
