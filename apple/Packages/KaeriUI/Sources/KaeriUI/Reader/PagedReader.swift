// Pages one at a time, or two as a spread. Swipes follow the finger, taps and keys crossfade,
// and Paper mode turns instantly with nothing in between.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct PagedReader: View {
    @Bindable var model: ReaderModel
    let theme: ReaderTheme
    let size: CGSize
    let scale: CGFloat
    let tone: PageTone
    let paper: Bool
    let onTap: (Double) -> Void
    @State private var drag: CGFloat = 0
    @State private var fading: Int?
    @State private var fade: Double = 1
    @State private var settling = false

    private var rtl: Bool { model.view?.direction == .rightToLeft }
    /// Which way a drag goes to move forward: right in right-to-left, left otherwise.
    private var sign: CGFloat { rtl ? 1 : -1 }
    private var current: Int { min(model.current, max(model.leaves.count - 1, 0)) }

    var body: some View {
        ZStack {
            if let under = fading, model.leaves.indices.contains(under) { leaf(under) }
            ForEach(visible, id: \.self) { index in
                leaf(index)
                    .offset(x: offset(of: index))
                    .opacity(index == current && fading != nil ? fade : 1)
            }
        }
        .frame(width: size.width, height: size.height)
        .clipped()
        .contentShape(.rect)
        .gesture(swipe)
        .simultaneousGesture(SpatialTapGesture().onEnded { tap($0.location.x / max(size.width, 1)) })
        .onAppear { model.position = (current, max(model.leaves.count, 1)) }
        .onChange(of: model.current) { _, _ in model.position = (current, max(model.leaves.count, 1)) }
        .onChange(of: model.leaves.count) { _, _ in model.position = (current, max(model.leaves.count, 1)) }
        .onChange(of: model.goTo?.tick) { _, _ in
            if let to = model.goTo?.leaf { model.shown(to, how: .jump) }
        }
        .onChange(of: model.jump?.tick) { _, _ in
            if let to = model.jump?.index, model.leaves.indices.contains(to) { model.shown(to, how: .jump) }
        }
        .onChange(of: model.turnRequest) { _, request in
            guard let request else { return }
            turn(forward: request.forward, how: .key)
        }
        .accessibilityScrollAction { edge in turn(forward: edge == .trailing || edge == .bottom, how: .turn) }
    }

    private var visible: [Int] {
        var all = [current]
        if drag != 0 || settling {
            if model.leaves.indices.contains(current + 1) { all.append(current + 1) }
            if model.leaves.indices.contains(current - 1) { all.append(current - 1) }
        }
        return all
    }

    private func offset(of index: Int) -> CGFloat {
        if index == current { return drag }
        return drag + (index > current ? -sign : sign) * size.width
    }

    @ViewBuilder private func leaf(_ index: Int) -> some View {
        if model.leaves.indices.contains(index) {
            let item = model.leaves[index]
            if item.endPage {
                ReaderEndPage(model: model, theme: theme, size: size)
            } else {
                LeafView(
                    model: model, leaf: item, size: size, theme: theme, tone: tone, scale: scale,
                    fitWidth: model.choice(.fit) == "width")
                    .background(Color.clear)
            }
        } else {
            Color.clear
        }
    }

    // MARK: Turning

    private func tap(_ x: Double) {
        let layout = model.choice(.tapLayout) ?? "edges"
        if model.chromeShown || layout == "off" { onTap(x); return }
        let edge = layout == "sides" ? 0.5 : 0.33
        if x < edge || (layout == "sides" && x >= 0.5) || x > 1 - edge {
            let leftSide = x < 0.5
            // Left is forward when the book runs right to left.
            turn(forward: leftSide == rtl, how: .turn)
        } else {
            onTap(x)
        }
    }

    private func turn(forward: Bool, how: TurnHow) {
        guard model.leaves.indices.contains(current) else { return }
        if forward, model.leaves[current].endPage {
            if let next = model.view?.next { model.openChapter(next.id) }
            return
        }
        let target = current + (forward ? 1 : -1)
        guard model.leaves.indices.contains(target) else { return }
        if paper {
            model.shown(target, how: how)
            return
        }
        let old = current
        fading = old
        fade = 0
        model.shown(target, how: how)
        withAnimation(.quick, completionCriteria: .logicallyComplete) {
            fade = 1
        } completion: {
            fading = nil
        }
    }

    private var swipe: some Gesture {
        DragGesture(minimumDistance: 14)
            .onChanged { value in
                guard !settling else { return }
                var x = value.translation.width
                let forward = x * sign > 0
                let target = current + (forward ? 1 : -1)
                let endForward = forward && model.leaves.indices.contains(current) && model.leaves[current].endPage
                if !model.leaves.indices.contains(target) || (endForward && model.view?.next == nil) { x /= 4 }
                drag = x
            }
            .onEnded { value in
                guard !settling else { return }
                let x = value.translation.width
                let flick = abs(value.velocity.width) > 450 && value.velocity.width * x > 0
                let commit = abs(x) > size.width * 0.25 || flick
                let forward = x * sign > 0
                let target = current + (forward ? 1 : -1)
                let endForward = forward && model.leaves.indices.contains(current) && model.leaves[current].endPage
                if commit, endForward {
                    drag = 0
                    if let next = model.view?.next { model.openChapter(next.id) }
                    return
                }
                guard commit, model.leaves.indices.contains(target) else {
                    withAnimation(paper ? nil : .standard) { drag = 0 }
                    return
                }
                if paper {
                    drag = 0
                    model.shown(target, how: .swipe)
                    return
                }
                settling = true
                withAnimation(.standard, completionCriteria: .logicallyComplete) {
                    drag = sign * (forward ? 1 : -1) * size.width
                } completion: {
                    var transaction = Transaction()
                    transaction.disablesAnimations = true
                    withTransaction(transaction) {
                        model.shown(target, how: .swipe)
                        drag = 0
                        settling = false
                    }
                }
            }
    }
}

// MARK: End page

/// The page after the last: 28 pt margins, the next chapter, and the one amber Continue.
struct ReaderEndPage: View {
    @Bindable var model: ReaderModel
    let theme: ReaderTheme
    let size: CGSize
    @Environment(\.readerClose) private var closeReader

    var body: some View {
        let chapter = model.view?.chapter.label ?? ""
        let next = model.end?.next ?? model.view?.next
        VStack(alignment: .leading, spacing: 0) {
            Spacer(minLength: 0)
            Text(L.readerEndOfChapter(chapter: chapter)).font(.ui(24, weight: .semibold)).foregroundStyle(theme.silk)
            Text(model.read ? L.readerEndRead : L.readerEndNotRead).font(.ui(15)).foregroundStyle(theme.haze)
                .padding(.top, 6)
            if let next {
                VStack(alignment: .leading, spacing: 4) {
                    Text(L.readerEndNext).font(.ui(13, weight: .semibold)).foregroundStyle(theme.haze).textCase(.uppercase)
                    Text(next.title ?? next.label).font(.ui(20, weight: .semibold)).foregroundStyle(theme.silk)
                    Text(detail(next)).font(.ui(15)).foregroundStyle(theme.haze)
                }
                .padding(.top, 28)
            } else if let why = model.end?.why {
                VStack(alignment: .leading, spacing: 6) {
                    Text(title(why, chapter)).font(.ui(20, weight: .semibold)).foregroundStyle(theme.silk)
                    if let body = body(why) { Text(body).font(.ui(15)).foregroundStyle(theme.haze) }
                }
                .padding(.top, 28)
            }
            Spacer(minLength: 0)
            if let next {
                Button(L.readerContinue) { model.openChapter(next.id) }
                    .buttonStyle(ReaderAmberStyle(theme: theme))
                Text(L.readerSwipeToContinue).font(.ui(13)).foregroundStyle(theme.haze)
                    .frame(maxWidth: .infinity).padding(.top, 12)
            }
            Button(L.readerBackToSeries) { closeReader() }
                .buttonStyle(ReaderPillStyle(theme: theme))
                .padding(.top, 12)
        }
        .padding(.horizontal, 28)
        .padding(.bottom, 72)
        .frame(width: size.width, height: size.height, alignment: .leading)
        .background(theme.page)
    }

    private func detail(_ next: ChapterRow) -> String {
        if let pages = next.pages { return L.readerEndNextDetail(chapter: next.label, pages: Int(pages)) }
        return next.label
    }

    private func title(_ why: NoNext, _ chapter: String) -> String {
        switch why {
        case .notOutYet: L.readerEndNotOut(chapter: chapter)
        case .caughtUp: L.readerEndCaughtUp
        case .finished: L.readerEndFinished
        case .noSourceHasIt: L.readerEndNoSource(chapter: chapter)
        case .offline: L.readerEndOffline(chapter: chapter)
        }
    }

    private func body(_ why: NoNext) -> String? {
        switch why {
        case .notOutYet: L.readerEndNotOutBody
        case .caughtUp: L.readerEndCaughtUpBody
        case .finished: L.readerEndFinishedBody
        case .noSourceHasIt: nil
        case .offline: L.readerEndOfflineBody
        }
    }
}
