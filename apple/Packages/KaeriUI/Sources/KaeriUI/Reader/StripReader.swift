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
    @Environment(\.dynamicTypeSize) private var typeSize
    @ScaledMetric(relativeTo: .title2) private var markerHeight: CGFloat = 168
    @State private var position = ScrollPosition()
    @State private var offsetY: CGFloat = 0
    @State private var restoring = true
    /// A jump is scrolling: the places it passes are not reported, and where it lands goes to the
    /// engine as the person's jump.
    @State private var jumping = false
    /// The image a jump goes to, until the scroll reaches it.
    @State private var landing: Int?
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
    @State private var contentHeight: CGFloat = 0
    /// The row at the top of the screen, where it sat and how far into it the screen's top was.
    /// Rows above it that change height, images landing taller or shorter than their guess, move
    /// it, and the scroll follows it so what is on screen stays put.
    @State private var anchor: Anchor?
    @State private var phase: ScrollPhase = .idle
    /// The person's own last move went down the strip: only that passes the open chapter's
    /// marker. A scroll the strip makes itself, to follow rows that changed height, to restore a
    /// place or to land a jump, never does.
    @State private var goingOn = false
    /// The screen's height step the top of the screen is in. Rows a screen above it to two
    /// below are drawn; the rest is room of their height, so every row sits where `rows` puts it.
    @State private var band = 0

    struct Anchor: Equatable {
        var id: String
        var top: CGFloat
        /// How far into the row, from 0 to 1.
        var into: CGFloat
        /// How far past the row's foot, in points: the row above the lead holds a place in it.
        var past: CGFloat = 0
    }

    private struct Metrics: Equatable {
        var x: CGFloat
        var y: CGFloat
        var height: CGFloat
        /// The scroll's own height: images that land shorter than their guess shorten it under a
        /// place that stays put, and the place is worked out again then too, or the marker
        /// could come into view with nothing to open the next chapter.
        var content: CGFloat
    }

    /// One row of the scroll, where it sits and how tall it is, so the place in it is worked out
    /// without measuring.
    struct Row: Identifiable {
        enum Kind {
            case image(chapter: String, part: LeafPart)
            /// Credit pages the engine skipped where they were: a 44 point line while skipped,
            /// each image under its caption once brought back.
            case credits(chapter: String, pages: [CreditPage])
            /// Into `next`, after `chapter`, whose pages skipped at its end it names.
            case marker(next: ChapterRow, source: String?, chapter: String)
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
        var isLead: Bool { if case .lead = kind { true } else { false } }
    }

    private var parts: [LeafPart] { model.leaves.filter { !$0.endPage }.flatMap(\.parts) }
    private var chapter: String { model.view?.chapter.id ?? "" }
    private var next: ChapterRow? { model.endNext }
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
        // A chapter's images with its credit pages where the engine took them out: a group
        // before the image it sat before, and those at the end after the last image once shown.
        func images(_ id: String, _ list: [LeafPart], _ trail: Int?) {
            var groups = creditGroups(id, count: list.count)
            for part in list {
                for anchor in groups.keys.sorted() where anchor <= Int(part.page) {
                    let pages = groups.removeValue(forKey: anchor) ?? []
                    add("\(id)~\(anchor)", .credits(chapter: id, pages: pages), creditsHeight(pages, chapter: id), trail)
                }
                add("\(id)#\(part.page)", .image(chapter: id, part: part), height(of: part, chapter: id) + spacing, trail)
            }
            let end = groups.values.flatMap { $0 }.filter { model.isRevealed($0, chapter: id) }.sorted { $0.page < $1.page }
            if !end.isEmpty {
                add("\(id)~end", .credits(chapter: id, pages: end), creditsHeight(end, chapter: id), trail) // i18n-skip: a row id
            }
        }
        for (n, past) in model.trail.chapters.enumerated() {
            images(past.id, past.parts, n)
            add("\(past.id)>", .marker(next: past.next, source: past.nextSource, chapter: past.id), markerHeight(past.id), n)
        }
        if waiting {
            add("lead", .lead, leadHeight, nil) // i18n-skip: a row id
            return rows
        }
        guard !parts.isEmpty else { return rows }
        images(chapter, parts, nil)
        if model.flowFailure != nil {
            // The next chapter would not open: the end page takes the marker's and the lead's
            // room, so the strip keeps its length and nothing on screen moves.
            add("\(chapter)!", .foot, max(size.height * 0.8, 420, markerHeight(chapter) + leadHeight), nil)
        } else if let next {
            add("\(chapter)>", .marker(next: next, source: model.end?.nextSource, chapter: chapter), markerHeight(chapter), nil)
            add("lead", .lead, leadHeight, nil) // i18n-skip: a row id
        } else {
            add("\(chapter)!", .foot, max(size.height * 0.8, 420), nil)
        }
        return rows
    }

    /// A chapter's credit pages by the image they sat before; those at the end under the count.
    private func creditGroups(_ chapter: String, count: Int) -> [Int: [CreditPage]] {
        var groups: [Int: [CreditPage]] = [:]
        for credit in model.chapterCredits(chapter) {
            let anchor = model.anchor(of: credit, in: chapter)
            groups[credit.atEnd ? max(anchor, count) : anchor, default: []].append(credit)
        }
        return groups
    }

    /// The line while any of the group is skipped, and each page brought back under its caption.
    /// A page kept loses its caption at once; it becomes an image of the chapter with the engine's
    /// next layout.
    private func creditsHeight(_ pages: [CreditPage], chapter: String) -> CGFloat {
        var h: CGFloat = pages.contains { !model.isRevealed($0, chapter: chapter) } ? lineHeight : 0
        for credit in pages where model.isRevealed(credit, chapter: chapter) {
            h += (isKeeping(credit, chapter: chapter) ? 0 : lineHeight) + creditHeight(credit) + spacing
        }
        return h
    }

    /// The credit line and captions: 44 points, and room for two lines at accessibility sizes,
    /// where they wrap (they stop growing at accessibility1).
    private var lineHeight: CGFloat { typeSize.isAccessibilitySize ? 68 : 44 }

    private func isKeeping(_ credit: CreditPage, chapter: String) -> Bool {
        model.keeping.map { $0.chapter == chapter && $0.credit.page == credit.page } ?? false
    }

    private func creditHeight(_ credit: CreditPage) -> CGFloat {
        let w = CGFloat(credit.file.crop?.width ?? credit.file.width)
        let h = CGFloat(credit.file.crop?.height ?? credit.file.height)
        return w > 0 ? (column * h / w).rounded() : (column * 1.5).rounded()
    }

    /// The marker, with room for the line naming the pages skipped at the end of the chapter before it.
    private func markerHeight(_ chapter: String) -> CGFloat {
        markerHeight + (model.endSkipped(chapter).isEmpty ? 0 : 32)
    }

    var body: some View {
        let rows = self.rows
        let shown = drawn(rows)
        ScrollView(mac ? Axis.Set.vertical : [.vertical, .horizontal], showsIndicators: false) {
            // Not a lazy stack: it guesses the height of the rows it has not drawn, so a scroll to
            // a place far down showed another image than the one the place names.
            VStack(spacing: 0) {
                Color.clear.frame(height: shown.first?.top ?? 0)
                ForEach(shown) { row in
                    view(of: row).frame(height: row.height)
                }
            }
            // As wide as the screen while fitted, so it only scrolls sideways once zoomed.
            .frame(width: max(size.width, column), height: rows.last.map { $0.top + $0.height } ?? 0, alignment: .top)
        }
        .scrollBounceBehavior(.basedOnSize, axes: .horizontal)
        .scrollPosition($position)
        .onScrollGeometryChange(for: Metrics.self) { geometry in
            Metrics(x: geometry.contentOffset.x + geometry.contentInsets.leading,
                    y: geometry.contentOffset.y + geometry.contentInsets.top, height: geometry.containerSize.height,
                    content: geometry.contentSize.height)
        } action: { _, metrics in
            offsetX = metrics.x
            track(metrics)
        }
        .onScrollPhaseChange { _, phase in
            self.phase = phase
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
        // Scrolled past the marker of a chapter still landing: it flows on once its end is real.
        .onChange(of: model.stripSettled) { _, settled in
            if settled, goingOn { flowIfPast(y: offsetY, height: size.height) }
        }
        .onChange(of: model.stripRelaid) { _, _ in
            restoring = true
            restore()
        }
        // The line comes after the images; while the place is still being restored it moves with them.
        .onChange(of: model.chapterCredits(chapter).count) { _, _ in if restoring { restore() } }
        // Show from a marker or the end puts the pages back above it and scrolls to them.
        .onChange(of: model.revealed) { old, new in
            guard let key = new.subtracting(old).first,
                  model.chapterCredits(key.chapter).contains(where: { $0.page == key.page && $0.atEnd }),
                  let row = rows.first(where: { $0.id == "\(key.chapter)~end" }) else { return } // i18n-skip: a row id
            restoring = false
            withAnimation(instant ? nil : .standard) { position.scrollTo(y: row.top) }
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
            // A turn at the foot, with the marker already above the middle, opens the next
            // chapter: the scroll has nowhere to go, so no new place would.
            if request.forward, flowIfPast(y: offsetY, height: size.height) { return }
            // Never past the foot, where a scroll would not go at all.
            let end = contentHeight > size.height ? contentHeight - size.height : .infinity
            let to = min(max(0, offsetY + (request.forward ? step : -step)), end)
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
        // The zoom keeps the place under the fingers, not the top row.
        anchor = nil
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
        let trail = trailIndex
        let images = rows.filter { $0.isImage && $0.trail == trail }
        guard !images.isEmpty else { return }
        let image = min(max(index, 0), images.count - 1)
        goingOn = false
        jumping = true
        landing = image
        withAnimation(instant ? nil : .standard) {
            position.scrollTo(y: images[image].top)
        } completion: {
            jumping = false
            if trail == nil { model.shown(image, how: .jump) }
            // The scroll can report the place it left once more before it reaches the new one.
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(600))
                if landing == image { landing = nil }
            }
        }
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
        case .credits(let chapter, let pages):
            StripCredits(model: model, chapter: chapter, pages: pages, theme: theme, tone: tone, width: column,
                         heights: pages.map(creditHeight), spacing: spacing, scale: scale, lineHeight: lineHeight,
                         instant: instant)
                .frame(width: column, height: row.height, alignment: .top)
                .clipped()
        case .marker(let next, let source, let chapter):
            StripMarker(next: next, facts: Self.facts(next, source: source), theme: theme,
                        skipped: model.endSkipped(chapter).count) {
                withAnimation(instant ? nil : .standard) { model.showEndCredits(chapter) }
            }
            .frame(width: column)
        case .lead:
            ZStack(alignment: .top) {
                PagePlate(page: 1, theme: theme, atOnce: true, image: true)
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
        guard let at = model.stripStart, restoring, !parts.isEmpty else { return }
        let images = rows.filter { $0.isImage && $0.trail == nil }
        guard !images.isEmpty else { return }
        let row = images[min(Int(at.image), images.count - 1)]
        position.scrollTo(y: row.top + CGFloat(at.offset) * (row.height - spacing))
    }

    /// The open chapter's marker is above the middle of the screen: the next chapter opens under
    /// it. True when it did.
    @discardableResult private func flowIfPast(y: CGFloat, height: CGFloat) -> Bool {
        guard !restoring, model.stripSettled, let marker = rows.last(where: \.isMarker), marker.trail == nil,
              y + height / 2 > marker.top + marker.height / 2 else { return false }
        model.flowOn()
        guard model.flowing != nil else { return false }
        // Into the next chapter: passing its marker takes a move of the person's own again.
        goingOn = false
        return true
    }

    /// The rows in the band around the screen, in order.
    private func drawn(_ rows: [Row]) -> [Row] {
        let screen = max(size.height, 1)
        let from = CGFloat(band - 1) * screen, to = CGFloat(band + 3) * screen
        return rows.filter { $0.top + $0.height >= from && $0.top <= to }
    }

    /// Rows above the one at the top of the screen changed height: the scroll moves with it, at
    /// once, and the place is worked out again from there. A scroll under way, a restore or a
    /// jump goes on as it was. True when it moved.
    private func follow(_ rows: [Row], _ metrics: Metrics) -> Bool {
        if let held = anchor, phase == .idle, !restoring, !jumping, landing == nil,
           let to = Self.following(held, in: rows) {
            anchor = Self.anchor(in: rows, at: to)
            if abs(to - metrics.y) >= 1 {
                position.scrollTo(y: to)
                // The strip moved itself: the next report is not the person going on.
                offsetY = to
                return true
            }
        }
        anchor = Self.anchor(in: rows, at: metrics.y)
        return false
    }

    /// What holds the place at `y`: the row there and how far into it. Never the lead, which
    /// stands in for the next chapter's first image and moves under that chapter's images once
    /// they come: the row above it holds the place then, so the images take the lead's room
    /// rather than the strip running on past them.
    static func anchor(in rows: [Row], at y: CGFloat) -> Anchor? {
        guard let row = rows.last(where: { $0.top <= y && !$0.isLead }) else { return nil }
        let offset = y - row.top
        if offset > row.height { return Anchor(id: row.id, top: row.top, into: 1, past: offset - row.height) }
        return Anchor(id: row.id, top: row.top, into: offset / max(row.height, 1))
    }

    /// Where the strip scrolls to keep `anchor` in place in `rows`, when that row moved; nil
    /// when it has not moved or is gone.
    static func following(_ anchor: Anchor, in rows: [Row]) -> CGFloat? {
        guard let row = rows.first(where: { $0.id == anchor.id }), row.top != anchor.top else { return nil }
        return max(0, row.top + anchor.into * row.height + anchor.past)
    }

    /// Where the person is. The chapter whose rows hold the middle of the screen names the line,
    /// the image at the top is the place, and the open chapter's marker crossing the middle
    /// opens the next chapter under it.
    private func track(_ metrics: Metrics) {
        let before = offsetY
        offsetY = metrics.y
        // Which way the person last went. The strip's own scrolls set `offsetY` where they land,
        // so they show here as no move.
        if !restoring, !jumping, landing == nil {
            if metrics.y > before + 0.5 { goingOn = true } else if metrics.y < before - 0.5 { goingOn = false }
        }
        let step = Int((metrics.y / max(metrics.height, 1)).rounded(.down))
        if step != band { band = step }
        let rows = self.rows
        if follow(rows, metrics) { contentHeight = metrics.content; return }
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
            if let landing, at == landing, !jumping { self.landing = nil }
            if on == nil, !restoring, !waiting, !jumping, landing == nil {
                // Only once every image has its real height is the foot where the strip draws it.
                let foot = model.stripSettled
                    && images[images.count - 1].top + images[images.count - 1].height <= metrics.y + metrics.height
                // With the last image's foot in sight the chapter has been seen to its end: the
                // place is that image, which is what counts it as read, though short credit
                // pages above it may still hold the top of the screen.
                let place = foot ? images.count - 1 : at
                let row = images[place]
                let into = min(max((metrics.y - row.top) / max(row.height - spacing, 1), 0), 1)
                model.stripAt(StripPos(image: UInt32(place), offset: Float(into)), footVisible: foot)
            }
        }
        contentHeight = metrics.content
        if goingOn { flowIfPast(y: metrics.y, height: metrics.height) }
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
                if !painted { PagePlate(page: Int(part.page) + 1, theme: theme, image: true) }
                PageImage(file: file, longest: longest, tone: tone) { painted = true }
            } else if let failure = model.failure(of: part, chapter: chapter) {
                PageFailed(page: Int(part.page) + 1, failure: failure, theme: theme) { model.retry(page: part.page) }
            } else {
                PagePlate(page: Int(part.page) + 1, theme: theme, image: true)
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
    /// Pages skipped at the end of the chapter before, named under the facts with Show.
    var skipped = 0
    var show: () -> Void = {}

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(L.readerEndNext).font(.ui(13)).foregroundStyle(theme.haze)
            Text(next.title.map { L.readerChapterLineTitled(chapter: next.number ?? next.label, title: $0) } ?? ReaderChrome.chapterName(next.label))
                .font(.showTitle(size: 22, relativeTo: .title2)).fontWeight(.bold)
                .foregroundStyle(theme.silk)
                .lineLimit(3)
            if !facts.isEmpty {
                Text(facts).font(.ui(14)).foregroundStyle(theme.haze).padding(.top, 2)
            }
            if skipped > 0 {
                ReaderSkippedLine(count: skipped, theme: theme, show: show)
                    .padding(.top, 14)
            }
        }
        .padding(.horizontal, 24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        .background(theme.ground)
        // The facts read as one; the line keeps its own Show button.
        .accessibilityElement(children: skipped > 0 ? .contain : .combine)
    }
}

/// Credit pages in the flow of a strip. While skipped, one 44 point row on the ground names
/// them, and the whole row is Show. Shown, each image sits under its caption, Skipped as a
/// credit page with Not a credit page.
private struct StripCredits: View {
    let model: ReaderModel
    let chapter: String
    let pages: [CreditPage]
    let theme: ReaderTheme
    let tone: PageTone
    let width: CGFloat
    let heights: [CGFloat]
    let spacing: CGFloat
    let scale: CGFloat
    let lineHeight: CGFloat
    let instant: Bool

    private var skipped: [CreditPage] { pages.filter { !model.isRevealed($0, chapter: chapter) } }

    var body: some View {
        VStack(spacing: 0) {
            // The line and the first page's caption share one slot, so Show crossfades the line
            // into the caption in place, with no jump.
            ZStack {
                if !skipped.isEmpty {
                    line.transition(.opacity)
                } else if let first = pages.first, model.isRevealed(first, chapter: chapter), !keeping(first) {
                    caption(first).transition(.opacity)
                }
            }
            .frame(maxWidth: .infinity)
            .frame(height: skipped.isEmpty && headless ? 0 : lineHeight)
            ForEach(Array(pages.enumerated()), id: \.element.page) { n, credit in
                if model.isRevealed(credit, chapter: chapter) {
                    VStack(spacing: 0) {
                        if n > 0, !keeping(credit) { caption(credit).transition(.opacity) }
                        PageImage(file: credit.file, longest: max(width, heights[n]) * scale, tone: tone)
                            .frame(width: width, height: heights[n])
                            .clipped()
                            .padding(.bottom, spacing)
                            .accessibilityElement()
                            .accessibilityLabel(L.readerCreditPageLabel)
                            .accessibilityAddTraits(.isImage)
                    }
                    // The image fades in under the caption while the row grows.
                    .transition(.opacity.animation(instant ? nil : .fadeIn))
                }
            }
        }
        // Past accessibility1 the rows would crowd the strip; they wrap up to there.
        .dynamicTypeSize(...DynamicTypeSize.accessibility1)
    }

    /// The first page is being kept, so its caption has gone and the slot closes.
    private var headless: Bool {
        guard let first = pages.first else { return true }
        return keeping(first)
    }

    /// "1 credit page skipped  Show", the whole row a button.
    private var line: some View {
        Button {
            withAnimation(instant ? nil : .standard) { model.reveal(skipped, chapter: chapter) }
        } label: {
            (Text(L.readerCreditPagesSkipped(count: skipped.count)).font(.ui(13)).foregroundStyle(theme.haze)
                + Text(verbatim: "  ") // i18n-skip
                + Text(L.readerCreditShow).font(.ui(13, weight: .semibold)).foregroundStyle(theme.silk).underline())
                .multilineTextAlignment(.center)
                .padding(.horizontal, 16)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(theme.ground)
                .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityLabel(L.readerCreditPagesSkipped(count: skipped.count))
        .accessibilityHint(L.readerCreditShowHint)
    }

    private func keeping(_ credit: CreditPage) -> Bool {
        model.keeping.map { $0.chapter == chapter && $0.credit.page == credit.page } ?? false
    }

    private func caption(_ credit: CreditPage) -> some View {
        let words = Text(L.readerCreditShown).font(.ui(13)).foregroundStyle(theme.haze)
        let button = Button(L.readerCreditNot) {
            withAnimation(instant ? nil : .standard) { model.keepCredit(credit, chapter: chapter) }
        }
        .font(.ui(13, weight: .semibold)).foregroundStyle(theme.silk)
        .underline()
        .buttonStyle(.plain)
        .padding(.vertical, 12).contentShape(.rect).padding(.vertical, -12)
        .pointerHover()
        // On one line while it fits; at large sizes the button goes under the words.
        return ViewThatFits(in: .horizontal) {
            HStack(spacing: 6) { words.fixedSize(); button.fixedSize() }
            VStack(spacing: 2) { words.multilineTextAlignment(.center); button }
        }
        .padding(.horizontal, 16)
        .frame(maxWidth: .infinity)
        .frame(height: lineHeight)
        .background(theme.ground)
        .accessibilityElement(children: .contain)
    }
}
