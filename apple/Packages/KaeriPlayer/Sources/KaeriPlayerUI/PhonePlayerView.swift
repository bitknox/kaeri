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
    /// The party sheet opens at the conversation: it came from the column's "n more in chat".
    @State private var sheetAtChat = false
    /// The phone is held sideways: the party opens as a trailing panel over the video, not a sheet.
    @State private var sideways = false
    @GestureState private var drag: CGFloat = 0
    @GestureState private var held = false
    @State private var heldSpeed: Double?
    @State private var flash: SeekFlash?
    @State private var height: CGFloat = 0
    /// The chat field's bar, opened by the chat button (or C on a hardware keyboard).
    @State private var fielding = false
    @FocusState private var fieldFocus: Bool
    @State private var keyboard: CGFloat = 0
    /// Typing brings up the on-screen keyboard: no hardware keyboard is attached, or the last
    /// keyboard frame was a full one rather than a hardware keyboard's shortcuts bar.
    @State private var softKeyboard = GCKeyboard.coalesced == nil
    @State private var width: CGFloat = 0
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(AppStore.self) private var store
    private let metrics = PlayerMetrics.phone

    /// The Mac's menus as sheets. The speed is in Picture and sound, since the bar has no room
    /// for a speed button of its own.
    enum Sheet: String, Identifiable {
        case tracks, episodes, picture
        /// The watch party, from the people button while in one.
        case party
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

    private var visible: Bool { (model.controlsVisible || sheet != nil) && !model.stillWatching && model.failure == nil && !curtain }
    private var curtain: Bool { model.starting != nil || !model.hasPicture || model.holdsCurtain }
    /// How far the swipe down has gone toward closing, 0 to 1.
    private var pull: CGFloat { min(drag / 120, 1) }

    public var body: some View {
        GeometryReader { proxy in
            ZStack {
                Color.black
                ZStack {
                    PlayerPicture(model: model)
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
                // While typing on a phone, one row fits between the header and the field's bar.
                PartyColumn(
                    model: model, metrics: metrics, covered: sheet == .party,
                    rows: fielding && UIScreen.main.bounds.height < 500 ? 1 : PartyChat.columnRows
                ) {
                    sheetAtChat = true
                    sheet = .party
                }
                    .padding(.top, 72)
                    .padding(.leading, 56)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                if chatShown {
                    ReactionMomentView(metrics: metrics)
                        .padding(.trailing, 56)
                        .padding(.bottom, model.corner == nil ? 84 : metrics.cornerInset.bottom + 88)
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
                        .animation(.standard, value: model.corner == nil)
                        .allowsHitTesting(false)
                }
                chatBar
                sidePanel
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
            .simultaneousGesture(dismissal)
            .simultaneousGesture(speedHold(width: proxy.size.width))
            .simultaneousGesture(
                MagnifyGesture().onEnded { value in model.setFill(value.magnification > 1) }
            )
        }
        .ignoresSafeArea()
        .statusBarHidden()
        .persistentSystemOverlays(.hidden)
        .playerAccessibility(model)
        .sensoryFeedback(.impact(weight: .light), trigger: drag > 120) { _, crossed in crossed }
        .sheet(item: sheetShown) { sheet in sheetContent(sheet) }
        .onGeometryChange(for: Bool.self, of: Self.isSideways) { sideways = $0 }
        .animation(reduceMotion ? .fadeIn : .standard, value: partyBeside)
        .onChange(of: sheet) { _, open in
            model.hold("sheet", open != nil)
            // A sheet closes the field's bar; the draft stays, and the party sheet's field shows it.
            if open != nil, fielding { closeField() }
        }
        .onChange(of: model.menuRequests) { sheet = Sheet(model.requestedMenu) }
        .onChange(of: store.party.panelRequests) {
            #if FIXTURES
            // The conversation's shot opens the sheet as the column's fold does, at the chat.
            if PartyScene.named() == .chatpanel { sheetAtChat = true }
            #endif
            sheet = store.party.inParty ? .party : nil
        }
        // The party is over: its sheet goes with it.
        .onChange(of: store.party.inParty) { _, inParty in
            if !inParty {
                if sheet == .party { sheet = nil }
                if fielding { closeField() }
            }
        }
        .animation(.fadeIn, value: fielding)
        .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillChangeFrameNotification)) { note in
            guard let end = (note.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect) else { return }
            let screen = UIScreen.main.bounds.height
            keyboard = max(0, screen - end.minY)
            if fielding { softKeyboard = keyboard >= 100 }
        }
        .onChange(of: fieldFocus) { _, focused in
            if !focused, fielding, store.party.chat.draft.isEmpty { closeField() }
        }
        .onChange(of: store.party.inParty, initial: true) { _, inParty in
            model.marksBlocked = inParty
            model.inParty = inParty
        }
        .onChange(of: model.chatRequests) { if chatShown, store.party.chat.allowed { startTyping() } }
        .background {
            // A hardware keyboard's C opens the field over the control row, as on the Mac (iPad).
            if !fielding, chatShown {
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
        .onAppear { PlayerOrientation.landscape(true) }
        .onDisappear { PlayerOrientation.landscape(false) }
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
            Button { model.viewerClosed() } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 17, weight: .semibold))
                    .frame(width: 44, height: 44)
                    .background(.overVideo(0.62), in: .circle)
                    .overlay(Circle().strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
                    .contentShape(.circle)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(L.playerClose)
            VStack(alignment: .leading, spacing: 1) {
                Text(model.showTitle)
                    .font(.showTitle(size: 15))
                    .fixedLineHeight(20)
                Text(model.episodeLine)
                    .font(.system(size: 13))
                    .contentTransition(.opacity)
                    .animation(.standard, value: model.episodeLine)
            }
            .lineLimit(1)
            .frame(maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 0) {
                headerButton(
                    PartyButton.symbol(inParty: store.party.inParty), PartyButton.label(inParty: store.party.inParty)
                ) {
                    // The party while in one, the form that starts one otherwise.
                    if store.party.inParty { sheet = .party } else { store.party.open(.playing) }
                }
                if chatShown, store.party.chat.allowed {
                    headerButton("bubble.left", L.partyChatTitle) { startTyping() }
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
                headerButton("list.bullet", L.playerEpisodes) { sheet = .episodes }
            }
            .background(.overVideo(0.62), in: .capsule)
            .overlay(Capsule().strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
            // The side panel covers this edge: the buttons fade out while it is open.
            .opacity(partyBeside ? 0 : 1)
            .allowsHitTesting(!partyBeside)
            .animation(partyBeside ? .fadeOut : .fadeIn, value: partyBeside)
        }
        .frame(height: 44)
        .padding(.horizontal, 56)
        .padding(.top, 16)
    }

    private var chatShown: Bool { store.party.inParty && store.party.chat.shown }

    /// The four reactions in a column at the trailing edge, where the thumb rests.
    @ViewBuilder
    private var reactionColumn: some View {
        if chatShown {
            ReactionButtons(metrics: metrics, size: 44, glyph: 20, vertical: true)
                .padding(.vertical, 2)
                .background(.overVideo(0.62), in: .capsule)
                .overlay(Capsule().strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
                .opacity(partyBeside ? 0 : 1)
                .allowsHitTesting(!partyBeside)
                .animation(partyBeside ? .fadeOut : .fadeIn, value: partyBeside)
                .padding(.top, 72)
                .padding(.trailing, 56)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing)
        }
    }

    private func startTyping() {
        fielding = true
        fieldFocus = true
        model.hold("chat", true)
    }

    private func closeField() {
        fielding = false
        fieldFocus = false
        model.hold("chat", false)
    }

    /// The field above the keyboard: 36 pt and Send; the video keeps playing above it. On an iPad
    /// with a hardware keyboard, where no keyboard comes up, it stands over the control row as on
    /// the Mac: on glass at the leading edge, 520 pt at most, with no band behind it.
    @ViewBuilder
    private var chatBar: some View {
        let chat = store.party.chat
        let hardware = UIDevice.current.userInterfaceIdiom == .pad && !softKeyboard
        if fielding, chatShown {
            HStack(spacing: 8) {
                ChatTextField(
                    text: Binding(get: { chat.draft }, set: { chat.draft = $0 }),
                    placeholder: L.partyChatField, style: hardware ? .overVideo : .roomy, limit: chat.maxLength,
                    remaining: chat.remaining,
                    waitLabel: chat.wait > 0 ? L.partyChatWait(seconds: chat.wait) : nil,
                    waitSpoken: chat.wait > 0 ? L.partyChatWaitSpoken(seconds: chat.wait) : nil,
                    focus: $fieldFocus
                ) {
                    chat.submit()
                }
                // With a hardware keyboard Return sends, as on the Mac: no button.
                if !hardware { ChatSendButton(enabled: chat.canSend) { chat.submit() } }
            }
            .frame(maxWidth: hardware ? 520 : .infinity, alignment: .leading)
            .padding(.horizontal, 56)
            .padding(.vertical, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(hardware ? Color.clear : Palette.ground.opacity(0.88))
            .padding(.bottom, max(keyboard, 76))
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
            .transition(.opacity)
            .onKeyPress(.escape) { closeField(); return .handled }
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

    private var partyBeside: Bool { sheet == .party && sideways }

    /// The sheet on screen: not the party's while it is the side panel.
    private var sheetShown: Binding<Sheet?> {
        Binding(get: { partyBeside ? nil : sheet }, set: { sheet = $0 })
    }

    /// The party's members and conversation, in the sheet and in the sideways panel.
    private var partyScroll: some View {
        ScrollViewReader { reader in
            ScrollView { PartyPanel(metrics: metrics) }
                .task {
                    // Opened from "n more in chat": at the conversation, not the members.
                    guard sheetAtChat else { return }
                    sheetAtChat = false
                    await Task.yield()
                    reader.scrollTo(PartyPanel.chatPlace, anchor: .top)
                }
        }
    }

    /// The party on a phone held sideways (watch together chat board): 360 pt at the trailing
    /// edge over the video, which stays whole. A tap on the picture or a swipe toward the edge
    /// closes it; under Reduce Motion it crossfades in place.
    @ViewBuilder
    private var sidePanel: some View {
        if partyBeside {
            partyScroll
                .frame(width: 360)
                .frame(maxHeight: .infinity)
                .background(Palette.raised, in: UnevenRoundedRectangle(topLeadingRadius: 28, bottomLeadingRadius: 28))
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .trailing)
                .transition(reduceMotion ? .opacity : .move(edge: .trailing))
                .gesture(DragGesture(minimumDistance: 24).onEnded { value in
                    if value.translation.width > 60 { sheet = nil }
                })
                .accessibilityAddTraits(.isModal)
                .accessibilityAction(.escape) { sheet = nil }
                .preferredColorScheme(.dark)
        }
    }

    @ViewBuilder
    private func sheetContent(_ sheet: Sheet) -> some View {
        if sheet == .party {
            partyScroll
            .presentationDetents([.medium, .large])
            .presentationBackground(Palette.ground)
            .presentationDragIndicator(.visible)
            .preferredColorScheme(.dark)
        } else {
            menuSheet(sheet)
        }
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
                    case .party:
                        EmptyView()
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
                    case .picture, .party: nil
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

    private func toggleControls() {
        if sheet == .party, sideways {
            sheet = nil
            return
        }
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
                if value.translation.height > 120 || value.velocity.height > 800 { model.viewerClosed() }
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

/// The iPhone turns to landscape while the player is up and back to portrait after; the iPad
/// stays as it is held. The app delegate answers `supportedInterfaceOrientationsFor` with `mask`.
@MainActor
public enum PlayerOrientation {
    public private(set) static var mask: UIInterfaceOrientationMask = .allButUpsideDown

    static func landscape(_ on: Bool) {
        guard UIDevice.current.userInterfaceIdiom == .phone,
            let scene = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first
        else { return }
        mask = on ? .landscape : .allButUpsideDown
        for window in scene.windows {
            window.rootViewController?.setNeedsUpdateOfSupportedInterfaceOrientations()
        }
        scene.requestGeometryUpdate(.iOS(interfaceOrientations: on ? .landscape : .portrait)) { _ in }
    }
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
