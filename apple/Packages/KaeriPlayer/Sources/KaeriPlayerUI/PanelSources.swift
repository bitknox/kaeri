// The panel's Sources tab: what plays now and its streams, the sub and dub switch, and every
// other source in the order set in Sources, each with this episode's releases in the order the
// source gave them. A pick plays from the playhead, for this episode only; the row it was picked
// on says how the switch goes.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct PanelSources: View {
    let model: PlayerModel
    let metrics: PanelMetrics
    @Environment(AppStore.self) private var store
    /// The sources whose releases are shown.
    @State private var expanded: Set<String> = []

    var body: some View {
        let id = model.staged?.id
        ScrollViewReader { proxy in
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                PanelHeading(title: L.playerPanelPlayingNow, metrics: metrics)
                    .id(Self.top)
                playing
                translation
                Rectangle()
                    .fill(Palette.glassRule)
                    .frame(height: 1)
                    .padding(.horizontal, metrics.rowPad)
                    .padding(.top, 10)
                    .padding(.bottom, 4)
                PanelHeading(title: L.playerPanelOtherSources, metrics: metrics)
                    .id(Self.others)
                others
                PanelText(
                    text: "\(L.playerPanelOrder) \(L.playerPanelPickScope)", size: metrics.detail,
                    line: metrics.detailLine, color: Palette.haze)
                    .padding(.horizontal, metrics.rowPad)
                    .padding(.top, 10)
                    .padding(.bottom, 4)
                    // Under the rows as they come in at once, never sliding over them as they fade in.
                    .transaction(value: model.panelReleases?.rows == nil) { $0.animation = nil }
            }
            .padding(.horizontal, metrics.pad)
            .padding(.bottom, metrics.pad)
            .animation(.standard, value: expanded)
            .animation(.standard, value: model.playingSource)
            .animation(.standard, value: model.playingRelease)
            .animation(.fadeIn, value: model.panelReleases)
        }
        .scrollIndicators(.hidden)
        .onChange(of: model.playingRelease) { _, landed in
            // A switch landed: what plays now is at the top.
            guard landed != nil else { return }
            withAnimation(.standard) { proxy.scrollTo(Self.top, anchor: .top) }
        }
        .onChange(of: model.panelShot, initial: true) { _, shot in
            // A shot's row at the top as the tab opens, before anything moves.
            if let row = shot?.scroll {
                Task { @MainActor in proxy.scrollTo(row, anchor: .top) }
            }
            // A shot's opened source at the top, its releases under it.
            guard let open = shot?.expanded else { return }
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(600))
                withAnimation(.standard) { proxy.scrollTo(open, anchor: .top) }
            }
        }
        }
        .task(id: id) {
            guard let id else { return }
            store.loadSourceOptions(id)
            model.loadPanelReleases()
        }
        .onChange(of: model.panelShot, initial: true) { _, shot in
            guard let shot else { return }
            expanded = shot.expanded.map { [$0] } ?? []
        }
        .onChange(of: model.panelSwitch?.target.providerId) { _, provider in
            // A release picked elsewhere, or tried again, shows on its row.
            guard let provider, !provider.isEmpty, model.panelSwitch?.target.release != nil else { return }
            expanded.insert(provider)
        }
    }

    private static let top = "playing now"
    /// The Other sources heading's id.
    static let others = "other sources"

    // MARK: Playing now

    /// The source's own line: its coverage of the title, or why it is resting.
    private var playingOption: SourceOption? {
        guard let id = model.staged?.id, let options = store.sourceChoices.options[id] else { return nil }
        if let release = model.playingRelease {
            return options.first { $0.providerId == release.providerId }
        }
        guard let name = model.playingSource else { return nil }
        return options.first { $0.name == name }
    }

    /// The source playing, by its id, to leave it out of the others.
    private var playingProvider: String? {
        model.playingRelease?.providerId ?? playingOption?.providerId
    }

    @ViewBuilder
    private var playing: some View {
        let option = playingOption
        let name = model.playingSource ?? option?.name
        if let name {
            VStack(alignment: .leading, spacing: 1) {
                PanelText(text: name, size: metrics.title + 1, line: metrics.titleLine + 2, weight: .semibold)
                if let line = option.map(Self.line) {
                    PanelText(text: line, size: metrics.detail, line: metrics.detailLine, color: Palette.haze)
                        .monospacedDigit()
                }
            }
            .padding(.horizontal, metrics.rowPad)
            .padding(.top, 2)
            .padding(.bottom, 4)
            .accessibilityElement(children: .combine)
        }
        if let release = model.playingRelease {
            PanelChoiceRow(
                model: model, metrics: metrics, title: release.title, detail: ReleaseWords.detail(release),
                target: SwitchTarget(providerId: release.providerId, name: release.providerName, release: release.id),
                on: true, sub: true, action: nil)
        } else {
            ForEach(model.menus?.sources ?? [], id: \.id) { stream in
                PanelChoiceRow(
                    model: model, metrics: metrics, title: stream.label, detail: stream.health ?? stream.detail,
                    target: SwitchTarget(providerId: "", name: name ?? "", stream: stream.id),
                    on: stream.current,
                    action: stream.current ? nil : { model.switchTo(stream: stream) })
            }
        }
    }

    // MARK: Sub or dub

    /// The subtitled or dubbed switch. A pick moves the fill at once; while the engine finds that
    /// version the episode plays on, the picked segment shows a spinner from 600 ms and the line
    /// under it counts from 3 s. A failure moves the fill back and says so under the switch.
    private var translation: some View {
        let id = model.staged?.id
        let dub = id.flatMap { store.titles.entries[$0]?.dub }
        let episode = PanelEpisodes.current(model)
        let number = episode.flatMap { Double($0) }
        // The dub does not reach this episode yet.
        let pastDub: Bool = {
            guard let dub, !dub.complete, let through = dub.through, let number else { return false }
            return number > Double(through)
        }()
        let current = model.menus?.translation ?? .sub
        let pending = model.versionSwitch.flatMap { $0.failed == nil ? $0 : nil }
        let shown = pending?.to ?? current
        let pick = Binding<KaeriKit.Translation>(
            get: { shown },
            set: { picked in
                guard picked != shown, !(picked == .dub && pastDub) else { return }
                model.switchTo(version: picked)
            })
        var notes: [String] = []
        if let id, let line = store.titles.dubLines[id] { notes.append(line) }
        if store.party.inParty { notes.append(L.partyVersionOwn) }
        return VStack(alignment: .leading, spacing: 8) {
            Picker(L.playerTranslation, selection: pick) {
                Text(L.playerStartSubtitled).tag(KaeriKit.Translation.sub)
                Text(L.audioDubbed).tag(KaeriKit.Translation.dub)
                    .selectionDisabled(pastDub)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .controlSize(metrics.touch ? .large : .regular)
            .buttonSizing(.flexible)
            .frame(maxWidth: .infinity)
            .overlay { VersionSpinner(pending: pending, metrics: metrics) }
            TimelineView(.periodic(from: .now, by: 1)) { _ in
                let finding = pending.flatMap { $0.note(episode: episode ?? "", at: ContinuousClock.now) }
                let lines = notes + (finding.map { [$0] } ?? [])
                if !lines.isEmpty {
                    VStack(alignment: .leading, spacing: 2) {
                        ForEach(lines, id: \.self) { note in
                            PanelText(text: note, size: metrics.detail, line: metrics.detailLine, color: Palette.haze)
                                .transition(.opacity)
                        }
                    }
                    .monospacedDigit()
                    .animation(.fadeIn, value: finding != nil)
                }
            }
            if let failed = model.versionSwitch?.failed {
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Image(systemName: "exclamationmark.triangle")
                        .font(.system(size: metrics.detail, weight: .medium))
                    PanelText(text: failed, size: metrics.detail, line: metrics.detailLine, color: Palette.vermilion)
                }
                .foregroundStyle(.vermilion)
                .transition(.opacity.animation(.fadeIn))
                .accessibilityElement(children: .combine)
            }
        }
        .padding(.horizontal, metrics.rowPad)
        .padding(.top, 8)
        .padding(.bottom, 4)
        .animation(.fadeIn, value: model.versionSwitch?.failed)
    }

    // MARK: Other sources

    /// Every other source in the order set in Sources, or, before their lines come, the releases'
    /// own sources in the order the engine found them. While the tab loads, the sources' rows as
    /// the engine last said them, in the same order.
    private var groups: [(providerId: String, name: String, option: SourceOption?)] {
        let skip = playingProvider
        let options = model.staged.flatMap { store.sourceChoices.options[$0.id] }
        if let rows = model.panelReleases?.rows {
            return rows.filter { $0.providerId != skip && $0.name != model.playingSource }.map { row in
                (row.providerId, row.name, options?.first { $0.providerId == row.providerId })
            }
        }
        if let options {
            return options.filter { $0.providerId != skip }.map { ($0.providerId, $0.name, $0) }
        }
        var seen: Set<String> = []
        return (model.panelReleases?.candidates ?? []).compactMap { candidate in
            guard candidate.providerId != skip, seen.insert(candidate.providerId).inserted else { return nil }
            return (candidate.providerId, candidate.providerName, nil)
        }
    }

    @ViewBuilder
    private var others: some View {
        // Asked and not answered whole yet, or answering row by row: each source's own line.
        if let why = model.panelReleases?.failed {
            // No source was asked: why, in place of the rows.
            PanelText(text: why, size: metrics.detail, line: metrics.detailLine, color: Palette.haze)
                .padding(.horizontal, metrics.rowPad)
                .frame(maxWidth: .infinity, minHeight: metrics.choice, alignment: .leading)
                .transition(.opacity)
        } else if let releases = model.panelReleases, releases.rows != nil || releases.candidates == nil {
            loadingRows(releases)
        } else {
            answeredRows
        }
    }

    /// Every other source's row while the tab loads, from the first frame and in its order, its line
    /// as the engine last said it: nothing for 600 ms, then Asking and the seconds, then its answer
    /// crossfaded in place. A source can be picked once it has listed releases.
    @ViewBuilder
    private func loadingRows(_ releases: PanelReleases) -> some View {
        ForEach(groups, id: \.providerId) { group in
            let row = releases.rows?.first { $0.providerId == group.providerId }
            let listed: [SourceCandidate] = if case .listed(let listed)? = row?.state { listed } else { [] }
            let open = expanded.contains(group.providerId)
            // A source that listed releases now: its coverage, never an older note of it resting or failing.
            let coverage = group.option.flatMap { $0.note == nil ? Self.line($0) : nil }
            let line = coverage ?? listed.first.flatMap { listed.count == 1 ? ReleaseWords.detail($0) : nil }
            let loading = row.map { releases.loading($0, listed: line) } ?? RowLoading(asked: releases.asked, since: releases.asked)
            let dim: Bool = switch row?.state {
            case .missed?, .off?: true
            default: false
            }
            PanelChoiceRow(
                model: model, metrics: metrics, title: group.name,
                target: SwitchTarget(providerId: group.providerId, name: group.name, release: listed.count == 1 ? listed.first?.id : nil),
                dim: dim, loading: loading,
                disclosure: listed.count > 1 ? (L.playerPanelReleases(count: listed.count), open) : nil,
                action: pick(group.providerId, listed: listed, open: open))
                .id(group.providerId)
            if listed.count > 1, open {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(listed, id: \.id) { release in
                        PanelChoiceRow(
                            model: model, metrics: metrics, title: release.title, detail: ReleaseWords.detail(release),
                            target: SwitchTarget(providerId: release.providerId, name: group.name, release: release.id),
                            sub: true, inset: true, action: { model.switchTo(release) })
                    }
                }
                .transition(.opacity)
            }
        }
    }

    /// What a click on a source's row does once it has listed: its releases opened or closed, or
    /// its one release played. Nothing before then.
    private func pick(_ providerId: String, listed: [SourceCandidate], open: Bool) -> (() -> Void)? {
        if listed.count > 1 {
            return { if open { expanded.remove(providerId) } else { expanded.insert(providerId) } }
        }
        guard let release = listed.first else { return nil }
        return { model.switchTo(release) }
    }

    @ViewBuilder
    private var answeredRows: some View {
        ForEach(groups, id: \.providerId) { group in
            let releases = model.panelReleases?.of(group.providerId) ?? []
            let open = expanded.contains(group.providerId)
            let resting = group.option?.note != nil
            let line = group.option.map(Self.line)
            if releases.count > 1, !resting {
                PanelChoiceRow(
                    model: model, metrics: metrics, title: group.name, detail: line,
                    target: SwitchTarget(providerId: group.providerId, name: group.name),
                    disclosure: (L.playerPanelReleases(count: releases.count), open),
                    action: {
                        if open { expanded.remove(group.providerId) } else { expanded.insert(group.providerId) }
                    })
                    .id(group.providerId)
                if open {
                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(releases, id: \.id) { release in
                            PanelChoiceRow(
                                model: model, metrics: metrics, title: release.title, detail: ReleaseWords.detail(release),
                                target: SwitchTarget(providerId: release.providerId, name: group.name, release: release.id),
                                sub: true, inset: true, action: { model.switchTo(release) })
                        }
                    }
                    .transition(.opacity)
                }
            } else if let release = releases.first, !resting {
                // One release: the source's row plays it.
                PanelChoiceRow(
                    model: model, metrics: metrics, title: group.name, detail: line ?? ReleaseWords.detail(release),
                    target: SwitchTarget(providerId: release.providerId, name: group.name, release: release.id),
                    action: { model.switchTo(release) })
                    .id(group.providerId)
            } else {
                // Resting, or nothing for this episode: said, and nothing to pick.
                PanelChoiceRow(
                    model: model, metrics: metrics, title: group.name, detail: line,
                    target: SwitchTarget(providerId: group.providerId, name: group.name),
                    dim: true, action: nil)
                    .id(group.providerId)
            }
        }
    }

    /// A source's line: why it is resting, or how much of the title it has.
    static func line(_ option: SourceOption) -> String {
        switch option.note {
        case .off: L.sourceNoteOff
        case .resting: L.sourceNoteResting
        case .gone: L.sourceNoteGone
        case .ambiguous: L.sourceNoteAmbiguous
        case .failed(let reason): reason
        case nil: L.sourcePickerCoverage(covered: Int(option.covered), of: Int(option.of))
        }
    }
}

/// One stream, source or release: the check column, the name and its line, and a disclosure for a
/// source with several releases. A switch picked on it shows here: nothing for 600 ms, a spinner,
/// then "Starting · 3 s"; a failure says so in vermilion with Try again.
struct PanelChoiceRow: View {
    let model: PlayerModel
    let metrics: PanelMetrics
    let title: String
    var detail: String?
    let target: SwitchTarget
    var on = false
    /// A release's row: lighter text, whole titles.
    var sub = false
    /// Under its source, past the check column.
    var inset = false
    var dim = false
    /// While the Sources tab loads, the source's line: a spinner in the check column and Asking
    /// while it is asked, then its answer, each change crossfaded in place.
    var loading: RowLoading?
    var disclosure: (count: String, open: Bool)?
    let action: (() -> Void)?
    @State private var hovering = false

    var body: some View {
        let picked = model.panelSwitch.flatMap { $0.on(target) ? $0 : nil }
        content(picked)
            .padding(.vertical, metrics.touch ? 8 : 5)
            .padding(.leading, (metrics.touch ? metrics.rowPad - 6 : metrics.rowPad - 4) + (inset ? metrics.check : 0))
            .padding(.trailing, metrics.rowPad)
            .frame(maxWidth: .infinity, minHeight: metrics.choice, alignment: .leading)
            .background(
                hovering && action != nil ? Palette.silk.opacity(0.06) : .clear,
                in: RoundedRectangle(cornerRadius: metrics.rowRadius, style: .continuous))
            .contentShape(.rect)
            .onTapGesture { action?() }
            #if os(macOS)
            .onHover { hovering = $0 }
            #endif
            .pointingHand(action != nil)
            .accessibilityElement(children: .combine)
            .accessibilityAddTraits(action != nil ? (on ? [.isButton, .isSelected] : .isButton) : (on ? .isSelected : []))
            .accessibilityAction { action?() }
    }

    @ViewBuilder
    private func content(_ picked: PanelSwitch?) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: metrics.touch ? 6 : 4) {
                check(picked)
                VStack(alignment: .leading, spacing: 1) {
                    PanelText(
                        text: title, size: sub ? metrics.detail + 1 : metrics.title,
                        line: sub ? metrics.detailLine + 2 : metrics.titleLine, weight: sub ? .regular : .semibold,
                        color: dim ? Palette.haze : Palette.silk)
                    line(picked)
                }
                if let disclosure {
                    HStack(spacing: 4) {
                        Text(disclosure.count)
                        Image(systemName: "chevron.right")
                            .font(.system(size: metrics.detail - 1, weight: .semibold))
                            .rotationEffect(.degrees(disclosure.open ? 90 : 0))
                    }
                    .font(.system(size: metrics.detail))
                    .foregroundStyle(.haze)
                    .fixedSize()
                    .transition(.opacity)
                }
            }
            if let failed = picked?.failed {
                VStack(alignment: .leading, spacing: 6) {
                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                        Image(systemName: "exclamationmark.triangle")
                            .font(.system(size: metrics.detail, weight: .medium))
                        PanelText(text: failed, size: metrics.detail, line: metrics.detailLine, color: Palette.vermilion)
                    }
                    .foregroundStyle(.vermilion)
                    PanelButton(title: L.actionTryAgain, kind: .secondary, metrics: metrics) { model.retrySwitch() }
                }
                .padding(.top, 6)
                .padding(.leading, metrics.check + (metrics.touch ? 6 : 4))
                .transition(.opacity.animation(.fadeIn))
                .accessibilityElement(children: .contain)
            }
        }
        .animation(.fadeIn, value: picked?.failed)
    }

    /// The check column: the check on what plays, a spinner on a switch past 600 ms.
    private func check(_ picked: PanelSwitch?) -> some View {
        ZStack {
            if let picked, picked.failed == nil {
                TimelineView(.periodic(from: .now, by: 0.2)) { _ in
                    let shown = ContinuousClock.now - picked.since >= PanelSwitch.silent
                    spinner
                        .opacity(shown ? 1 : 0)
                        .animation(.fadeIn, value: shown)
                }
            } else if on {
                Image(systemName: "checkmark")
                    .font(.system(size: metrics.touch ? 15 : 12, weight: .semibold))
                    .transition(.opacity)
            } else if let loading, loading.asking {
                // From 600 ms; it fades out as the answer fades in.
                TimelineView(.periodic(from: .now, by: 0.2)) { _ in
                    let shown = loading.phase(at: ContinuousClock.now).spins
                    spinner
                        .opacity(shown ? 1 : 0)
                        .animation(.fadeIn, value: shown)
                }
                .transition(.opacity)
            }
        }
        .frame(width: metrics.check)
        .accessibilityHidden(true)
    }

    /// The check column's spinner: 16 pt on touch, in the 24 pt column.
    private var spinner: some View {
        ProgressView()
            .controlSize(metrics.touch ? .regular : .small)
            .scaleEffect(metrics.touch ? 0.8 : 0.9)
            .tint(Palette.silk)
    }

    /// The row's line, crossfading to "Starting · 3 s" once a switch on it takes that long.
    @ViewBuilder
    private func line(_ picked: PanelSwitch?) -> some View {
        if let picked, picked.failed == nil {
            TimelineView(.periodic(from: .now, by: 1)) { _ in
                let waited = ContinuousClock.now - picked.since
                let counting = waited >= PanelSwitch.counting
                ZStack(alignment: .leading) {
                    detailText.opacity(counting ? 0 : 1)
                    if counting {
                        PanelText(
                            text: L.playerPanelStarting(seconds: Int(waited.components.seconds)), size: metrics.detail,
                            line: metrics.detailLine, color: Palette.haze)
                            .monospacedDigit()
                            .transition(.opacity)
                    }
                }
                .animation(.fadeIn, value: counting)
            }
        } else if let loading {
            loadingLine(loading)
        } else {
            detailText
        }
    }

    /// The line while the tab loads, held one line high from the first frame: each change is one
    /// line crossfading in place, old and new together, and the seconds count on in place.
    private func loadingLine(_ loading: RowLoading) -> some View {
        TimelineView(.periodic(from: .now, by: loading.asking ? 0.2 : 3600)) { _ in
            let phase = loading.phase(at: ContinuousClock.now)
            ZStack(alignment: .leading) {
                if let text = phase.text, !text.isEmpty {
                    PanelText(text: text, size: metrics.detail, line: metrics.detailLine, color: Palette.haze)
                        .monospacedDigit()
                        // Read as Asking, so the row is not read out again each second it counts.
                        .accessibilityLabel(loading.asking ? L.playerStartRowAsking : text)
                        .id(phase.key)
                        .transition(.opacity)
                }
            }
            .frame(maxWidth: .infinity, minHeight: metrics.detailLine, alignment: .leading)
            .animation(.fadeIn, value: phase.key)
        }
    }

    @ViewBuilder
    private var detailText: some View {
        if let detail, !detail.isEmpty {
            PanelText(text: detail, size: metrics.detail, line: metrics.detailLine, color: Palette.haze)
                .monospacedDigit()
        }
    }
}

/// The spinner in the segment of the version being found, from 600 ms: over the segmented
/// control, which has no place for one, at the start of that segment's half.
private struct VersionSpinner: View {
    let pending: VersionSwitch?
    let metrics: PanelMetrics

    var body: some View {
        if let pending {
            TimelineView(.periodic(from: .now, by: 0.2)) { _ in
                let shown = pending.spins(at: ContinuousClock.now)
                HStack(spacing: 0) {
                    slot(on: pending.to == .sub, shown: shown)
                    slot(on: pending.to == .dub, shown: shown)
                }
            }
            .allowsHitTesting(false)
            .accessibilityHidden(true)
        }
    }

    private func slot(on: Bool, shown: Bool) -> some View {
        HStack {
            if on {
                ProgressView()
                    .controlSize(metrics.touch ? .small : .mini)
                    .tint(Palette.silk)
                    .opacity(shown ? 1 : 0)
                    .animation(.fadeIn, value: shown)
            }
            Spacer(minLength: 0)
        }
        .padding(.leading, metrics.touch ? 12 : 8)
        .frame(maxWidth: .infinity)
    }
}
