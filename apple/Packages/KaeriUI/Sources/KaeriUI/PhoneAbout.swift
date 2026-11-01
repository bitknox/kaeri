// About on iPhone, from Settings: what MacAbout's window says, as a page of the settings stack.
// Nagare, the name, the version, the licence, and the acknowledgements and the website; the bug
// report; Legal and contact with the legal line; then updates: the daily check's switch, what it
// found, and the action.

#if os(iOS)
import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct PhoneAboutPage: View {
    @Environment(\.openURL) private var openURL
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        ScrollView {
            VStack(spacing: 0) {
                Nagare(cell: 5)
                    .accessibilityHidden(true)
                Text("Kaeri") // i18n-skip
                    .font(.ui(22, weight: .bold))
                    .foregroundStyle(.silk)
                    .padding(.top, 20)
                Text(AppVersion.long)
                    .font(.ui(15))
                    .monospacedDigit()
                    .foregroundStyle(.haze)
                    .padding(.top, 4)
                Text(L.aboutLicence)
                    .font(.ui(15))
                    .foregroundStyle(.haze)
                    .multilineTextAlignment(.center)
                    .padding(.top, 12)
                HStack(spacing: 10) {
                    NavigationLink(value: PhoneSettingsPage.acknowledgements) {
                        Text(L.aboutAcknowledgements)
                    }
                    .buttonStyle(SecondaryCapsuleStyle(height: 44, fontSize: 17))
                    Button(L.aboutWebsite) { openURL(AppVersion.website) }
                        .buttonStyle(SecondaryCapsuleStyle(height: 44, fontSize: 17))
                }
                .padding(.top, 28)
                Button(L.aboutReportBug) { ReportBugAction(store: store, navigator: navigator)() }
                    .buttonStyle(SecondaryCapsuleStyle(height: 44, fontSize: 17))
                    .padding(.top, 10)
                Text(L.aboutReportBugDetail)
                    .font(.ui(13))
                    .foregroundStyle(.haze)
                    .multilineTextAlignment(.center)
                    .padding(.top, 8)
                Button(L.aboutLegalLink) { openURL(AppVersion.legal) }
                    .buttonStyle(SecondaryCapsuleStyle(height: 44, fontSize: 17))
                    .padding(.top, 20)
                Text(L.legalHostsNoContent)
                    .font(.ui(13))
                    .foregroundStyle(.haze)
                    .multilineTextAlignment(.center)
                    .padding(.top, 8)
                PhoneCrashSection()
                    .padding(.top, 32)
                PhoneUpdatesCard()
                    .padding(.top, 32)
            }
            .frame(maxWidth: .infinity)
            .padding(EdgeInsets(top: 48, leading: 16, bottom: 32, trailing: 16))
        }
        .background(.ground)
        .navigationTitle(L.aboutTitle)
        .navigationBarTitleDisplayMode(.inline)
    }
}

/// The notices of what the app carries: libmpv and what it is built from, and the subtitle font.
struct PhoneAcknowledgementsPage: View {
    private static let files = ["NOTICES-libmpv", "NOTICES-fonts"]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 28) {
                ForEach(Self.files, id: \.self) { name in
                    if let text = Self.text(name) {
                        Text(text)
                            .font(.ui(12, design: .monospaced))
                            .foregroundStyle(.haze)
                            .textSelection(.enabled)
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(16)
        }
        .background(.ground)
        .navigationTitle(L.aboutAcknowledgements)
        .navigationBarTitleDisplayMode(.inline)
    }

    private static func text(_ name: String) -> String? {
        Bundle.main.url(forResource: name, withExtension: "txt").flatMap { try? String(contentsOf: $0, encoding: .utf8) }
    }
}

/// Crash reports on the phone: the two switches, what the system still holds, and what is saved.
struct PhoneCrashSection: View {
    @Environment(AppStore.self) private var store
    @State private var viewing: CrashSummary?

    var body: some View {
        let saving = store.settings.isOn(.saveCrashReports)
        VStack(alignment: .leading, spacing: 10) {
            Text(L.settingsCrashSave).font(.ui(13)).foregroundStyle(.haze).textCase(.uppercase)
                .accessibilityAddTraits(.isHeader)
            VStack(alignment: .leading, spacing: 14) {
                Toggle(isOn: Binding(get: { saving }, set: { store.setSwitch(.saveCrashReports, on: $0) })) {
                    PhoneRowTitle(title: L.settingsCrashSave, note: L.settingsCrashSaveDetail)
                }
                .tint(.amber)
                if saving {
                    Toggle(isOn: Binding(get: { store.settings.isOn(.askAfterCrash) }, set: { store.setSwitch(.askAfterCrash, on: $0) })) {
                        PhoneRowTitle(title: L.settingsCrashAsk, note: L.settingsCrashAskDetail)
                    }
                    .tint(.amber)
                    ForEach(store.crash.held, id: \.id) { crash in
                        HStack {
                            Text(L.settingsCrashOsItem(when: CrashWords.when(crash))).font(.ui(15)).foregroundStyle(.silk)
                            Spacer()
                            Button(L.settingsCrashOsAdd) { store.crash.add(crash) }
                                .buttonStyle(SecondaryCapsuleStyle(height: 36, raisedSurface: true, fontSize: 15))
                        }
                    }
                }
                if saving || !store.crash.saved.isEmpty {
                    Text(store.crash.saved.isEmpty ? L.settingsCrashNone : L.settingsCrashSaved)
                        .font(.ui(15)).foregroundStyle(.haze)
                    ForEach(store.crash.saved, id: \.id) { crash in
                        HStack {
                            Text(L.reportCrashItem(when: CrashWords.when(crash), place: CrashWords.place(crash.place)))
                                .font(.ui(15)).foregroundStyle(.silk)
                            Spacer()
                            Button(L.settingsCrashView) { viewing = crash }
                                .buttonStyle(SecondaryCapsuleStyle(height: 36, raisedSurface: true, fontSize: 15))
                            Button(L.settingsCrashDelete) { store.crash.delete(crash.id) }
                                .buttonStyle(SecondaryCapsuleStyle(height: 36, raisedSurface: true, fontSize: 15))
                        }
                    }
                    if !store.crash.saved.isEmpty {
                        Button(L.settingsCrashDeleteAll) { store.crash.deleteAll() }
                            .buttonStyle(SecondaryCapsuleStyle(height: 36, raisedSurface: true, fontSize: 15))
                    }
                }
            }
            .padding(14)
            .background(Palette.raised, in: .rect(cornerRadius: 12))
            Text(L.settingsCrashFootnote).font(.ui(13)).foregroundStyle(.haze)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .onAppear { store.crash.refresh() }
        .onChange(of: saving) { _, on in
            if on { store.crash.offerHeld() } else { store.crash.forgetHeld() }
        }
        .sheet(item: $viewing) { CrashTextSheet(crash: $0) }
    }
}
#endif
