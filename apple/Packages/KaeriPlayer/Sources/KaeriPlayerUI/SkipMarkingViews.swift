// Skip intro on the video (skip-intro-and-invites board): the marking bar, the iPhone's side
// panel, the prompts in the corner, the sharing sheet and the Skip times panel. All of them
// draw what the engine says (`SkipMarkView`, `SkipPrompt`) and send the presses back.

import KaeriDesign
import KaeriKit
import KaeriStrings
import SwiftUI

/// Whether the marking view is the Mac and iPad's bar or the iPhone's side panel.
enum MarkLayout { case bar, panel }

/// The marking bar at the top of the video (720 pt, grown to the review in place), or on an
/// iPhone the 320 pt panel on the trailing edge.
struct SkipMarkBar: View {
    let model: PlayerModel
    let metrics: PlayerMetrics
    let layout: MarkLayout
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        if let view = model.skipMark {
            let review = view.step == .review
            Group {
                switch layout {
                case .bar: bar(view, review: review)
                case .panel: panel(view, review: review)
                }
            }
            .foregroundStyle(.silk)
            .font(.system(size: metrics.body))
            .overVideo(radius: layout == .panel ? 20 : (review ? 20 : 26))
            .animation(reduceMotion ? .quick : .standard, value: review)
            .accessibilityElement(children: .contain)
            .accessibilityLabel(L.playerMarkSkipTimes)
            .sensoryFeedback(.impact(weight: .light), trigger: model.skipSaves)
            .transition(.opacity)
        }
    }

    // MARK: The bar

    private func bar(_ view: SkipMarkView, review: Bool) -> some View {
        VStack(alignment: .leading, spacing: review ? 12 : 0) {
            HStack(spacing: 12) {
                kindPicker(view)
                    .frame(width: metrics.touch ? 250 : 230)
                Text(review ? L.playerMarkReviewTitle(kind: view.kind.title, length: SkipWords.span(view.length)) : SkipWords.step(view))
                    .font(.system(size: metrics.body, weight: review ? .semibold : .regular))
                    .monospacedDigit()
                    .lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .contentTransition(.opacity)
                if !review { markButton(view) } else { closeButton }
            }
            .frame(height: metrics.touch ? 40 : 32)
            if review {
                HStack(alignment: .top, spacing: 24) {
                    edgeRow(view, edge: .start)
                    edgeRow(view, edge: .end)
                }
                lines(view)
                HStack {
                    Button(L.playerMarkWatchEdges) { model.previewSkipEdges() }
                        .buttonStyle(PillButtonStyle(kind: .secondary, metrics: metrics))
                        .opacity(view.previewing ? 0.6 : 1)
                    Spacer(minLength: 0)
                    saveButton(view)
                }
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, review ? 14 : 10)
        .frame(width: 720)
    }

    // MARK: The iPhone's panel

    private func panel(_ view: SkipMarkView, review: Bool) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                kindPicker(view)
                closeButton
            }
            if review {
                Text(L.playerMarkReviewTitle(kind: view.kind.title, length: SkipWords.span(view.length)))
                    .font(.system(size: metrics.body, weight: .semibold))
                    .monospacedDigit()
                edgeRow(view, edge: .start)
                edgeRow(view, edge: .end)
                lines(view)
                HStack(spacing: 8) {
                    Button(L.playerMarkWatchEdges) { model.previewSkipEdges() }
                        .buttonStyle(PillButtonStyle(kind: .secondary, metrics: metrics))
                    Spacer(minLength: 0)
                    saveButton(view)
                }
            } else {
                Text(SkipWords.step(view)).foregroundStyle(.haze).fixedSize(horizontal: false, vertical: true)
                Text(PlayerClock.label(model.position))
                    .font(.system(size: 28, weight: .semibold))
                    .monospacedDigit()
                    .frame(maxWidth: .infinity)
                    .accessibilityLabel(PlayerClock.spoken(model.position))
                HStack(spacing: 6) {
                    ForEach([-5, -1, 1, 5], id: \.self) { step in
                        Button { model.nudgeSkipEdge(.start, by: Int64(step) * 1000) } label: {
                            Text(step > 0 ? "+\(step) s" : "\u{2212}\(-step) s").monospacedDigit().frame(maxWidth: .infinity)
                        }
                        .buttonStyle(PillButtonStyle(kind: .secondary, metrics: metrics))
                    }
                }
                markButton(view)
            }
        }
        .padding(14)
        .frame(width: 320)
        .sensoryFeedback(.selection, trigger: model.skipMark?.start)
    }

    // MARK: Parts

    private func kindPicker(_ view: SkipMarkView) -> some View {
        Picker(L.playerMarkSkipTimes, selection: Binding(get: { view.kind }, set: { model.setSkipMarkKind($0) })) {
            ForEach(SkipKind.marked, id: \.self) { Text($0.title).tag($0) }
        }
        .pickerStyle(.segmented)
        .labelsHidden()
    }

    private func markButton(_ view: SkipMarkView) -> some View {
        Button(view.step == .start ? L.playerMarkStart : L.playerMarkEnd) { model.markSkipEdge() }
            .buttonStyle(PillButtonStyle(kind: .primary, metrics: metrics))
            .keyboardShortcut(.defaultAction)
            .sensoryFeedback(.impact(weight: .light), trigger: view.start) { old, new in old != new }
    }

    private func saveButton(_ view: SkipMarkView) -> some View {
        Button(L.playerMarkSave) { model.saveSkipMark() }
            .buttonStyle(PillButtonStyle(kind: .primary, metrics: metrics))
            .keyboardShortcut(.defaultAction)
            .disabled(!view.canSave)
            .opacity(view.canSave ? 1 : 0.4)
    }

    private var closeButton: some View {
        Button { model.cancelSkipMark() } label: {
            Image(systemName: "xmark")
                .font(.system(size: metrics.touch ? 13 : 10, weight: .semibold))
                .frame(width: metrics.touch ? 32 : 22, height: metrics.touch ? 32 : 22)
                .contentShape(.circle)
        }
        .buttonStyle(OverVideoButtonStyle())
        .accessibilityLabel(L.actionClose)
    }

    /// "Starts 1:31 −1 s +1 s".
    private func edgeRow(_ view: SkipMarkView, edge: SkipEdge) -> some View {
        let time = (edge == .start ? view.start : view.end) ?? 0
        let name = edge == .start ? L.playerMarkEdgeStart : L.playerMarkEdgeEnd
        return HStack(spacing: 8) {
            Text(edge == .start ? L.playerMarkStarts : L.playerMarkEnds).foregroundStyle(.haze)
            Text(PlayerClock.label(time))
                .font(.system(size: metrics.body, weight: .semibold))
                .monospacedDigit()
                .contentTransition(.numericText())
                .animation(.quick, value: time)
                .frame(minWidth: 44, alignment: .leading)
            Spacer(minLength: 0)
            Button(L.playerMarkBack1) { model.nudgeSkipEdge(edge, by: -PlayerTiming.nudge) }
                .buttonStyle(PillButtonStyle(kind: .secondary, metrics: metrics))
                .accessibilityLabel(L.playerMarkBack1Label(edge: name))
            Button(L.playerMarkForward1) { model.nudgeSkipEdge(edge, by: PlayerTiming.nudge) }
                .buttonStyle(PillButtonStyle(kind: .secondary, metrics: metrics))
                .accessibilityLabel(L.playerMarkForward1Label(edge: name))
        }
        .padding(.leading, 2)
        .overlay(alignment: .leading) {
            if layout == .bar, model.markEdge == edge {
                Capsule().fill(.amber).frame(width: 2, height: 16).offset(x: -8)
            }
        }
    }

    @ViewBuilder
    private func lines(_ view: SkipMarkView) -> some View {
        let all = SkipWords.lines(view)
        if !all.isEmpty {
            VStack(alignment: .leading, spacing: 4) {
                ForEach(Array(all.enumerated()), id: \.offset) { _, line in
                    Text(line)
                        .font(.system(size: metrics.touch ? 13 : 12))
                        .foregroundStyle(view.check == .tooShort ? Palette.amber : Palette.haze)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
    }
}

// MARK: Prompts

/// "Was that the opening?" with Mark it and Don't ask again, or "Wrong opening times?" with
/// Report and Mark, in the corner for 8 s.
struct SkipPromptCard: View {
    let model: PlayerModel
    let metrics: PlayerMetrics

    var body: some View {
        if let offer = model.skipPrompt {
            switch offer.prompt {
            case .wasOpening:
                card(L.playerWasOpening, [
                    (L.playerDontAskAgain, .quiet, { model.neverAskWasOpening() }),
                    (L.playerMarkIt, .primary, { model.markFromPrompt() }),
                ])
            case .wrongTimes(let kind):
                card(L.playerSkipWrong(kind: kind.inline), [
                    (L.playerSkipReport, .secondary, { model.reportSkip(kind) }),
                    (L.playerSkipMark, .primary, { model.markFromPrompt() }),
                ])
            case .share:
                EmptyView()
            }
        }
    }

    private func card(_ title: String, _ buttons: [(String, PillButtonStyle.Kind, () -> Void)]) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title).font(.system(size: metrics.body, weight: .semibold))
            HStack(spacing: 8) {
                Spacer(minLength: 0)
                ForEach(Array(buttons.enumerated()), id: \.offset) { _, button in
                    Button(button.0, action: button.2)
                        .buttonStyle(PillButtonStyle(kind: button.1, metrics: metrics))
                }
            }
        }
        .foregroundStyle(.silk)
        .padding(12)
        .frame(width: 320)
        .overVideo(radius: metrics.cardRadius)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(title)
    }
}

/// "Share your skip times with AniSkip?", once, on the first Save.
struct SkipShareSheet: View {
    let model: PlayerModel
    let metrics: PlayerMetrics

    var body: some View {
        if case .share = model.skipPrompt?.prompt {
            ZStack {
                Palette.controlsDim.ignoresSafeArea()
                VStack(alignment: .leading, spacing: 0) {
                    Text(L.playerShareTitle).font(.system(size: 17, weight: .semibold))
                    Text(L.playerShareBody).foregroundStyle(.haze).padding(.top, 6)
                    Text(L.playerShareSent).font(.system(size: 13)).foregroundStyle(.haze).padding(.top, 8)
                    HStack(spacing: 8) {
                        Spacer(minLength: 0)
                        Button(L.playerShareNo) { model.answerShareSkipTimes(false) }
                            .buttonStyle(PillButtonStyle(kind: .secondary, metrics: .phone))
                        Button(L.playerShareYes) { model.answerShareSkipTimes(true) }
                            .buttonStyle(PillButtonStyle(kind: .primary, metrics: .phone))
                            .keyboardShortcut(.defaultAction)
                    }
                    .padding(.top, 16)
                }
                .font(.system(size: 15))
                .foregroundStyle(.silk)
                .padding(20)
                .frame(width: 380)
                .overVideo(radius: 24)
                .accessibilityElement(children: .contain)
            }
            .transition(.opacity)
        }
    }
}

// MARK: Skip times panel

/// More > Skip times: Opening, Ending and Recap with their times and where each came from.
struct SkipTimesPanel: View {
    let model: PlayerModel
    let metrics: PlayerMetrics

    static let width: CGFloat = 360

    var body: some View {
        if model.skipTimesOpen {
            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    Text(L.playerSkipTimes).font(.system(size: metrics.body, weight: .semibold))
                    Spacer(minLength: 0)
                    Button { model.closeSkipTimes() } label: {
                        Image(systemName: "xmark")
                            .font(.system(size: metrics.touch ? 13 : 10, weight: .semibold))
                            .frame(width: metrics.touch ? 32 : 22, height: metrics.touch ? 32 : 22)
                            .contentShape(.circle)
                    }
                    .buttonStyle(OverVideoButtonStyle())
                    .accessibilityLabel(L.actionClose)
                }
                ForEach(model.skipTimeRows) { row in
                    HStack(spacing: 8) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(row.kind.title)
                            Text([row.times, row.origin].compactMap { $0 }.joined(separator: " · "))
                                .font(.system(size: metrics.touch ? 13 : 12))
                                .monospacedDigit()
                                .foregroundStyle(.haze)
                        }
                        Spacer(minLength: 0)
                        if row.canReport {
                            Button(L.playerSkipReport) { model.reportSkip(row.kind) }
                                .buttonStyle(PillButtonStyle(kind: .quiet, metrics: metrics))
                        }
                        if row.canClear {
                            Button(L.playerSkipClear) { model.clearMySkip(row.kind) }
                                .buttonStyle(PillButtonStyle(kind: .quiet, metrics: metrics))
                        }
                        Button(L.playerSkipMark) { model.beginSkipMark(row.kind) }
                            .buttonStyle(PillButtonStyle(kind: .secondary, metrics: metrics))
                            .disabled(!model.canMarkSkips)
                    }
                }
                if model.marksBlocked {
                    Text(L.playerMarkUnavailable).font(.system(size: 12)).foregroundStyle(.haze)
                }
            }
            .font(.system(size: metrics.body))
            .foregroundStyle(.silk)
            .padding(14)
            .frame(width: Self.width)
            .overVideo(radius: 16)
            .accessibilityElement(children: .contain)
            .transition(.opacity)
        }
    }
}

/// The marking bar's chrome as the views place it: the bar centred under the title row, the
/// iPhone's panel on the trailing edge, then the prompts' sheet and the Skip times panel.
struct SkipMarkLayer: View {
    let model: PlayerModel
    let metrics: PlayerMetrics
    let layout: MarkLayout

    var body: some View {
        ZStack {
            SkipMarkBar(model: model, metrics: metrics, layout: layout)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: layout == .bar ? .top : .trailing)
                .padding(layout == .bar ? .top : .trailing, layout == .bar ? 64 : 8)
                .padding(.vertical, layout == .bar ? 0 : 60)
            SkipTimesPanel(model: model, metrics: metrics)
                .padding(.top, 60)
                .padding(.trailing, 16)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing)
            SkipShareSheet(model: model, metrics: metrics)
        }
        .animation(model.skipMark != nil ? .fadeIn : .fadeOut, value: model.skipMark != nil)
        .animation(model.skipTimesOpen ? .fadeIn : .fadeOut, value: model.skipTimesOpen)
        .animation(.fadeIn, value: model.skipPrompt?.prompt.isCard == false)
    }
}
