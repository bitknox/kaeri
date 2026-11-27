// The iPhone's and iPad's player, full screen over the app (T2–T5): the PhonePlayer boards, the
// gestures AVKit has taught, and Picture in Picture.

#if os(iOS)
import AVKit
import GameController
import KaeriDesign
import KaeriKit
import KaeriPlayer
import KaeriStore
import KaeriStrings
import SwiftUI
import UIKit

/// The player's content in its full-screen cover. The controls fade in on a tap and out 3 s
/// after (T4); they stay while paused, while scrubbing and while a sheet is up.
public struct PhonePlayerView: View {
    let model: PlayerModel
    @State private var sheet: Sheet?
    /// The Episodes and Sources panel at the trailing edge, and its tab.
    @State private var episodesPanel = false
    @State private var panelTab: PanelTab = .episodes
    /// The phone is held sideways: the room's reactions stand at the trailing edge.
    @State private var sideways = false
    /// The sideways strip's 8 s after any change in the room, as on the Mac's full screen.
    @State private var stripFresh = false
    @State private var stripTask: Task<Void, Never>?
    /// Held upright in a room: the room's own chevron folds it, so the header's is hidden.
    @State private var roomUpright = false
    @GestureState private var drag: CGFloat = 0
    @GestureState private var held = false
    @State private var heldSpeed: Double?
    @State private var flash: SeekFlash?
    @State private var height: CGFloat = 0
    @State private var width: CGFloat = 0
    /// Sideways in a room: the chat field is open.
    @FocusState private var roomFieldFocus: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(AppStore.self) private var store
    private let metrics = PlayerMetrics.phone

    /// The Mac's menus as sheets. The speed is in Picture and sound, since the bar has no room
    /// for a speed button of its own.
    enum Sheet: String, Identifiable {
        case tracks, episodes, picture
        var id: String { rawValue }

        init(_ menu: PlayerMenu) {
            switch menu {
            case .tracks: self = .tracks
            case .episodes: self = .episodes
            case .speed, .picture: self = .picture
            }
        }
    }

    /// Double taps' running count on one side: "+20 s".
    struct SeekFlash: Equatable {
        var forward: Bool
        var seconds: Int
        var at: ContinuousClock.Instant
    }

    public init(model: PlayerModel) {
        self.model = model
    }

    /// The strip over bare video shows while anyone isn't watching, for 8 s after any change,
    /// and with the controls.
    private var stripShown: Bool {
        let room = store.room
        return room.anyoneNotWatching || visible || stripFresh || room.view?.waiting.isEmpty == false
    }

    private func freshenStrip() {
        stripTask?.cancel()
        stripFresh = true
        stripTask = Task {
            try? await Task.sleep(for: .seconds(8))
            if !Task.isCancelled { stripFresh = false }
        }
    }

    private var visible: Bool {
        (model.controlsVisible || sheet != nil) && !model.stillWatching && model.failure == nil && !curtain && !episodesPanel
    }
    /// Up from the playback staged to its first frame, and a roll-on's starting screen its 0.6 s.
    /// A switch to another release holds the old picture instead (`PlayerModel.holding`).
    private var curtain: Bool {
        model.starting != nil || (!model.hasPicture && !model.holding) || model.holdsCurtain || model.startingHeld
    }
    /// How far the swipe down has gone toward closing, 0 to 1.
    private var pull: CGFloat { min(drag / 120, 1) }

    /// Held upright in a room on a phone (or an iPad in a compact width): the video on top and
    /// the room under it. Turned sideways it is full screen with the strip.
    private func roomUpright(_ size: CGSize) -> Bool {
        store.room.active && !(UIDevice.current.userInterfaceIdiom == .pad && size.width >= 600)
            && size.height > size.width
    }

    /// An iPad in a regular width: the Mac's column beside the video.
    private func roomBeside(_ size: CGSize) -> Bool {
        store.room.active && store.room.shown && UIDevice.current.userInterfaceIdiom == .pad && size.width >= 600
    }

    public var body: some View {
        GeometryReader { outer in
            let upright = roomUpright(outer.size)
            let beside = roomBeside(outer.size)
            HStack(spacing: 0) {
                VStack(spacing: 0) {
                    // Upright, the video sits under the status bar: the padding takes the top
                    // inset, so the player's own ignoresSafeArea has none left to grow into.
                    playerBody
                        .overlay { roomLayer(upright: upright, beside: beside) }
                        .frame(height: upright ? (outer.size.width * 220 / 390).rounded() : nil)
                        .clipped()
                        .padding(.top, upright ? outer.safeAreaInsets.top : 0)
                    if upright {
                        PhoneRoomUpright(room: store.room, tracks: { sheet = .tracks }, episodes: { sheet = .episodes })
                            .ignoresSafeArea(.container, edges: .bottom)
                            .transition(.opacity)
                    }
                }
                .ignoresSafeArea(.container, edges: upright ? .top : [])
                if beside {
                    RoomColumn(room: store.room, height: outer.size.height)
                        .ignoresSafeArea()
                        .transition(.opacity)
                }
            }
            .animation(reduceMotion ? .fadeIn : .standard, value: upright)
            .animation(reduceMotion ? .fadeIn : .standard, value: beside)
            .statusBarHidden(!upright)
            .onChange(of: upright, initial: true) { _, on in roomUpright = on }
            // Leave the room?, over the whole player: drawn here, as an alert's presentation
            // takes the player's cover down with it.
            .overlay {
                if let ask = store.room.leaving {
                    RoomLeaveDialog(room: store.room, ask: ask, touch: true)
                }
            }
            .animation(.fadeIn, value: store.room.leaving)
        }
        // Measured on the whole screen: the upright room's video is wider than tall too.
        .onGeometryChange(for: Bool.self, of: Self.isSideways) { sideways = $0 }
        .onChange(of: store.room.active, initial: true) { _, active in PhoneOrientation.room(active) }
        .onAppear {
            store.room.playerShown = true
            store.room.closesPlayer = { [model] in model.viewerClosed() }
        }
        .onChange(of: store.room.changedAt) { freshenStrip() }
        .onDisappear {
            stripTask?.cancel()
            stripTask = nil
            PhoneOrientation.room(false)
            store.room.playerShown = false
            store.room.closesPlayer = {}
        }
        .background(store.room.active ? Palette.ground : .black)
    }

    /// Over the video in a room: the room's panel, and the strip while sideways.
    @ViewBuilder
    private func roomLayer(upright: Bool, beside: Bool) -> some View {
        let room = store.room
        if let panel = room.videoPanel {
            // An iPad with the Mac's column beside the video takes the Mac's panel too.
            if beside {
                RoomMacVideoPanel(room: room, panel: panel)
                    .transition(.opacity)
            } else {
                PhoneRoomVideoPanel(room: room, panel: panel)
                    .transition(.opacity)
            }
        }
        if room.inRoom, !upright, sideways || !room.shown {
            let shown = stripShown
            RoomStrip(room: room, disc: 26, touch: true)
                .padding(.top, visible ? 76 : 14)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
                .opacity(shown ? 1 : 0)
                .animation(shown ? .fadeIn : .fadeOut, value: shown)
                .allowsHitTesting(shown)
                .animation(.standard, value: visible)
                .transition(.opacity)
        }
        if room.inRoom, sideways, room.writing, !room.chatOff {
            roomField
        }
        if room.inRoom, upright {
            // The chevron folds the room into the bar over the tabs.
            Button { close() } label: {
                Image(systemName: "chevron.down")
                    .font(.system(size: 16, weight: .semibold))
                    .frame(width: 36, height: 36)
                    .glassEffect(.regular.interactive(), in: .circle)
                    .frame(width: 44, height: 44)
                    .contentShape(.circle)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(RoomCopy.closeToBar)
            .padding(.leading, 12)
            .padding(.top, 8)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .transition(.opacity)
        }
    }

    /// Sideways in a room: the chat field over the video's foot.
    private var roomField: some View {
        let room = store.room
        return HStack(spacing: 8) {
            TextField("", text: Binding(get: { room.draft }, set: { room.draft = $0 }), prompt: Text(RoomCopy.saySomething).foregroundStyle(.haze))
                .font(.system(size: 16))
                .focused($roomFieldFocus)
                .submitLabel(.send)
                .onSubmit { room.say(); room.writing = false }
                .padding(.horizontal, 16)
                .frame(height: 44)
                .glassEffect(.regular, in: .capsule)
            Button { room.say(); room.writing = false } label: {
                Image(systemName: "arrow.up").font(.system(size: 18, weight: .bold))
                    .frame(width: 44, height: 44)
                    .background(Palette.amber, in: .circle)
                    .foregroundStyle(Palette.ground)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(RoomCopy.send)
        }
        .padding(.horizontal, 56)
        .padding(.bottom, 20)
        .frame(maxHeight: .infinity, alignment: .bottom)
        .onAppear { roomFieldFocus = true }
        .onChange(of: roomFieldFocus) { _, focused in if !focused, room.draft.isEmpty { room.writing = false } }
    }

    private var playerBody: some View {
        GeometryReader { proxy in
            ZStack {
                Color.black
                ZStack {
                    PlayerPicture(model: model)
                    HeldPictureView(model: model)
                    // Out of the tree once it fades, so VoiceOver does not read "Starting episode 7"
                    // over a playing video.
                    if curtain {
                        StartingCurtain(model: model, metrics: metrics)
                            .transition(.opacity)
                    }
                    BufferingLine(model: model, reduceMotion: reduceMotion)
                }
                .animation(curtain ? .easeIn(duration: 0.25) : .fadeIn, value: curtain)
                .scaleEffect(reduceMotion ? 1 : 1 - 0.1 * pull)
                .clipShape(.rect(cornerRadius: reduceMotion ? 0 : 16 * pull))
                .opacity(reduceMotion ? 1 - 0.5 * pull : 1)
                .offset(y: reduceMotion ? 0 : drag)
                .animation(.standard, value: drag == 0)

                // The taps live on their own layer under the chrome, so the chrome's buttons get
                // their first tap instead of a parent's double-tap gesture waiting on it.
                Color.clear
                    .contentShape(.rect)
                    .onTapGesture(count: 2) { location in doubleTapped(at: location, width: proxy.size.width) }
                    .onTapGesture { toggleControls() }
                    .accessibilityHidden(true)

                Group {
                    PlayerScrim().allowsHitTesting(false)
                    chrome
                }
                .opacity(visible && drag == 0 ? 1 : 0)
                .animation(visible ? .fadeIn : .fadeOut, value: visible)
                .allowsHitTesting(visible)

                CornerCard(model: model, metrics: metrics)
                    .padding(metrics.cornerInset)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
                ResumeNoteView(model: model, metrics: metrics)
                    .padding(.top, 72)
                    .padding(.leading, 56)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                    .animation(.standard, value: model.resume)
                PlayerPanelLayer(
                    model: model, metrics: UIDevice.current.userInterfaceIdiom == .pad ? .pad : .phone,
                    open: episodesPanel, tab: $panelTab, close: closeEpisodesPanel)
                // The iPhone gets the side panel; the iPad, wide and tall enough, the Mac's bar.
                let wide = proxy.size.width >= 700 && proxy.size.height >= 600
                SkipMarkLayer(model: model, metrics: metrics, layout: wide ? .bar : .panel)
                    .padding(.horizontal, wide ? 0 : 48)
                stats(in: proxy.size)
                if held {
                    Text("2×")
                        .font(.system(size: 15, weight: .semibold))
                        .padding(.horizontal, 14)
                        .frame(height: 32)
                        .overVideo(radius: 16)
                        .padding(.top, 20)
                        .frame(maxHeight: .infinity, alignment: .top)
                        .transition(.opacity)
                }
                if let flash {
                    Text(flash.forward ? L.playerFlashForward(seconds: flash.seconds) : L.playerFlashBack(seconds: flash.seconds))
                        .font(.system(size: 17, weight: .semibold))
                        .monospacedDigit()
                        .padding(.horizontal, 16)
                        .frame(height: 40)
                        .overVideo(radius: 20)
                        .frame(maxWidth: .infinity, alignment: flash.forward ? .trailing : .leading)
                        .padding(.horizontal, proxy.size.width / 8)
                        .transition(.opacity)
                }
                if model.stillWatching {
                    Palette.controlsDim
                    StillWatchingDialog(model: model, metrics: metrics)
                }
            }
            .foregroundStyle(.silk)
            .contentShape(.rect)
            // The panel's own scrolling and swipe take the touches while it is open.
            .simultaneousGesture(dismissal, including: episodesPanel ? .subviews : .all)
            .simultaneousGesture(speedHold(width: proxy.size.width), including: episodesPanel ? .subviews : .all)
            .simultaneousGesture(
                MagnifyGesture().onEnded { value in model.setFill(value.magnification > 1) },
                including: episodesPanel ? .subviews : .all
            )
        }
        .ignoresSafeArea()
        .persistentSystemOverlays(.hidden)
        .playerAccessibility(model)
        .sensoryFeedback(.impact(weight: .light), trigger: drag > 120) { _, crossed in crossed }
        .sheet(item: $sheet) { sheet in sheetContent(sheet) }
        .onChange(of: sheet) { _, open in model.hold("sheet", open != nil) }
        .onChange(of: model.menuRequests) {
            if model.requestedMenu == .episodes {
                sheet = nil
                openEpisodesPanel()
            } else {
                sheet = Sheet(model.requestedMenu)
            }
        }
        .onChange(of: sheet) { _, open in if open != nil { episodesPanel = false } }
        .onChange(of: store.room.inRoom, initial: true) { _, inRoom in roomChanged(inRoom) }
        .onChange(of: model.chatRequests) { if store.room.inRoom, !store.room.chatOff { store.room.writing = true } }
        .background {
            // A hardware keyboard's C opens the room's field, as on the Mac (iPad).
            if store.room.inRoom, !store.room.writing, !store.room.chatOff {
                Button(L.menuSaySomething) { model.requestChat() }
                    .keyboardShortcut("c", modifiers: [])
                    .frame(width: 0, height: 0)
                    .opacity(0)
                    .accessibilityHidden(true)
            }
        }
        .onChange(of: held) { _, on in speedHeld(on) }
        .onGeometryChange(for: CGFloat.self, of: { $0.size.height }) { height = $0 }
        .onGeometryChange(for: CGFloat.self, of: { $0.size.width }) { width = $0 }
        .onChange(of: SubtitleLift(visible: visible, height: height), initial: true) { _, lift in
            // The subtitles rise above the bar while the controls are up, as PhonePlayer has them.
            model.liftSubtitles(lift.visible ? metrics.subtitleLift / max(lift.height, 1) : 0, animated: !reduceMotion)
        }
        .onAppear { PhoneOrientation.player(true) }
        .onDisappear { PhoneOrientation.player(false) }
        .preferredColorScheme(.dark)
    }

    // MARK: Chrome

    private var chrome: some View {
        ZStack {
            VStack(spacing: 0) {
                header
                Spacer(minLength: 0)
                bottomBar
            }
            reactionColumn
        }
        .overVideoText()
    }

    private var header: some View {
        HStack(spacing: 12) {
            Button { close() } label: {
                Image(systemName: store.room.inRoom ? "chevron.down" : "xmark")
                    .font(.system(size: 17, weight: .semibold))
                    .frame(width: 44, height: 44)
                    .background(.overVideo(0.62), in: .circle)
                    .overlay(Circle().strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
                    .contentShape(.circle)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(store.room.inRoom ? RoomCopy.closeToBar : L.playerClose)
            .opacity(store.room.inRoom && roomUpright ? 0 : 1)
            .allowsHitTesting(!(store.room.inRoom && roomUpright))
            VStack(alignment: .leading, spacing: 1) {
                Text(model.showTitle)
                    .font(.showTitle(size: 15))
                    .fixedLineHeight(20)
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(model.episodeLine)
                        .font(.system(size: 13))
                        .contentTransition(.opacity)
                        .animation(.standard, value: model.episodeLine)
                    if model.isPrivate {
                        PlayerPrivateLabel(size: 13)
                            .transition(.opacity)
                    }
                }
                .animation(.fadeIn, value: model.isPrivate)
            }
            .lineLimit(1)
            .frame(maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 0) {
                if store.room.inRoom {
                    // In a room: Chat opens the field (sideways; upright the room has its own).
                    if !store.room.chatOff {
                        headerButton("bubble.left", L.roomChat) { store.room.writing = true }
                    }
                } else if !store.room.active {
                    // Watch together: a room starts from this episode.
                    headerButton("person.2", RoomCopy.columnTitle) {
                        if let id = model.starting?.id ?? model.staged?.id {
                            store.room.start(id: id, episode: model.episode?.number ?? model.startingEpisode ?? "")
                        }
                    }
                }
                headerButton("captions.bubble", L.playerSubtitlesAndAudio) { sheet = .tracks }
                headerButton("slider.horizontal.3", L.playerPictureAndSound) { sheet = .picture }
                if model.pictureInPictureToggle != nil {
                    // The Mac's button, until the PhonePlayer board draws one.
                    headerButton("pip.enter", L.playerPictureInPicture) { model.togglePictureInPicture() }
                        .disabled(!model.pictureInPictureAvailable)
                        .opacity(model.pictureInPictureAvailable ? 1 : 0.4)
                }
                if model.playOnTVAvailable {
                    headerButton("tv", L.tvPlayOn) { model.playOnTV() }
                }
                AirPlayButton()
                    .frame(width: 44, height: 44)
                    .accessibilityLabel("AirPlay") // i18n-skip
                headerButton("list.bullet", L.playerEpisodes) { openEpisodesPanel() }
            }
            .background(.overVideo(0.62), in: .capsule)
            .overlay(Capsule().strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
        }
        .frame(height: 44)
        .padding(.horizontal, 56)
        .padding(.top, 16)
    }

    /// In a room or out of one: marking is off, and the player's Skip opening skips for the
    /// room, with the chat saying who.
    private func roomChanged(_ inRoom: Bool) {
        model.marksBlocked = inRoom
        model.inRoom = inRoom
        let room = store.room
        if inRoom {
            model.skipsWithRoom = { to in
                room.skipTo = to
                room.skipOpening()
            }
        } else {
            model.skipsWithRoom = nil
        }
    }

    /// Close, or in a room fold it into the bar over the tabs: the player closes and the room
    /// stays, "Player closed" to the others.
    private func close() {
        if store.room.inRoom { store.room.keepRoom() } else { model.viewerClosed() }
    }

    /// The four reactions in a column at the trailing edge, where the thumb rests.
    @ViewBuilder
    private var reactionColumn: some View {
        if store.room.inRoom, sideways, !store.room.chatOff {
            ReactionButtons(metrics: metrics, size: 44, glyph: 20, vertical: true)
                .padding(.vertical, 2)
                .background(.overVideo(0.62), in: .capsule)
                .overlay(Capsule().strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
                .padding(.top, 72)
                .padding(.trailing, 56)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing)
        }
    }




    private var bottomBar: some View {
        HStack(spacing: 4) {
            barButton("gobackward.10", L.playerBack10) { model.seek(by: -10) }
            barButton(model.paused ? "play.fill" : "pause.fill", model.paused ? L.actionPlay : L.actionPause) {
                model.togglePause()
            }
            barButton("goforward.10", L.playerForward10) { model.seek(by: 10) }
            PlayerTime(seconds: model.position, metrics: metrics)
                .frame(width: 40, alignment: .leading)
                .padding(.leading, 8)
            SeekLine(model: model, metrics: metrics)
                .padding(.horizontal, 8)
            PlayerTime(seconds: max(0, (model.duration ?? 0) - model.position), remaining: true, metrics: metrics)
                .frame(width: 52, alignment: .trailing)
        }
        .frame(height: 44)
        .padding(.leading, 56)
        // The iPhone's marking panel takes the trailing edge: the seek row stops short of it, so
        // the marked stretch stays in view.
        .padding(.trailing, model.skipMark != nil && !(width >= 700 && height >= 600) ? 56 + 320 + 16 : 56)
        .animation(.standard, value: model.skipMark != nil)
        .padding(.bottom, 26)
    }

    private func headerButton(_ symbol: String, _ label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 17))
                .frame(width: 44, height: 44)
                .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .voiceControlLabels(label)
    }

    private func barButton(_ symbol: String, _ label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 20, weight: .medium))
                .frame(width: 44, height: 44)
                .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .voiceControlLabels(label)
    }

    private nonisolated static func isSideways(_ proxy: GeometryProxy) -> Bool {
        proxy.size.width > proxy.size.height && proxy.size.height < 500
    }


    @ViewBuilder
    private func sheetContent(_ sheet: Sheet) -> some View {
        menuSheet(sheet)
    }

    @ViewBuilder
    private func menuSheet(_ sheet: Sheet) -> some View {
        if sheet == .episodes {
            // The episode under the pointer or with the keyboard's focus, else the one playing,
            // in a footer under the list, kept only if the table had synopses when it opened.
            PhoneEpisodesSheet(model: model, metrics: metrics) { menuList(sheet, pointed: $0, focused: $1) }
        } else {
            menuList(sheet, pointed: nil, focused: nil)
        }
    }

    @ViewBuilder
    private func menuList(_ sheet: Sheet, pointed: Binding<String?>?, focused: Binding<String?>?) -> some View {
        ScrollViewReader { list in
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    switch sheet {
                    case .tracks:
                        TrackLists(model: model, metrics: metrics)
                    case .picture:
                        SpeedList(model: model, metrics: metrics)
                        if model.menus != nil {
                            MenuDivider()
                            PictureLists(model: model, metrics: metrics)
                        }
                        MenuDivider()
                        // The Mac's More menu is this sheet here: Stats for nerds goes up over
                        // the picture, so the sheet goes down to show it.
                        MenuRow(title: L.playerStats, checked: model.statsShown, metrics: metrics) {
                            model.toggleStats()
                            if model.statsShown { self.sheet = nil }
                        }
                        MenuDivider()
                        MenuRow(
                            title: L.playerMarkSkipTimes, detail: model.marksBlocked ? L.playerMarkUnavailable : nil,
                            checked: false, metrics: metrics
                        ) {
                            guard model.canMarkSkips else { return }
                            self.sheet = nil
                            model.beginSkipMark()
                        }
                        MenuDivider()
                        MenuRow(title: L.playerSkipTimes, checked: false, metrics: metrics) {
                            self.sheet = nil
                            model.toggleSkipTimes()
                        }
                        // From 85% of the episode, where it counts as watched.
                        if let offer = model.discussionOffer {
                            MenuDivider()
                            MenuRow(
                                title: L.discussionOpen,
                                detail: offer.count.map { L.discussionApplePlayerNote(count: Int($0)) } ?? L.discussionApplePauses,
                                checked: false, metrics: metrics
                            ) {
                                self.sheet = nil
                                model.openDiscussion()
                            }
                        }
                    case .episodes:
                        EpisodeList(model: model, metrics: metrics, picked: { self.sheet = nil }, pointed: pointed, focused: focused)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
                .environment(\.pointedMenuRow, model.pointedRow)
            }
            .onAppear {
                // The row in use, in view however long the list.
                let row: String? =
                    switch sheet {
                    case .tracks: TrackLists.selectedRow(model)
                    case .episodes: EpisodeList.selectedRow(model)
                    case .picture: nil
                    }
                if let row { list.scrollTo(row, anchor: .center) }
            }
        }
        .presentationDetents([.medium, .large])
        .presentationBackground(Palette.ground)
        .presentationDragIndicator(.visible)
        .preferredColorScheme(.dark)
    }

    // MARK: Stats for nerds

    /// Stats for nerds, which stays up while the controls hide. Over the picture at the top
    /// trailing corner, under the top bar, as on the Mac. An iPhone held sideways is too short
    /// for that: the panel is compact there and ends above the subtitles, which sit higher
    /// while the controls are up; its rows scroll when that leaves them too little room. On an
    /// upright window too narrow for it, under the picture at the full width less 16 pt.
    private func stats(in size: CGSize) -> some View {
        let upright = size.height > size.width && size.width < 600
        let short = size.height < 500
        let picture = BufferingLine.picture(in: size, aspect: model.videoAspect ?? 16 / 9)
        let insets =
            if upright {
                EdgeInsets(top: picture.maxY + 16, leading: 16, bottom: 32, trailing: 16)
            } else if short {
                // The subtitles' two lines from 18 pt above the bottom, lifted with the controls.
                EdgeInsets(top: visible ? 72 : 16, leading: 0, bottom: 90 + (visible ? metrics.subtitleLift : 0), trailing: 56)
            } else {
                EdgeInsets(top: 72, leading: 0, bottom: 84, trailing: 56)
            }
        return StatsPanel(
            model: model, metrics: metrics, width: upright ? size.width - 32 : StatsPanel.width, compact: short && !upright
        )
        .padding(insets)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: upright ? .top : .topTrailing)
        .animation(.standard, value: insets)
        .animation(model.statsPanel != nil ? .fadeIn : .fadeOut, value: model.statsPanel != nil)
    }

    // MARK: Gestures

    /// A tap shows or hides the controls and never pauses; a double tap on the left or right
    /// third seeks 10 s, and each further one within 0.6 s adds 10 more.
    private func doubleTapped(at location: CGPoint, width: CGFloat) {
        let third = location.x / max(width, 1)
        guard third < 1 / 3 || third > 2 / 3 else {
            toggleControls()
            return
        }
        let forward = third > 2 / 3
        let now = ContinuousClock.now
        var next = SeekFlash(forward: forward, seconds: 10, at: now)
        if let flash, flash.forward == forward, now - flash.at < .milliseconds(600) {
            next.seconds = flash.seconds + 10
        }
        withAnimation(.quick) { flash = next }
        model.seek(by: forward ? 10 : -10)
        Task {
            try? await Task.sleep(for: .milliseconds(900))
            if flash?.at == now { withAnimation(.fadeOut) { flash = nil } }
        }
    }

    /// The Episodes and Sources panel opens on Episodes (player panel board).
    private func openEpisodesPanel() {
        sheet = nil
        panelTab = .episodes
        episodesPanel = true
    }

    private func closeEpisodesPanel() {
        episodesPanel = false
        model.poke()
    }

    private func toggleControls() {
        if model.controlsVisible && !model.paused { model.hideControls() } else { model.poke() }
    }

    /// The video follows the finger down and closes past 120 pt or on a fast flick, otherwise
    /// springs back (T3).
    private var dismissal: some Gesture {
        DragGesture(minimumDistance: 12)
            .updating($drag) { value, state, _ in
                guard model.scrubbing == nil, abs(value.translation.height) > abs(value.translation.width) else { return }
                state = max(0, value.translation.height)
            }
            .onEnded { value in
                guard model.scrubbing == nil else { return }
                if value.translation.height > 120 || value.velocity.height > 800 { close() }
            }
    }

    /// A long press on the right half plays at 2× while held.
    private func speedHold(width: CGFloat) -> some Gesture {
        LongPressGesture(minimumDuration: 0.5)
            .sequenced(before: DragGesture(minimumDistance: 0))
            .updating($held) { value, state, _ in
                if case .second(true, let drag?) = value, drag.startLocation.x > width / 2 { state = true }
            }
    }

    private func speedHeld(_ on: Bool) {
        if on {
            heldSpeed = model.speed
            model.setSpeed(2)
        } else if let heldSpeed {
            model.setSpeed(heldSpeed)
            self.heldSpeed = nil
        }
    }
}

/// AirPlay for audio: AirPlay video needs AVPlayer and a stream the receiver fetches itself.
struct AirPlayButton: UIViewRepresentable {
    func makeUIView(context: Context) -> AVRoutePickerView {
        let view = AVRoutePickerView()
        view.tintColor = UIColor(Palette.silk)
        view.activeTintColor = UIColor(Palette.amber)
        view.prioritizesVideoDevices = false
        return view
    }

    func updateUIView(_ view: AVRoutePickerView, context: Context) {}
}

/// The Episodes sheet on iPhone and iPad: the list, and under it the footer with the title and
/// synopsis of the row under the pointer or with the keyboard's focus, else the playing one, as
/// the Mac's panel has it.
private struct PhoneEpisodesSheet<List: View>: View {
    @Environment(AppStore.self) private var store
    let model: PlayerModel
    let metrics: PlayerMetrics
    @ViewBuilder let list: (Binding<String?>, Binding<String?>) -> List
    @State private var pointed: String?
    @State private var focused: String?

    var body: some View {
        list($pointed, $focused)
            .safeAreaInset(edge: .bottom, spacing: 0) {
                EpisodeFooter(
                    model: model, metrics: metrics, table: model.staged.flatMap { store.titles.episodes[$0.id] } ?? [],
                    pointed: pointed, focused: focused
                )
                .background(Palette.ground)
            }
    }
}
#endif
