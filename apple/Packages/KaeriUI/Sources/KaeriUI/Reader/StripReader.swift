// The strip: every image of the chapter in one vertical scroll. Full width on phones, a centred
// column on iPad and Mac. The next chapter follows in the same scroll after a slim marker, and
// the chapters already read stay above it.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct StripReader: View {
    @Bindable var model: ReaderModel
    let theme: ReaderTheme
    let mac: Bool
    let size: CGSize
    let tone: PageTone
    let onTap: () -> Void
    @AppStorage(ReaderDefaults.gap) private var gap = "none"
    @Environment(\.displayScale) private var scale
    @Environment(\.horizontalSizeClass) private var sizeClass
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.readerPaper) private var paper
    @ScaledMetric(relativeTo: .title2) private var markerHeight: CGFloat = 168
    @State private var position = ScrollPosition()
    @State private var offsetY: CGFloat = 0
    @State private var restoring = true
    /// The next chapter's first image, kept while the strip flows into it: opening it clears the
    /// model's previews before its own images come.
    @State private var lead: PageFile?
    @State private var ahead = Ahead()
    /// How far the strip is zoomed: the column is this many times its fitted width, and the
    /// strip scrolls sideways across it. iPhone and iPad only.
    @State private var zoom: CGFloat = 1
    /// A pinch under way, drawn over the strip until it ends and the column takes it.
    @State private var pinch: CGFloat = 1
    @State private var pinchAt: CGPoint = .zero
    @State private var pinching = false
    @State private var offsetX: CGFloat = 0

    private struct Metrics: Equatable {
        var x: CGFloat
        var y: CGFloat
        var height: CGFloat
    }

    /// One row of the scroll, where it sits and how tall it is, so the place in it is worked out
    /// without measuring.
    private struct Row: Identifiable {
        enum Kind {
            case image(chapter: String, part: LeafPart)
            case marker(next: ChapterRow, source: String?)
            /// Under the marker until the next chapter's images come: its first image when the
            /// engine made it, over the page plate.
            case lead
            case foot
        }

        let id: String
        let kind: Kind
        let top: CGFloat
        let height: CGFloat
        /// The trail chapter it belongs to; nil for the open one.
        let trail: Int?

        var isImage: Bool { if case .image = kind { true } else { false } }
        var isMarker: Bool { if case .marker = kind { true } else { false } }
    }

    private var parts: [LeafPart] { model.leaves.filter { !$0.endPage }.flatMap(\.parts) }
    private var chapter: String { model.view?.chapter.id ?? "" }
    private var next: ChapterRow? { model.end?.next ?? model.view?.next }
    /// The strip ran on and the next chapter's images are not here yet.
    private var waiting: Bool { model.flowing != nil || (!model.trail.chapters.isEmpty && parts.isEmpty) }
    /// Paper mode and Reduce Motion jump instead of scrolling.
    private var instant: Bool { paper || reduceMotion }

    private var column: CGFloat { fitColumn * zoom }

    /// The strip zooms in up to this.
    private static let most: CGFloat = 3

    private var fitColumn: CGFloat {
        if !mac && sizeClass == .compact { return size.width }
        let base: CGFloat = mac ? 720 : (size.width > size.height ? 620 : 720)
        let factor = (model.amount(.stripWidth) ?? 0.6) / 0.6
        return min(size.width, base * factor)
    }

    private var spacing: CGFloat { gap == "small" ? 8 : 0 }

    /// An image's height at the column's width, from the ratio the book gives before the file is
    /// made, so nothing moves when it lands.
    private func height(of part: LeafPart, chapter: String) -> CGFloat {
        if let ratio = part.ratio, ratio > 0 { return (column / CGFloat(ratio)).rounded() }
        if let file = model.file(of: part, chapter: chapter) {
            let w = CGFloat(file.crop?.width ?? file.width)
            let h = CGFloat(file.crop?.height ?? file.height)
            if w > 0 { return (column * h / w).rounded() }
        }
        return (column * 1.5).rounded()
    }

    private var leadHeight: CGFloat {
        guard let lead, lead.width > 0 else { return size.height }
        return max(size.height, (column * CGFloat(lead.height) / CGFloat(lead.width)).rounded())
    }

    private var rows: [Row] {
        var rows: [Row] = []
        var y: CGFloat = 0
        func add(_ id: String, _ kind: Row.Kind, _ h: CGFloat, _ trail: Int?) {
            rows.append(Row(id: id, kind: kind, top: y, height: h, trail: trail))
            y += h
        }
        for (n, past) in model.trail.chapters.enumerated() {
            for part in past.parts {
                add("\(past.id)#\(part.page)", .image(chapter: past.id, part: part), height(of: part, chapter: past.id) + spacing, n)
            }
            add("\(past.id)>", .marker(next: past.next, source: past.nextSource), markerHeight, n)
        }
        if waiting {
            add("lead", .lead, leadHeight, nil) // i18n-skip: a row id
            return rows
        }
        for part in parts {
            add("\(chapter)#\(part.page)", .image(chapter: chapter, part: part), height(of: part, chapter: chapter) + spacing, nil)
        }
        guard !parts.isEmpty else { return rows }
        if let next {
            add("\(chapter)>", .marker(next: next, source: model.end?.nextSource), markerHeight, nil)
            add("lead", .lead, leadHeight, nil) // i18n-skip: a row id
        } else {
            add("\(chapter)!", .foot, max(size.height * 0.8, 420), nil)
        }
        return rows
    }

    var body: some View {
        let rows = self.rows
        ScrollView(mac ? Axis.Set.vertical : [.vertical, .horizontal], showsIndicators: false) {
            LazyVStack(spacing: 0) {
                ForEach(rows) { row in
                    view(of: row).frame(height: row.height)
                }
            }
            // As wide as the screen while fitted, so it only scrolls sideways once zoomed.
            .frame(width: max(size.width, column))
        }
        .scrollBounceBehavior(.basedOnSize, axes: .horizontal)
        .scrollPosition($position)
        .onScrollGeometryChange(for: Metrics.self) { geometry in
            Metrics(x: geometry.contentOffset.x + geometry.contentInsets.leading,
                    y: geometry.contentOffset.y + geometry.contentInsets.top, height: geometry.containerSize.height)
        } action: { _, metrics in
            offsetX = metrics.x
            track(metrics)
        }
        .onScrollPhaseChange { _, phase in
            if phase == .interacting { restoring = false; model.hideChrome() }
        }
        #if os(iOS)
        .scaleEffect(pinch, anchor: UnitPoint(x: pinchAt.x / max(size.width, 1), y: pinchAt.y / max(size.height, 1)))
        .simultaneousGesture(
            SpatialTapGesture(count: 2).onEnded { tap in zoomByTap(at: tap.location) }
                .exclusively(before: SpatialTapGesture().onEnded { _ in onTap() })
        )
        .simultaneousGesture(
            MagnifyGesture()
                .onChanged { value in
                    if !pinching {
                        pinching = true
                        pinchAt = value.startLocation
                    }
                    // A little past each end while the fingers are down, so the limit is felt.
                    pinch = min(max(value.magnification, 0.85 / zoom), Self.most * 1.1 / zoom)
                }
                .onEnded { _ in
                    pinching = false
                    settle(to: zoom * pinch, at: pinchAt)
                }
        )
        #else
        .simultaneousGesture(SpatialTapGesture().onEnded { _ in onTap() })
        #endif
        .frame(width: size.width, height: size.height)
        .onAppear {
            model.position = (0, max(parts.count, 1))
            keepLead()
            restore()
        }
        .onChange(of: parts.count) { _, _ in
            if model.inTrail == nil { model.position = (model.position.index, max(parts.count, 1)) }
            restore()
        }
        .onChange(of: loaded) { _, _ in if restoring { restore() } }
        .onChange(of: model.view?.chapter.id) { _, _ in
            // Flowing on keeps the place: the new chapter's images take the lead's room.
            if !model.trail.chapters.isEmpty { restoring = false; return }
            // A chapter opened afresh starts fitted.
            zoom = 1
            pinch = 1
            lead = nil
            restoring = true
            position.scrollTo(y: 0)
        }
        .onChange(of: next.flatMap { model.previews[$0.id] }?.path) { _, _ in keepLead() }
        .onChange(of: model.stripRestore) { _, at in
            guard at != nil else { return }
            restoring = true
            restore()
        }
        .onChange(of: model.stripRelaid) { _, _ in
            restoring = true
            restore()
        }
        .onChange(of: model.jump?.tick) { _, _ in
            guard let index = model.jump?.index else { return }
            go(to: index)
        }
        .onChange(of: model.turnRequest) { _, request in
            guard let request else { return }
            // Up from the top of the strip goes into the chapter before, at its last image.
            if !request.forward, offsetY <= 1, model.inTrail == nil, let previous = model.view?.previous {
                model.openChapter(previous.id, atEnd: true)
                return
            }
            let step = request.line ? 120 : size.height * 0.9
            restoring = false
            let to = max(0, offsetY + (request.forward ? step : -step))
            withAnimation(instant ? nil : .standard) { position.scrollTo(y: to) }
        }
    }

    /// A double tap: in at the point, or back to fitted. The strip grows about the point and
    /// the column takes the zoom once it has.
    private func zoomByTap(at point: CGPoint) {
        let to: CGFloat = zoom > 1.01 ? 1 : 2
        pinchAt = point
        guard !instant else { return settle(to: to, at: point) }
        withAnimation(.standard, completionCriteria: .logicallyComplete) {
            pinch = to / zoom
        } completion: {
            settle(to: to, at: point)
        }
    }

    /// The column takes the zoom, and the strip scrolls so the place under the fingers stays put.
    private func settle(to wanted: CGFloat, at point: CGPoint) {
        let to = wanted < 1.05 ? 1 : min(wanted, Self.most)
        restoring = false
        let before = rows
        let oldColumn = column
        // The row under the point and how far into it, and how far across the column.
        let y = offsetY + point.y
        let x = offsetX + point.x - (max(size.width, oldColumn) - oldColumn) / 2
        let row = before.last { $0.top <= y }
        let down = row.map { (y - $0.top) / max($0.height, 1) } ?? 0
        let across = x / max(oldColumn, 1)
        var still = Transaction()
        still.disablesAnimations = true
        withTransaction(still) {
            zoom = to
            pinch = 1
        }
        let after = rows
        let newColumn = column
        let top = row.flatMap { row in after.first { $0.id == row.id } }.map { $0.top + down * $0.height } ?? y
        let left = (max(size.width, newColumn) - newColumn) / 2 + across * newColumn
        let wide = max(size.width, newColumn)
        position.scrollTo(point: CGPoint(x: min(max(left - point.x, 0), wide - size.width),
                                         y: max(top - point.y, 0)))
    }

    /// To an image of the chapter the line is on.
    private func go(to index: Int) {
        restoring = false
        let images = rows.filter { $0.isImage && $0.trail == trailIndex }
        guard !images.isEmpty else { return }
        let to = images[min(max(index, 0), images.count - 1)].top
        withAnimation(instant ? nil : .standard) { position.scrollTo(y: to) }
    }

    /// The trail chapter the line is on, by its place in the trail; nil for the open one.
    private var trailIndex: Int? {
        guard let inTrail = model.inTrail else { return nil }
        return model.trail.chapters.firstIndex { $0.id == inTrail.id }
    }

    @ViewBuilder private func view(of row: Row) -> some View {
        switch row.kind {
        case .image(let chapter, let part):
            StripImage(
                model: model, chapter: chapter, part: part, theme: theme, tone: tone,
                width: column, longest: max(column, row.height - spacing) * scale)
                .frame(width: column, height: row.height - spacing)
                .padding(.bottom, spacing)
                .onAppear {
                    if row.trail == nil, model.file(of: part, chapter: chapter) == nil { model.needPage(part.page) }
                }
        case .marker(let next, let source):
            StripMarker(next: next, facts: Self.facts(next, source: source), theme: theme)
                .frame(width: column)
        case .lead:
            ZStack(alignment: .top) {
                PagePlate(page: 1, theme: theme, atOnce: true)
                if let lead {
                    PageImage(file: lead, longest: leadHeight * scale, tone: tone)
                        .frame(width: column, height: column * CGFloat(lead.height) / CGFloat(max(lead.width, 1)))
                }
            }
            .frame(width: column)
            .clipped()
            .accessibilityHidden(true)
        case .foot:
            ReaderEndPage(model: model, theme: theme, size: CGSize(width: column, height: row.height))
        }
    }

    /// The marker's facts: "34 images · from books.home.example".
    static func facts(_ next: ChapterRow, source: String?) -> String {
        let images = next.pages.flatMap { $0 > 0 ? L.readerDividerImages(count: Int($0)) : nil }
        switch (images, source) {
        case (let images?, let source?): return L.readerDividerFactsSource(facts: images, source: source)
        case (nil, let source?): return L.readerEndFrom(source: source)
        case (let images?, nil): return images
        case (nil, nil): return ""
        }
    }

    /// How many images have their pixels, so a late arrival can nudge the restore.
    private var loaded: Int {
        parts.filter { model.file(of: $0, chapter: chapter) != nil }.count
    }

    private func keepLead() {
        guard let next, let file = model.previews[next.id] else { return }
        lead = file
    }

    private func restore() {
        let at = model.stripRestore ?? model.view?.start.strip
        guard let at, restoring, !parts.isEmpty else { return }
        let images = rows.filter { $0.isImage && $0.trail == nil }
        guard !images.isEmpty else { return }
        let row = images[min(Int(at.image), images.count - 1)]
        position.scrollTo(y: row.top + CGFloat(at.offset) * (row.height - spacing))
    }

    /// Where the person is. The chapter whose rows hold the middle of the screen names the line,
    /// the image at the top is the place, and the open chapter's marker crossing the middle
    /// opens the next chapter under it.
    private func track(_ metrics: Metrics) {
        offsetY = metrics.y
        let rows = self.rows
        let middle = metrics.y + metrics.height / 2
        let markers = rows.filter(\.isMarker)
        let on = markers.first { middle < $0.top + $0.height / 2 }?.trail
        let inTrail = on.map { model.trail.chapters[$0] }
        if model.inTrail?.id != inTrail?.id { model.inTrail = inTrail }
        let images = rows.filter { $0.isImage && $0.trail == on }
        if !images.isEmpty {
            let at = images.lastIndex { $0.top <= metrics.y } ?? 0
            if model.position.index != at || model.position.count != images.count {
                model.position = (at, images.count)
            }
            if on == nil, !restoring, !waiting {
                let foot = images[images.count - 1].top + images[images.count - 1].height <= metrics.y + metrics.height
                // With the last image's foot in sight the chapter has been seen to its end: the
                // place is that image, which is what counts it as read, though short credit
                // pages above it may still hold the top of the screen.
                let place = foot ? images.count - 1 : at
                let row = images[place]
                let into = min(max((metrics.y - row.top) / max(row.height - spacing, 1), 0), 1)
                model.stripAt(StripPos(image: UInt32(place), offset: Float(into)), footVisible: foot)
            }
        }
        if !restoring, let marker = markers.last, marker.trail == nil, middle > marker.top + marker.height / 2 {
            model.flowOn()
        }
        prepare(rows, from: metrics.y + metrics.height, to: metrics.y + metrics.height * 2)
    }

    /// The screen below this one, decoded before it scrolls in so it draws on its first frame with
    /// no fade, and its files asked for.
    private func prepare(_ rows: [Row], from top: CGFloat, to bottom: CGFloat) {
        for row in rows where row.top + row.height > top && row.top < bottom {
            guard case .image(let chapter, let part) = row.kind else { continue }
            guard let file = model.file(of: part, chapter: chapter) else {
                if row.trail == nil, !waiting { ahead.ask(row.id) { model.needPage(part.page) } }
                continue
            }
            ahead.decode(file, longest: max(column, row.height - spacing) * scale)
        }
    }
}

/// What the strip asked for ahead of the scroll, so each ask goes once.
@MainActor
private final class Ahead {
    private var asked: Set<String> = []

    func ask(_ id: String, _ ask: () -> Void) {
        guard asked.insert("ask " + id).inserted else { return } // i18n-skip: a cache key
        ask()
    }

    func decode(_ file: PageFile, longest: CGFloat) {
        guard PageImages.cached(file, longestSide: longest) == nil,
              asked.insert("decode \(file.path) \(Int(longest))").inserted // i18n-skip: a cache key
        else { return }
        Task { _ = await PageImages.load(file, longestSide: longest) }
    }
}

/// One image of the strip: the plate holds its place until the image has faded in over it.
private struct StripImage: View {
    let model: ReaderModel
    let chapter: String
    let part: LeafPart
    let theme: ReaderTheme
    let tone: PageTone
    let width: CGFloat
    let longest: CGFloat
    @State private var painted = false

    var body: some View {
        ZStack {
            if let file = model.file(of: part, chapter: chapter) {
                if !painted { PagePlate(page: Int(part.page) + 1, theme: theme) }
                PageImage(file: file, longest: longest, tone: tone) { painted = true }
            } else if let failure = model.failure(of: part, chapter: chapter) {
                PageFailed(page: Int(part.page) + 1, failure: failure, theme: theme) { model.retry(page: part.page) }
            } else {
                PagePlate(page: Int(part.page) + 1, theme: theme)
            }
        }
        .frame(width: width)
        .clipped()
    }
}

/// The slim marker into the next chapter: Next, its title and its facts, on the ground. It
/// scrolls past like an image.
private struct StripMarker: View {
    let next: ChapterRow
    let facts: String
    let theme: ReaderTheme

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(L.readerEndNext).font(.ui(13)).foregroundStyle(theme.haze)
            Text(next.title.map { L.readerChapterLineTitled(chapter: next.number ?? next.label, title: $0) } ?? next.label)
                .font(.showTitle(size: 22, relativeTo: .title2)).fontWeight(.bold)
                .foregroundStyle(theme.silk)
                .lineLimit(3)
            if !facts.isEmpty {
                Text(facts).font(.ui(14)).foregroundStyle(theme.haze).padding(.top, 2)
            }
        }
        .padding(.horizontal, 24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        .background(theme.ground)
        .accessibilityElement(children: .combine)
    }
}
