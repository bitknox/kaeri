// Opening your own link: a magnet link, an info hash or a torrent file, then "Is this ...?" with
// Play, Change and Play without tracking. One sheet with two steps, as docs/native-ux.md asks.

import KaeriKit
import KaeriStore
import KaeriDesign
import KaeriStrings
import SwiftUI
import UniformTypeIdentifiers

extension View {
    /// The Open link or file sheet: shown when the navigator asks for it, and when the engine
    /// has read a link the system handed over.
    func openLinkSheet(_ navigator: Navigator) -> some View {
        modifier(OpenLinkPresenter(navigator: navigator))
    }
}

private struct OpenLinkPresenter: ViewModifier {
    @Environment(AppStore.self) private var store
    let navigator: Navigator

    func body(content: Content) -> some View {
        content.sheet(isPresented: Binding(
            get: { navigator.openLinkShown || store.opened.link != nil },
            set: { shown in
                if !shown {
                    navigator.openLinkShown = false
                    store.opened.close()
                }
            }
        )) {
            OpenLinkSheet()
        }
    }
}

/// What the confirm step says of the link: the question, from the guess.
enum OpenedWords {
    static func question(_ link: OpenedLink, picked: Entry?, episode: String) -> String {
        if let picked {
            return episode.isEmpty
                ? L.openedConfirmTitle(title: picked.showTitle)
                : L.openedConfirm(title: picked.showTitle, episode: episode)
        }
        return L.openedNoGuess
    }
}

struct OpenLinkSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        Group {
            if let link = store.opened.link {
                OpenedConfirm(link: link)
            } else {
                OpenLinkPaste()
            }
        }
        .padding(30)
        .sheetWidth(520)
        .background(.raised)
    }
}

/// The first step: a magnet link or info hash to paste, or a torrent file to choose.
private struct OpenLinkPaste: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var choosing = false
    @FocusState private var focused: Bool

    var body: some View {
        @Bindable var opened = store.opened
        VStack(alignment: .leading, spacing: 0) {
            Text(L.openedPasteTitle)
                .font(.mac(15, weight: .semibold))
                .macLineHeight(20)
                .foregroundStyle(.silk)
            TextField(L.openedPastePlaceholder, text: $opened.input)
                .textFieldStyle(.plain)
                .font(.mac(13, design: .monospaced))
                .focused($focused)
                .onSubmit(read)
                #if os(iOS)
                .keyboardType(.URL)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                #endif
                .padding(.horizontal, 10)
                .frame(height: 36)
                .background(.ground, in: .rect(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
                .padding(.top, 14)
            HStack(spacing: 10) {
                Button(L.openedChooseFile) { choosing = true }
                    .buttonStyle(SecondaryCapsuleStyle())
                Spacer(minLength: 0)
                Button(L.actionCancel) { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle())
                Button(L.openedOpen, action: read)
                    .buttonStyle(PrimaryCapsuleStyle())
                    .disabled(store.opened.input.trimmingCharacters(in: .whitespaces).isEmpty || store.opened.reading)
            }
            .padding(.top, 20)
        }
        .task { focused = true }
        .fileImporter(isPresented: $choosing, allowedContentTypes: [UTType(filenameExtension: "torrent") ?? .data]) { result in
            if case .success(let url) = result { store.openFromSystem(url) }
        }
    }

    private func read() { store.openLink(store.opened.input) }
}

/// The second step: the guess, and what to do with it.
private struct OpenedConfirm: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @Environment(Navigator.self) private var navigator
    let link: OpenedLink
    @State private var picked: Entry?
    @State private var episode = ""
    @State private var seeded = false
    @State private var query = ""
    @State private var turningOn = false

    var body: some View {
        let opened = store.opened
        VStack(alignment: .leading, spacing: 0) {
            if opened.changing || (picked == nil) {
                change(opened)
            } else if let picked {
                confirm(picked)
            }
            actions(opened)
        }
        .onAppear(perform: seed)
        .onChange(of: link.link) { seeded = false; seed() }
        .sheet(isPresented: $turningOn) { turnOnSheet }
    }

    @ViewBuilder private var turnOnSheet: some View {
        #if os(iOS)
        if UIDevice.current.userInterfaceIdiom == .phone { PhoneTorrentSheet() } else { TorrentSheet() }
        #else
        TorrentSheet()
        #endif
    }

    private func seed() {
        guard !seeded else { return }
        seeded = true
        picked = link.guess?.title
        episode = link.guess?.episode ?? ""
        store.opened.changing = link.guess == nil || navigator.launchSheet == "opened-change"
    }

    private func confirm(_ entry: Entry) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .top, spacing: 14) {
                CoverArt(entry: entry, width: 64)
                VStack(alignment: .leading, spacing: 4) {
                    Text(OpenedWords.question(link, picked: entry, episode: episode))
                        .font(.mac(15, weight: .semibold))
                        .macLineHeight(20)
                        .foregroundStyle(.silk)
                        .fixedSize(horizontal: false, vertical: true)
                    if let name = link.name {
                        Text(name)
                            .font(.mac(12, design: .monospaced))
                            .macLineHeight(16)
                            .foregroundStyle(.haze)
                            .lineLimit(2)
                    }
                }
            }
            if episode.isEmpty || link.guess?.episode == nil {
                HStack(spacing: 10) {
                    Text(L.openedEpisode).font(.mac(13)).foregroundStyle(.haze)
                    TextField(L.openedEpisode, text: $episode)
                        .textFieldStyle(.plain)
                        .font(.mac(13, design: .monospaced))
                        #if os(iOS)
                        .keyboardType(.numbersAndPunctuation)
                        #endif
                        .padding(.horizontal, 10)
                        .frame(width: 90, height: 30)
                        .background(.ground, in: .rect(cornerRadius: 8))
                        .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
                }
            }
        }
    }

    /// Change: the engine's candidates and a search for other words.
    private func change(_ opened: OpenedLinkModel) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(link.guess == nil ? L.openedNoGuess : L.openedChangeTitle)
                .font(.mac(15, weight: .semibold))
                .macLineHeight(20)
                .foregroundStyle(.silk)
                .fixedSize(horizontal: false, vertical: true)
            TextField(L.openedChangeSearch, text: $query)
                .textFieldStyle(.plain)
                .font(.mac(13))
                .onSubmit { store.findOpened(query) }
                .padding(.horizontal, 10)
                .frame(height: 32)
                .background(.ground, in: .rect(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
            if let failed = opened.searchFailed {
                Text(failed).font(.mac(13)).foregroundStyle(.vermilion)
            }
            ScrollView {
                VStack(spacing: 0) {
                    ForEach(opened.choices, id: \.id) { entry in
                        Button {
                            picked = entry
                            opened.changing = false
                        } label: {
                            HStack(spacing: 12) {
                                CoverArt(entry: entry, width: 32)
                                VStack(alignment: .leading, spacing: 1) {
                                    Text(entry.showTitle).font(.mac(13, weight: .medium)).foregroundStyle(.silk)
                                    Text(entry.metadataLine).font(.mac(12)).foregroundStyle(.haze)
                                }
                                Spacer(minLength: 0)
                            }
                            .padding(.vertical, 6)
                            .contentShape(.rect)
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .frame(maxHeight: 280)
        }
    }

    private func actions(_ opened: OpenedLinkModel) -> some View {
        let ready = picked != nil && !opened.changing && !episode.trimmingCharacters(in: .whitespaces).isEmpty
        return VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 10) {
                if !link.playbackOn {
                    Button(L.openedTurnOn) { turningOn = true }
                        .buttonStyle(PrimaryCapsuleStyle())
                } else if let picked, ready {
                    Button(L.openedPlay) { store.playOpened(picked, episode: episode, title: picked.showTitle); dismiss() }
                        .buttonStyle(PrimaryCapsuleStyle())
                    Button(L.openedDownload) { store.downloadOpened(picked, episode: episode); dismiss() }
                        .buttonStyle(SecondaryCapsuleStyle())
                }
                Spacer(minLength: 0)
                Button(L.actionCancel) { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle())
            }
            HStack(spacing: 10) {
                if picked != nil, !opened.changing {
                    Button(L.openedChange) { opened.changing = true }
                        .buttonStyle(SecondaryCapsuleStyle())
                }
                Button(L.openedPlayUntracked) { store.playOpenedUntracked(); dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle())
                    .disabled(!link.playbackOn)
                Spacer(minLength: 0)
            }
            Text(L.openedUntrackedDetail)
                .font(.mac(12))
                .macLineHeight(16)
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.top, 20)
    }
}
