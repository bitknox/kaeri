// The strip: every image of the chapter in one vertical scroll. Full width on phones, a centred
// column on iPad and Mac.

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
    @State private var position = ScrollPosition()
    @State private var offsetY: CGFloat = 0
    @State private var restoring = true

    private struct Metrics: Equatable {
        var y: CGFloat
        var atFoot: Bool
    }

    private var parts: [LeafPart] { model.leaves.filter { !$0.endPage }.flatMap(\.parts) }

    private var column: CGFloat {
        if !mac && sizeClass == .compact { return size.width }
        let base: CGFloat = mac ? 720 : (size.width > size.height ? 620 : 720)
        let factor = (model.amount(.stripWidth) ?? 0.6) / 0.6
        return min(size.width, base * factor)
    }

    private var spacing: CGFloat { gap == "small" ? 8 : 0 }

    private func height(of part: LeafPart) -> CGFloat {
        let chapter = model.view?.chapter.id ?? ""
        guard let file = model.file(of: part, chapter: chapter) else { return column * 1.5 }
        let w = CGFloat(file.crop?.width ?? file.width)
        let h = CGFloat(file.crop?.height ?? file.height)
        return w > 0 ? column * h / w : column * 1.5
    }

    private func top(of image: Int) -> CGFloat {
        parts.prefix(image).reduce(0) { $0 + height(of: $1) + spacing }
    }

    var body: some View {
        let chapter = model.view?.chapter.id ?? ""
        ScrollView(.vertical, showsIndicators: false) {
            LazyVStack(spacing: spacing) {
                ForEach(0..<parts.count, id: \.self) { index in
                    let part = parts[index]
                    let h = height(of: part)
                    ZStack {
                        if let file = model.file(of: part, chapter: chapter) {
                            PageImage(file: file, longest: max(column, h) * scale, tone: tone)
                        } else if let failure = model.failure(of: part, chapter: chapter) {
                            PageFailed(page: Int(part.page) + 1, failure: failure, theme: theme) { model.retry(page: part.page) }
                        } else {
                            PagePlate(page: Int(part.page) + 1, theme: theme)
                        }
                    }
                    .frame(width: column, height: h)
                    .clipped()
                    .onAppear { if model.file(of: part, chapter: chapter) == nil { model.needPage(part.page) } }
                }
                ReaderEndPage(model: model, theme: theme, size: CGSize(width: column, height: max(size.height * 0.8, 420)))
            }
            .frame(maxWidth: .infinity)
        }
        .scrollPosition($position)
        .onScrollGeometryChange(for: Metrics.self) { geometry in
            Metrics(
                y: geometry.contentOffset.y + geometry.contentInsets.top,
                atFoot: geometry.contentOffset.y + geometry.containerSize.height >= geometry.contentSize.height - 40)
        } action: { _, metrics in
            track(metrics)
        }
        .onScrollPhaseChange { _, phase in if phase == .interacting { restoring = false } }
        .simultaneousGesture(SpatialTapGesture().onEnded { _ in onTap() })
        .frame(width: size.width, height: size.height)
        .onAppear {
            model.position = (0, max(parts.count, 1))
            restore()
        }
        .onChange(of: parts.count) { _, _ in
            model.position = (model.position.index, max(parts.count, 1))
            restore()
        }
        .onChange(of: loaded) { _, _ in if restoring { restore() } }
        .onChange(of: model.stripRestore) { _, _ in restoring = true; restore() }
        .onChange(of: model.jump?.tick) { _, _ in
            guard let index = model.jump?.index else { return }
            restoring = false
            withAnimation(.standard) { position.scrollTo(y: top(of: min(max(index, 0), max(parts.count - 1, 0)))) }
        }
        .onChange(of: model.turnRequest) { _, request in
            guard let request else { return }
            let step = request.line ? 120 : size.height * 0.9
            restoring = false
            withAnimation(.standard) { position.scrollTo(y: max(0, offsetY + (request.forward ? step : -step))) }
        }
    }

    /// How many images have their pixels, so a late arrival can nudge the restore.
    private var loaded: Int {
        let chapter = model.view?.chapter.id ?? ""
        return parts.filter { model.file(of: $0, chapter: chapter) != nil }.count
    }

    private func restore() {
        let at = model.stripRestore ?? model.view?.start.strip
        guard let at, restoring, !parts.isEmpty else { return }
        let image = min(Int(at.image), parts.count - 1)
        position.scrollTo(y: top(of: image) + CGFloat(at.offset) * height(of: parts[image]))
    }

    private func track(_ metrics: Metrics) {
        offsetY = metrics.y
        var run: CGFloat = 0
        for (index, part) in parts.enumerated() {
            let h = height(of: part)
            if metrics.y < run + h + spacing || index == parts.count - 1 {
                let into = h > 0 ? min(max((metrics.y - run) / h, 0), 1) : 0
                model.position = (index, max(parts.count, 1))
                if !restoring { model.stripAt(StripPos(image: UInt32(index), offset: Float(into)), footVisible: metrics.atFoot) }
                return
            }
            run += h + spacing
        }
    }
}
