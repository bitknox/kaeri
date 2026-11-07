// First launch (native-ux, First launch): short pages, then the app. Welcome says what Kaeri
// is; where reading is on, what Kaeri is for comes next, the one page without Skip; AniList
// offers the sign-in; Sources takes an invite link and says whose code a plugin is, and after
// Read it offers a folder and a server for books. Each step is in Settings afterwards. A
// full-screen cover on iPhone, a sheet on Mac (560 × 480 pt) and iPad.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// Whether first launch has been seen, kept in the app's defaults.
enum FirstLaunchFlag {
    static let key = "firstLaunchSeen"

    /// Whether to show it on this launch: once, in the app itself, and not to an install already
    /// in use (a source, a signed-in tracker or something watched), which is marked seen instead.
    /// The Fixtures app and the dev harness show it only when a shot asks with `sheet=welcome`.
    @MainActor
    static func shows(_ navigator: Navigator, store: AppStore) async -> Bool {
        if let sheet = navigator.launchSheet, FirstLaunch.Page.shot(sheet) != nil { return true }
        guard Bundle.main.bundleIdentifier == "app.kaeri.app" else { return false }
        let seen = UserDefaults.standard.bool(forKey: key)
        let due = await store.firstLaunchDue(seen: seen)
        if !seen, !due { UserDefaults.standard.set(true, forKey: key) }
        return due
    }
}

/// Puts first launch over the app while it is due.
struct FirstLaunchPresenter: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var showing = false
    /// The invite link Join took, opened in the join sheet once first launch has gone.
    @State private var joining: String?
    /// Restore from a cloud folder, chosen on the code page: the picker opens once this ends.
    @State private var restoring = false
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    #endif

    func body(content: Content) -> some View {
        content
            .task {
                if await FirstLaunchFlag.shows(navigator, store: store) {
                    let page = navigator.launchSheet.flatMap(FirstLaunch.Page.shot) ?? .welcome
                    // The code page takes welcome-setup itself, once it is up.
                    if page != .setup {
                        // Taken by the pages as they appear: a sheet's content can be built
                        // before this state would reach it.
                        navigator.firstLaunchStart = (page, navigator.launchSheet.flatMap(FirstLaunch.Page.shotAnswer))
                        navigator.launchSheet = nil
                    }
                    showing = true
                } else {
                    navigator.firstLaunchSettled = true
                }
            }
            #if os(iOS)
            .fullScreenCover(isPresented: compact ? $showing : .constant(false), onDismiss: done) { flow }
            .sheet(isPresented: compact ? .constant(false) : $showing, onDismiss: done) { flow }
            #else
            .sheet(isPresented: $showing, onDismiss: done) {
                flow.frame(width: 560, height: 480)
            }
            #endif
    }

    private var flow: some View {
        FirstLaunch(finish: { link in
            joining = link
            showing = false
        }, restore: {
            restoring = true
            showing = false
        })
    }

    private func done() {
        UserDefaults.standard.set(true, forKey: FirstLaunchFlag.key)
        navigator.firstLaunchSettled = true
        if restoring {
            restoring = false
            store.sync.addCloudFolder()
        }
        if let joining {
            navigator.invite = joining
            self.joining = nil
        }
    }

    #if os(iOS)
    private var compact: Bool { sizeClass == .compact }
    #endif
}

/// The pages.
struct FirstLaunch: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    /// Ends first launch, with the invite link Join took, if it took one.
    let finish: (String?) -> Void
    /// Ends first launch and opens the cloud folder picker.
    var restore: () -> Void = {}

    enum Page: Int {
        case welcome, medium, anilist, sources, setup

        /// The page a shot's sheet name opens on.
        static func shot(_ name: String) -> Page? {
            switch name {
            case "welcome": .welcome // i18n-skip
            case "welcome-medium": .medium // i18n-skip
            case "welcome-lists", "welcome-lists-read": .anilist // i18n-skip
            case "welcome-sources", "welcome-books", "welcome-both": .sources // i18n-skip
            case "welcome-setup": .setup // i18n-skip
            default: nil
            }
        }

        /// What a shot of a later page took as the answer on the medium page.
        static func shotAnswer(_ name: String) -> KaeriIsFor? {
            switch name {
            case "welcome-lists", "welcome-both": .both // i18n-skip
            case "welcome-lists-read", "welcome-books": .read // i18n-skip
            default: nil
            }
        }
    }
    @State private var page: Page
    /// The answer the medium page took, which the pages after it follow.
    @State private var chosen: KaeriIsFor?
    init(start: Page = .welcome, answered: KaeriIsFor? = nil, finish: @escaping (String?) -> Void,
         restore: @escaping () -> Void = {}) {
        self.finish = finish
        self.restore = restore
        _page = State(initialValue: start)
        _chosen = State(initialValue: answered)
    }
    @State private var link = ""
    /// The link came from a kaeri:// link, so Join is the way on.
    @State private var openedFromLink = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            topBar
            Group {
                switch page {
                case .welcome: asks ? AnyView(welcomeFour) : AnyView(welcome)
                case .medium: medium
                case .anilist: asks ? AnyView(lists) : AnyView(anilist)
                case .sources: asks && reads ? AnyView(books) : AnyView(sources)
                case .setup: setup
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .transition(.opacity)
            .id(page)
        }
        .padding(padding)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(.ground)
        .animation(reduceMotion ? nil : .standard, value: page)
        // An invite link in the field holds the iPad's sheet open; a swipe down is Skip until then.
        .holdsInput(when: !link.isEmpty)
        .onAppear {
            // A shot's later page, and the answer the medium page took before it.
            guard let (start, answered) = navigator.firstLaunchStart else { return }
            navigator.firstLaunchStart = nil
            var instant = Transaction()
            instant.disablesAnimations = true
            withTransaction(instant) {
                page = start
                chosen = answered
            }
        }
        .task {
            // i18n-skip: a shot's sheet name
            if navigator.launchSheet == "welcome-setup" { navigator.launchSheet = nil; page = .setup }
        }
        .sheet(item: $bookSheet) { step in AddBookSourceSheet(start: step) }
        .sheet(isPresented: Bindable(navigator).addServerShown) { AddServerSheet() }
        .onChange(of: store.servers.added) { _, added in
            // The combined page stays for the books part.
            if added != nil, page == .sources, !(asks && reads) { finish(nil) }
        }
        // An invite link opened from Messages or Mail jumps here, filled in.
        .onChange(of: navigator.invite) { _, invite in
            if let invite {
                link = invite
                openedFromLink = true
                navigator.invite = nil
                page = .sources
            }
        }
        // Signed in: on to Sources.
        .onChange(of: store.accounts.tracker("anilist")?.connected ?? false) { _, connected in
            if connected, page == .anilist { page = .sources }
        }
    }

    private var padding: EdgeInsets {
        #if os(macOS)
        EdgeInsets(top: 16, leading: 40, bottom: 32, trailing: 24)
        #else
        EdgeInsets(top: 8, leading: 24, bottom: 24, trailing: 20)
        #endif
    }

    // MARK: Pages

    private var welcome: some View {
        VStack(alignment: .leading, spacing: 0) {
            Nagare(cell: 5)
                .accessibilityHidden(true)
            Text("Kaeri") // i18n-skip
                .font(.plat(mac: 26, touch: 28, weight: .semibold))
                .foregroundStyle(.silk)
                .padding(.top, 24)
                .accessibilityAddTraits(.isHeader)
            Text(L.onboardingTagline)
                .font(.plat(mac: 15, touch: 17))
                .foregroundStyle(.silk)
                .padding(.top, 8)
            Spacer(minLength: 24)
            actions {
                #if os(macOS)
                setUpElsewhere
                #endif
                Button { page = .anilist } label: { Text(L.onboardingContinue).touchFill() }
                    .buttonStyle(PrimaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
                    .keyboardShortcut(.defaultAction)
                #if os(iOS)
                setUpElsewhere
                #endif
            }
        }
        .fixedSize(horizontal: false, vertical: false)
    }

    /// Set up from another device: the code this device shows, and the way to a cloud folder.
    private var setup: some View {
        PairShowSheet(first: true, restore: restore, leave: { page = .welcome }, finished: { finish(nil) })
            .environment(store)
            .environment(\.syncEmbedded, true)
    }

    private var anilist: some View {
        let working = store.accounts.isWorking("anilist")
        return VStack(alignment: .leading, spacing: 0) {
            heading("AniList")
            Text(L.accountsWelcomeDetail(tracker: AccountsModel.name("anilist")))
                .font(.plat(mac: 15, touch: 17))
                .foregroundStyle(.silk)
                .padding(.top, 8)
            Text(L.accountsOtherTrackersNote)
                .font(.plat(mac: 13, touch: 15))
                .foregroundStyle(.haze)
                .padding(.top, 6)
            Spacer(minLength: 24)
            actions {
                #if os(macOS)
                notNow
                signIn(working)
                #else
                signIn(working)
                notNow
                #endif
            }
        }
    }

    private var sources: some View {
        let trimmed = link.trimmingCharacters(in: .whitespacesAndNewlines)
        let looksRight = InviteLinks.looksLikeInvite(trimmed)
        return VStack(alignment: .leading, spacing: 0) {
            heading(L.onboardingWhereTitle)
            Text((devicePicksFolders ? L.onboardingWhereHosts : L.onboardingWhereHostsMobile) + " " + L.onboardingWhereStores)
                .font(.plat(mac: 15, touch: 17))
                .foregroundStyle(.silk)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 8)
            Text(L.onboardingOwnHeading)
                .font(.plat(mac: 13, touch: 15, weight: .semibold))
                .foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
                .padding(.top, 20)
            HStack(spacing: 8) {
                if devicePicksFolders {
                    Button { FolderPicker.choose { store.addFolder($0); finish(nil) } } label: {
                        Text(L.sourcesAddFolderButton).touchFill()
                    }
                    .buttonStyle(SecondaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
                }
                Button { navigator.addServerShown = true } label: {
                    Text(L.sourcesAddServerButton).touchFill()
                }
                .buttonStyle(SecondaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
            }
            .padding(.top, 8)
            Text(L.onboardingDemoHeading)
                .font(.plat(mac: 13, touch: 15, weight: .semibold))
                .foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
                .padding(.top, 20)
            Text(L.onboardingDemoDetail)
                .font(.plat(mac: 13, touch: 15))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 2)
            Text(L.onboardingInviteHeading)
                .font(.plat(mac: 13, touch: 15, weight: .semibold))
                .foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
                .padding(.top, 20)
            HStack(spacing: 8) {
                TextField(L.onboardingInviteHint, text: $link)
                    .textFieldStyle(.roundedBorder)
                    .autocorrectionDisabled()
                    #if os(iOS)
                    .textInputAutocapitalization(.never)
                    .keyboardType(.URL)
                    #endif
                    .onSubmit { if looksRight { finish(trimmed) } }
                    .onChange(of: link) { _, new in
                        let value = new.trimmingCharacters(in: .whitespacesAndNewlines)
                        if InviteLinks.looksLikeInvite(value) {
                            store.previewStore(value)
                        } else {
                            store.stores.clearPreview()
                        }
                    }
                Button(L.actionJoin) { finish(trimmed) }
                    .buttonStyle(JoinStyle(amber: openedFromLink && looksRight, height: fieldHeight, fontSize: buttonFont))
                    .disabled(!looksRight)
            }
            .padding(.top, 8)
            invitePreview
                .padding(.top, 8)
            Text(L.pluginRiskWarning)
                .font(.plat(mac: 13, touch: 15))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 12)
            Spacer(minLength: 16)
            actions {
                #if os(macOS)
                skipForNow
                playDemo
                #else
                playDemo
                skipForNow
                #endif
            }
        }
        .onDisappear { store.stores.clearPreview() }
    }

    @ViewBuilder
    private var invitePreview: some View {
        switch store.stores.preview {
        case .checking?:
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text(L.storesCheckingLink).font(.plat(mac: 13, touch: 15)).foregroundStyle(.haze)
            }
        case .failed(let reason, let contact)?:
            VStack(alignment: .leading, spacing: 4) {
                FailedLine(joinFailureWords(reason))
                if let contact {
                    Text(L.storeInviteContact(contact: contact))
                        .font(.plat(mac: 12, touch: 13)).foregroundStyle(.haze)
                }
            }
            .fixedSize(horizontal: false, vertical: true)
        case .store(let invited)?:
            Text(invited.name)
                .font(.plat(mac: 13, touch: 15, weight: .semibold))
                .foregroundStyle(.silk)
        case nil:
            EmptyView()
        }
    }

    private var playDemo: some View {
        Button {
            store.playDemo()
            finish(nil)
        } label: { Text(L.onboardingPlayDemo).touchFill() }
            .buttonStyle(PrimaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
            .keyboardShortcut(.defaultAction)
    }

    private var skipForNow: some View {
        Button { finish(nil) } label: { Text(L.onboardingSkipForNow).touchFill() }
            .buttonStyle(SecondaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
    }

    // MARK: The four pages, where reading is on

    /// Whether this build asks what Kaeri is for, which makes first run four pages.
    private var asks: Bool { store.medium.choosable }

    /// Whether the answer the medium page took includes reading.
    private var reads: Bool { (chosen ?? store.medium.answer) != .watch }

    @State private var bookSheet: AddBookSourceStep?

    /// Back on touch at the top left, Skip at the top right on the pages that have it.
    private var topBar: some View {
        HStack {
            #if os(iOS)
            if asks, page != .welcome, page != .setup {
                Button { back() } label: {
                    Image(systemName: "chevron.left")
                        .font(.ui(17, weight: .semibold))
                        .foregroundStyle(.silk)
                        .frame(width: 44, height: 44)
                        .background(Palette.raised, in: .circle)
                        .contentShape(.circle)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(L.actionBack)
            }
            #endif
            Spacer()
            if showsSkip {
                #if os(iOS)
                // A raised capsule beside the round Back, as the board draws them.
                Button { finish(nil) } label: {
                    Text(L.actionSkip)
                        .font(.ui(17))
                        .foregroundStyle(.silk)
                        .padding(.horizontal, 16)
                        .frame(height: 44)
                        .background(Palette.raised, in: .capsule)
                        .contentShape(.capsule)
                }
                .buttonStyle(.plain)
                #else
                Button(L.actionSkip) { finish(nil) }
                    .buttonStyle(.plain)
                    .font(.ui(13))
                    .foregroundStyle(.haze)
                    .pointerHover()
                #endif
            }
        }
        .frame(height: Device.isMac ? 28 : 44)
    }

    private var showsSkip: Bool {
        if asks { return page == .anilist || page == .sources }
        return page != .sources && page != .setup
    }

    private func back() {
        switch page {
        case .medium: page = .welcome
        case .anilist: page = .medium
        case .sources: page = .anilist
        case .welcome, .setup: break
        }
    }

    /// "2 of 4" over the title.
    private func step(_ number: Int) -> some View {
        Text(L.onboardingStep(step: number, total: 4))
            .font(.plat(mac: 12, touch: 15))
            .foregroundStyle(.haze)
            .monospacedDigit()
            .padding(.top, Device.isMac ? 0 : 8)
    }

    /// The title and its line, as every page of the four starts.
    private func top(_ number: Int, _ title: String, _ line: String) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            step(number)
            Text(title)
                .font(.plat(mac: 22, touch: 28, weight: .bold))
                .foregroundStyle(.silk)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityAddTraits(.isHeader)
                .padding(.top, 6)
            Text(line)
                .font(.plat(mac: 13, touch: 17))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, Device.isMac ? 6 : 10)
        }
    }

    /// Welcome, 1 of 4: words first, naming both media, so it reads the same before the choice.
    private var welcomeFour: some View {
        VStack(alignment: .leading, spacing: 0) {
            top(1, L.onboardingWelcomeTo, L.onboardingWelcomeLine)
            Text(L.onboardingWelcomeNote)
                .font(.plat(mac: 13, touch: 15))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, Device.isMac ? 14 : 24)
            Spacer(minLength: 24)
            actions {
                #if os(macOS)
                setUpElsewhere
                #endif
                Button { page = .medium } label: { Text(L.onboardingContinue).touchFill() }
                    .buttonStyle(PrimaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
                    .keyboardShortcut(.defaultAction)
                #if os(iOS)
                setUpElsewhere
                #endif
            }
        }
    }

    /// What Kaeri is for, 2 of 4: no Skip, nothing chosen, Continue off until a row is.
    private var medium: some View {
        VStack(alignment: .leading, spacing: 0) {
            top(2, L.onboardingMediumTitle, L.onboardingMediumLine)
            MediumChoiceRows(chosen: $chosen)
                .padding(.top, Device.isMac ? 18 : 24)
            Spacer(minLength: 16)
            actions {
                #if os(macOS)
                Button { back() } label: { Text(L.actionBack) }
                    .buttonStyle(SecondaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
                    .frame(maxWidth: .infinity, alignment: .leading)
                #endif
                Button {
                    guard let chosen else { return }
                    if store.medium.asking == .firstRun {
                        store.medium.answer(.firstRun, with: chosen)
                    } else if chosen != store.medium.answer {
                        store.medium.choose(chosen)
                    }
                    page = .anilist
                } label: { Text(L.onboardingContinue).touchFill() }
                    .buttonStyle(PrimaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont, offIsRaised: true))
                    .disabled(chosen == nil)
                    .keyboardShortcut(.defaultAction)
                    .accessibilityHint(chosen == nil ? L.onboardingMediumHint : "")
            }
        }
    }

    /// AniList, 3 of 4: AniList first, the trackers that serve the answer named for later.
    private var lists: some View {
        let working = store.accounts.isWorking("anilist")
        let answer = chosen ?? store.medium.answer
        return VStack(alignment: .leading, spacing: 0) {
            top(3, L.onboardingListsTitle, MediumWords.listsLine(answer, tracker: AccountsModel.name("anilist")))
            VStack(alignment: .leading, spacing: 2) {
                Text(AccountsModel.name("anilist"))
                    .font(.plat(mac: 13, touch: 17))
                    .foregroundStyle(.silk)
                Text(L.onboardingListsAnilistLine)
                    .font(.plat(mac: 12, touch: 15))
                    .foregroundStyle(.haze)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, Device.isMac ? 12 : 16)
            .padding(.vertical, Device.isMac ? 10 : 11)
            .frame(minHeight: Device.isMac ? 48 : 64)
            .background(.raised, in: .rect(cornerRadius: Device.isMac ? 10 : 26))
            .accessibilityElement(children: .combine)
            .padding(.top, Device.isMac ? 18 : 24)
            Text(MediumWords.listsLater(answer))
                .font(.plat(mac: 12, touch: 13))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.horizontal, Device.isMac ? 12 : 16)
                .padding(.top, 8)
            Spacer(minLength: 16)
            actions {
                #if os(macOS)
                Button { back() } label: { Text(L.actionBack) }
                    .buttonStyle(SecondaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
                    .frame(maxWidth: .infinity, alignment: .leading)
                notNow
                connect(working)
                #else
                connect(working)
                Button { page = .sources } label: {
                    Text(L.actionNotNow)
                        .font(.ui(17, weight: .semibold))
                        .foregroundStyle(.silk)
                        .frame(maxWidth: .infinity)
                        .frame(height: 44)
                        .contentShape(.rect)
                }
                .buttonStyle(.plain)
                #endif
            }
        }
    }

    private func connect(_ working: Bool) -> some View {
        Button {
            store.connect("anilist")
        } label: {
            HStack(spacing: 8) {
                if working { ProgressView().controlSize(.small) }
                Text(L.onboardingConnect(tracker: AccountsModel.name("anilist")))
            }
            .touchFill()
        }
        .buttonStyle(PrimaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
        .disabled(working)
        .keyboardShortcut(.defaultAction)
    }

    /// Where books come from, 4 of 4 after Read: a folder, a server, an invite. After Both, the
    /// same page with a Shows part and a Books part over the one invite field. No list of
    /// sources and no suggestion; the plugin line sits beside the invite field.
    private var books: some View {
        let trimmed = link.trimmingCharacters(in: .whitespacesAndNewlines)
        let looksRight = InviteLinks.looksLikeInvite(trimmed)
        let both = (chosen ?? store.medium.answer) == .both
        return VStack(alignment: .leading, spacing: 0) {
            if both {
                top(4, L.onboardingBothTitle, L.onboardingBothLine)
                partHeading(L.onboardingBothShows)
                    .padding(.top, Device.isMac ? 14 : 20)
                rows {
                    if devicePicksFolders {
                        bookRow(L.onboardingBooksFolder, L.onboardingBothShowsFolderLine(device: Device.name)) {
                            FolderPicker.choose { store.addFolder($0) }
                        }
                        rowRule
                    }
                    bookRow(L.onboardingBooksServer, L.onboardingBothShowsServerLine) { navigator.addServerShown = true }
                }
                .padding(.top, 6)
                partHeading(L.onboardingBothBooks)
                    .padding(.top, Device.isMac ? 14 : 20)
                bookRows
                    .padding(.top, 6)
            } else {
                top(4, L.onboardingBooksTitle, L.onboardingBooksLine)
                bookRows
                    .padding(.top, Device.isMac ? 16 : 24)
            }
            Text(L.onboardingBooksInviteHeading)
                .font(.plat(mac: 13, touch: 15, weight: .semibold))
                .foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
                .padding(.horizontal, Device.isMac ? 12 : 16)
                .padding(.top, Device.isMac ? 16 : 22)
            TextField(L.onboardingBooksInvitePlaceholder, text: $link)
                .textFieldStyle(.plain)
                .font(.plat(mac: 13, touch: 17))
                .foregroundStyle(.silk)
                .autocorrectionDisabled()
                #if os(iOS)
                .textInputAutocapitalization(.never)
                .keyboardType(.URL)
                #endif
                .padding(.horizontal, Device.isMac ? 12 : 18)
                .frame(height: Device.isMac ? 28 : 52)
                .background(.raised, in: .capsule)
                .overlay(Capsule().strokeBorder(Palette.faintRule, lineWidth: 1))
                .padding(.top, 8)
                .onSubmit { if looksRight { finish(trimmed) } }
                .onChange(of: link) { _, new in
                    let value = new.trimmingCharacters(in: .whitespacesAndNewlines)
                    if InviteLinks.looksLikeInvite(value) {
                        store.previewStore(value)
                    } else {
                        store.stores.clearPreview()
                    }
                }
            invitePreview
                .padding(.horizontal, Device.isMac ? 12 : 16)
                .padding(.top, 6)
            Text(L.onboardingBooksPluginLine)
                .font(.plat(mac: 12, touch: 13))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.horizontal, Device.isMac ? 12 : 16)
                .padding(.top, 8)
            Spacer(minLength: 16)
            actions {
                #if os(macOS)
                Button { back() } label: { Text(L.actionBack) }
                    .buttonStyle(SecondaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
                    .frame(maxWidth: .infinity, alignment: .leading)
                #endif
                Button { finish(looksRight ? trimmed : nil) } label: { Text(L.actionDone).touchFill() }
                    .buttonStyle(PrimaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
                    .keyboardShortcut(.defaultAction)
            }
        }
        .onDisappear { store.stores.clearPreview() }
        #if os(iOS)
        .fileImporter(isPresented: $pickingFolder, allowedContentTypes: [.folder]) { result in
            guard case .success(let url) = result else { return }
            BookFolders.add(url, to: store)
        }
        #endif
        .onChange(of: store.bookSources.added?.id) { _, added in
            // The sheet says how many series it found first, when it knows, and closes on Done.
            if added != nil, store.bookSources.added?.series == nil { bookSheet = nil }
        }
    }

    @State private var pickingFolder = false

    /// A folder and a server for books, on one raised card.
    private var bookRows: some View {
        rows {
            bookRow(L.onboardingBooksFolder, L.onboardingBooksFolderLine) { addFolder() }
            rowRule
            bookRow(L.onboardingBooksServer, L.onboardingBooksServerLine) { bookSheet = .server }
        }
    }

    private func rows<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        VStack(spacing: 0) { content() }
            .background(.raised, in: .rect(cornerRadius: Device.isMac ? 10 : 26))
    }

    private var rowRule: some View {
        Rectangle().fill(Palette.faintRule).frame(height: 1).padding(.leading, Device.isMac ? 12 : 16)
            .accessibilityHidden(true)
    }

    /// Shows or Books over its rows on the combined page, as a list's group heading.
    private func partHeading(_ text: String) -> some View {
        Text(text)
            .groupHeaderStyle()
            .padding(.horizontal, Device.isMac ? 12 : 16)
            .accessibilityAddTraits(.isHeader)
    }

    private func addFolder() {
        #if os(macOS)
        FolderPicker.choose { path in
            store.bookSources.addFolder(FolderRef(path: path, treeUri: nil, bookmark: nil, name: nil))
        }
        #else
        pickingFolder = true
        #endif
    }

    private func bookRow(_ title: String, _ line: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(.plat(mac: 13, touch: 17))
                        .foregroundStyle(.silk)
                    Text(line)
                        .font(.plat(mac: 12, touch: 15))
                        .foregroundStyle(.haze)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 8)
                Image(systemName: "chevron.right")
                    .font(.ui(Device.isMac ? 11 : 14, weight: .semibold))
                    .foregroundStyle(.haze)
                    .accessibilityHidden(true)
            }
            .padding(.horizontal, Device.isMac ? 12 : 16)
            .padding(.vertical, Device.isMac ? 9 : 11)
            .frame(minHeight: Device.isMac ? 48 : 64)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
    }

    // MARK: Parts

    /// Set up from another device: a quiet capsule on the Mac, a text button under Continue on touch.
    private var setUpElsewhere: some View {
        #if os(macOS)
        Button { page = .setup } label: { Text(L.firstSetupOther) }
            .buttonStyle(SecondaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
            .frame(maxWidth: .infinity, alignment: .leading)
        #else
        Button { page = .setup } label: {
            Text(L.firstSetupOther)
                .font(.ui(17, weight: .semibold))
                .foregroundStyle(.silk)
                .frame(maxWidth: .infinity)
                .frame(height: 44)
                .contentShape(.rect)
        }
        .buttonStyle(.plain)
        #endif
    }

    private var notNow: some View {
        Button { page = .sources } label: { Text(L.actionNotNow).touchFill() }
            .buttonStyle(SecondaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
    }

    private func signIn(_ working: Bool) -> some View {
        Button {
            store.connect("anilist")
        } label: {
            HStack(spacing: 8) {
                if working { ProgressView().controlSize(.small) }
                Text(L.librarySignedOutAction)
            }
            .touchFill()
        }
        .buttonStyle(PrimaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
        .disabled(working)
        .keyboardShortcut(.defaultAction)
    }

    private func heading(_ text: String) -> some View {
        Text(text)
            .font(.plat(mac: 22, touch: 28, weight: .semibold))
            .foregroundStyle(.silk)
            .accessibilityAddTraits(.isHeader)
    }

    /// The page's buttons: trailing on the Mac, as a sheet's are, and full width at the foot on
    /// touch, where the thumb is.
    @ViewBuilder
    private func actions<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        #if os(macOS)
        HStack(spacing: 8) {
            Spacer()
            content()
        }
        #else
        VStack(spacing: 10) {
            content()
        }
        .frame(maxWidth: .infinity)
        #endif
    }

    private var buttonHeight: CGFloat {
        #if os(macOS)
        28
        #else
        50
        #endif
    }

    private var fieldHeight: CGFloat {
        #if os(macOS)
        24
        #else
        36
        #endif
    }

    private var buttonFont: CGFloat {
        #if os(macOS)
        13
        #else
        17
        #endif
    }
}

extension View {
    /// The whole width on touch, where a page's buttons fill its foot; as it is on the Mac.
    @ViewBuilder
    func touchFill() -> some View {
        #if os(iOS)
        frame(maxWidth: .infinity)
        #else
        self
        #endif
    }
}

/// Join: quiet until a link is typed, amber when the link arrived by being opened.
private struct JoinStyle: ButtonStyle {
    let amber: Bool
    let height: CGFloat
    let fontSize: CGFloat

    func makeBody(configuration: Configuration) -> some View {
        if amber {
            PrimaryCapsuleStyle(height: height, fontSize: fontSize).makeBody(configuration: configuration)
        } else {
            SecondaryCapsuleStyle(height: height, fontSize: fontSize).makeBody(configuration: configuration)
        }
    }
}
