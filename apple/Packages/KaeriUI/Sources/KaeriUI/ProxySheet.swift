// The Mac and iPad proxy form: asks for a proxy, or edits the saved one. A proxy is saved only
// once the check through it passes.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The proxy form: asked for when the route switch goes on, or editing the saved proxy. The
/// proxy is saved only once the check through it passes; Cancel leaves things as they were.
struct ProxySheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    /// Whether this adds a proxy or edits the saved one.
    var mode: ProxyFormMode = .add
    @State private var draft: ProxyDraft?
    /// What the check under way is for, saved once it passes.
    @State private var checking: ProxyDraft?
    /// The draft as the form opened, to tell a change from it.
    @State private var opened: ProxyDraft?

    private var editing: Bool { mode == .edit }

    var body: some View {
        let draft = Binding(get: { self.draft ?? ProxyDraft(store) }, set: { self.draft = $0 })
        let trial = store.providers.trial
        VStack(alignment: .leading, spacing: 0) {
            Text(editing ? L.torrentsSheetEditProxy : L.torrentsProxyRoute)
                .font(.mac(15, weight: .semibold))
                .macLineHeight(20)
                .foregroundStyle(.silk)
            Text(editing ? ProxyWords.editNote : L.torrentsRisk)
                .font(.mac(13))
                .macLineHeight(18)
                .foregroundStyle(.silk)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 4)

            proxyFields(draft).padding(.top, 18)
            if editing { note(ProxyWords.passwordKept).padding(.top, 8) }
            note(ProxyWords.checkNote).padding(.top, 14)
            if draft.wrappedValue.invalid { note(L.torrentsProxyInvalid).padding(.top, 8) }

            // As tall as the most the line can say, from the start, so the form and its
            // buttons hold still while the check runs and when it fails.
            ZStack(alignment: .topLeading) {
                ProxyCheckLine(trial: .said(.failed))
                    .hidden()
                    .accessibilityHidden(true)
                ProxyCheckLine(trial: trial)
            }
            .padding(.top, 14)

            HStack(spacing: 10) {
                Spacer()
                Button(L.actionCancel) { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    .keyboardShortcut(.cancelAction)
                Button(L.actionSave) { send(draft.wrappedValue) }
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
                    .disabled(!draft.wrappedValue.complete || trial == .checking)
            }
            .padding(.top, 20)
        }
        .padding(30)
        .sheetWidth(540)
        .background(.raised)
        .onAppear {
            let first = ProxyDraft(store)
            self.draft = first
            opened = first
        }
        .modifier(ProxySaving(checking: checking, draft: self.draft) { dismiss() })
        // Changed from what it opened with, or with a check under way, it stays open on iPad.
        .holdsInput(when: self.draft != opened || checking != nil)
    }

    /// The proxy's address and sign-in, held still while the check through them runs.
    private func proxyFields(_ draft: Binding<ProxyDraft>) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                labelled(L.torrentsProxyHost) { field("proxy.example", text: draft.host) }
                labelled(L.torrentsProxyPort) { field("1080", text: draft.port) }.frame(width: 88)
            }
            HStack(spacing: 12) {
                labelled(L.torrentsProxyUser) { field("", text: draft.user) }
                labelled(L.torrentsProxyPassword) { field("", text: draft.password, secure: true) }
            }
        }
        .disabled(store.providers.trial == .checking)
    }

    private func send(_ draft: ProxyDraft) {
        checking = draft
        store.check(draft)
    }

    private func label(_ text: String) -> some View {
        Text(text).font(.mac(12)).macLineHeight(16).foregroundStyle(.haze)
    }

    private func note(_ text: String) -> some View {
        Text(text)
            .font(.mac(12))
            .macLineHeight(17)
            .foregroundStyle(.haze)
            .fixedSize(horizontal: false, vertical: true)
    }

    @ViewBuilder
    private func field(_ prompt: String, text: Binding<String>, secure: Bool = false) -> some View {
        Group {
            if secure {
                SecureField("", text: text, prompt: Text(prompt))
            } else {
                TextField("", text: text, prompt: Text(prompt))
            }
        }
        .textFieldStyle(.plain)
        .font(.mac(13))
        .padding(.horizontal, 12)
        .frame(height: 30)
        .background(.ground, in: .rect(cornerRadius: 8))
        .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
    }

    private func labelled(_ title: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            label(title)
            content()
        }
    }
}
