// First launch (native-ux, First launch): three short pages, then the app. Welcome says what
// Kaeri is; AniList offers the sign-in; Sources takes an invite link and says whose code a
// plugin is. Every page can be skipped, and each step is in Settings afterwards. A
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
        // i18n-skip: a shot's sheet names; welcome-sources opens on the last page
        if navigator.launchSheet == "welcome" || navigator.launchSheet == "welcome-sources" || navigator.launchSheet == "welcome-setup" { return true }
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
    /// A shot's request to open on the last page.
    @State private var startOnSources = false
    /// Restore from a cloud folder, chosen on the code page: the picker opens once this ends.
    @State private var restoring = false
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    #endif

    func body(content: Content) -> some View {
        content
            .task {
                if await FirstLaunchFlag.shows(navigator, store: store) {
                    startOnSources = navigator.launchSheet == "welcome-sources"
                    // The code page takes welcome-setup itself, once it is up.
                    if navigator.launchSheet == "welcome" || startOnSources { navigator.launchSheet = nil }
                    showing = true
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
        FirstLaunch(startOnSources: startOnSources, finish: { link in
            joining = link
            showing = false
        }, restore: {
            restoring = true
            showing = false
        })
    }

    private func done() {
        UserDefaults.standard.set(true, forKey: FirstLaunchFlag.key)
        if restoring {
            restoring = false
            store.sync.sheet = .picker
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

/// The three pages.
struct FirstLaunch: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    /// Ends first launch, with the invite link Join took, if it took one.
    let finish: (String?) -> Void
    /// Ends first launch and opens the cloud folder picker.
    var restore: () -> Void = {}

    enum Page: Int { case welcome, anilist, sources, setup }
    @State private var page: Page
    init(startOnSources: Bool = false, finish: @escaping (String?) -> Void, restore: @escaping () -> Void = {}) {
        self.finish = finish
        self.restore = restore
        _page = State(initialValue: startOnSources ? .sources : .welcome)
    }
    @State private var link = ""
    /// The link came from a kaeri:// link, so Join is the way on.
    @State private var openedFromLink = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Spacer()
                if page != .sources, page != .setup {
                    Button(L.actionSkip) { finish(nil) }
                        .buttonStyle(.plain)
                        .font(.plat(mac: 13, touch: 17))
                        .foregroundStyle(.haze)
                        .pointerHover()
                }
            }
            .frame(height: 28)
            Group {
                switch page {
                case .welcome: welcome
                case .anilist: anilist
                case .sources: sources
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
        .task {
            // i18n-skip: a shot's sheet name
            if navigator.launchSheet == "welcome-setup" { navigator.launchSheet = nil; page = .setup }
        }
        .sheet(isPresented: Bindable(navigator).addServerShown) { AddServerSheet() }
        .onChange(of: store.servers.added) { _, added in
            if added != nil, page == .sources { finish(nil) }
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
