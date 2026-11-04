// The watch party in the player, from the people button: how the connection stands, who is in
// the room and who is ready, this member's ready switch, the invite link to copy, the server and
// room as text for someone joining with Syncplay itself, and Leave party. A popover on the Mac, a
// sheet on iPhone and iPad, as the player's other menus are; the Windows app's flyout says
// the same words. Each part keeps its place as the room changes.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct PartyPanel: View {
    @Environment(AppStore.self) private var store
    let metrics: PlayerMetrics
    /// Copy invite link was pressed: "Copied" stands in its label for 2 s.
    @State private var copied = false
    /// Where the conversation starts, for a sheet opened from the column's "n more in chat".
    static let chatPlace = "chat"

    var body: some View {
        let party = store.party
        VStack(alignment: .leading, spacing: 0) {
            if let view = party.party {
                header(view)
                members(party.members)
                    .padding(.top, 10)
                Text(party.members.map(PartyWords.readyLine) ?? " ")
                    .font(.system(size: small))
                    .foregroundStyle(.haze)
                    .contentTransition(.opacity)
                    .animation(.quick, value: party.members.map(PartyWords.readyLine))
                    .padding(.top, 8)
                Toggle(
                    L.partyReadyAction,
                    isOn: Binding(get: { view.ready }, set: { party.setReady($0) })
                )
                .toggleStyle(.switch)
                .font(.system(size: metrics.body))
                .padding(.top, 10)
                ChatSection(metrics: metrics)
                    .id(Self.chatPlace)
                    .padding(.top, 12)
                Divider().padding(.vertical, 12)
                invite(view)
                plain(view)
                    .padding(.top, 12)
                if let motd = view.motd, !motd.isEmpty {
                    Text(motd)
                        .font(.system(size: small))
                        .foregroundStyle(.haze)
                        .fixedSize(horizontal: false, vertical: true)
                        .padding(.top, 10)
                }
                Button {
                    party.leave()
                } label: {
                    Text(party.leaving ? L.partyLeaving : L.partyLeave)
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .controlSize(metrics.touch ? .large : .regular)
                .disabled(party.leaving)
                .padding(.top, 14)
            }
        }
        .foregroundStyle(.silk)
        .padding(metrics.touch ? 20 : 16)
        // A readable column on a phone held sideways and on iPad, where the sheet runs wide.
        .frame(width: metrics.touch ? nil : 300, alignment: .leading)
        .frame(maxWidth: metrics.touch ? 520 : nil, alignment: .leading)
        .frame(maxWidth: metrics.touch ? .infinity : nil)
    }

    private var small: CGFloat { metrics.touch ? 13 : 11 }

    private func header(_ view: PartyView) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(L.partyTitle)
                .font(.system(size: metrics.body + 2, weight: .semibold))
            HStack(spacing: 6) {
                if case .reconnecting = view.connection {
                    ProgressView().controlSize(.mini)
                }
                Text(PartyWords.connection(view))
                    .contentTransition(.opacity)
            }
            .font(.system(size: small))
            .foregroundStyle(.haze)
            .animation(.quick, value: PartyWords.connection(view))
        }
        .accessibilityElement(children: .combine)
    }

    @ViewBuilder
    private func members(_ members: [PartyMember]?) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            if let members {
                ForEach(members, id: \.name) { member in
                    MemberRow(member: member, metrics: metrics, muted: store.party.chat.isMuted(member.name))
                        .contextMenu {
                            if !member.you {
                                let muted = store.party.chat.isMuted(member.name)
                                Button(muted ? L.partyUnmute(name: member.name) : L.partyMute(name: member.name)) {
                                    store.party.chat.mute(member.name, !muted)
                                }
                            }
                        }
                }
            } else {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text(L.partyWaitingMembers)
                        .foregroundStyle(.haze)
                }
                .font(.system(size: metrics.body))
                .frame(minHeight: metrics.touch ? 44 : 36)
            }
        }
        .animation(.standard, value: members)
    }

    private func invite(_ view: PartyView) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Button {
                guard let link = view.link ?? view.invite else { return }
                PlayerModel.pasteboard(link)
                copied = true
            } label: {
                Label {
                    Text(copied ? L.actionCopied : L.partyCopyInvite)
                        .contentTransition(.opacity)
                } icon: {
                    Image(systemName: copied ? "checkmark" : "doc.on.doc")
                }
                .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .controlSize(metrics.touch ? .large : .regular)
            .disabled(view.link == nil && view.invite == nil)
            .animation(.quick, value: copied)
            .task(id: copied) {
                guard copied else { return }
                try? await Task.sleep(for: .seconds(2))
                copied = false
            }
            if let code = view.code {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(L.partyCode).foregroundStyle(.haze)
                    Text(code)
                        .font(.system(size: small + 3, weight: .medium, design: .monospaced))
                        .foregroundStyle(.silk)
                        .textSelection(.enabled)
                }
                .font(.system(size: small))
                .accessibilityElement(children: .combine)
            }
            Text(view.link == nil && view.invite == nil
                 ? L.partyInviteAfterPlay
                 : L.partyInviteDetail)
                .font(.system(size: small))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    /// The server and room as text, for a friend with Syncplay's own app.
    private func plain(_ view: PartyView) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(L.partyOtherApp)
                .font(.system(size: small, weight: .semibold))
            Text(L.partyOtherAppDetail(server: view.server, room: view.room))
                .font(.system(size: small))
                .foregroundStyle(.haze)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

/// A member: ready or not, their name, whether their file is this one, and their state.
private struct MemberRow: View {
    let member: PartyMember
    let metrics: PlayerMetrics
    let muted: Bool

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: member.ready == true ? "checkmark" : "circle")
                .font(.system(size: metrics.body - 2, weight: member.ready == true ? .semibold : .regular))
                .foregroundStyle(member.ready == true ? .silk : .haze)
                .frame(width: 16)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 1) {
                Text(PartyWords.name(member))
                    .font(.system(size: metrics.body))
                if let file = PartyWords.file(member) {
                    Text(file)
                        .font(.system(size: metrics.touch ? 13 : 11))
                        .foregroundStyle(.haze)
                }
            }
            .lineLimit(1)
            Spacer(minLength: 8)
            Text(muted ? L.partyMemberMuted : PartyWords.state(member))
                .font(.system(size: metrics.touch ? 13 : 11, weight: member.ready == true && !muted ? .semibold : .regular))
                .foregroundStyle(member.ready == true && !muted ? .silk : .haze)
                .contentTransition(.opacity)
        }
        .frame(minHeight: metrics.touch ? 44 : 36)
        .animation(.quick, value: member.ready)
        .animation(.quick, value: muted)
        .accessibilityElement(children: .combine)
    }
}

/// The people button's words: what it opens.
enum PartyButton {
    static func label(inParty: Bool) -> String {
        inParty ? L.partyTitle : L.partyOpen
    }

    static func symbol(inParty: Bool) -> String {
        inParty ? "person.2.fill" : "person.2"
    }
}

/// The conversation under the members, and the field. Hidden, off, or in the room's words.
struct ChatSection: View {
    let metrics: PlayerMetrics
    @Environment(AppStore.self) private var store
    @FocusState private var focus: Bool

    private var small: CGFloat { metrics.touch ? 13 : 11 }

    var body: some View {
        let chat = store.party.chat
        VStack(alignment: .leading, spacing: 8) {
            if chat.hidden {
                VStack(alignment: .leading, spacing: 4) {
                    Text(L.partyChatHidden).font(.system(size: metrics.body))
                    Text(L.partyChatHiddenLine).font(.system(size: small)).foregroundStyle(.haze)
                    Button(L.partyChatShow) { chat.show() }
                        .buttonStyle(.bordered)
                        .controlSize(metrics.touch ? .large : .small)
                        .padding(.top, 4)
                }
            } else if !chat.allowed {
                Text(L.partyChatOff)
                    .font(.system(size: small))
                    .foregroundStyle(.haze)
            } else {
                let lines = chat.rows.map(ChatLineData.init)
                if !lines.isEmpty {
                    ChatConversation(lines, size: metrics.touch ? 13 : 12, spacing: metrics.touch ? 9 : 7, maxHeight: metrics.touch ? 260 : 200)
                }
                if chat.failed != nil {
                    HStack(spacing: 4) {
                        Text(L.partyChatNotSent)
                        Text("·")
                        Button(L.partyChatRetry) { chat.retry() }.buttonStyle(.plain).underline().foregroundStyle(.silk)
                    }
                    .font(.system(size: small))
                    .foregroundStyle(.haze)
                }
                ForEach(chat.state?.muted ?? [], id: \.self) { name in
                    HStack(spacing: 4) {
                        Text(L.partyMutedLine(name: name))
                        Text("·")
                        Button(L.partyUnmuteShort) { chat.mute(name, false) }.buttonStyle(.plain).underline().foregroundStyle(.silk)
                    }
                    .font(.system(size: small))
                    .foregroundStyle(.haze)
                }
                HStack(spacing: 8) {
                    ChatTextField(
                        text: Binding(get: { chat.draft }, set: { chat.draft = $0 }),
                        placeholder: L.partyChatField, style: metrics.touch ? .roomy : .panel, limit: chat.maxLength,
                        remaining: chat.remaining,
                        waitLabel: chat.wait > 0 ? L.partyChatWait(seconds: chat.wait) : nil,
                        waitSpoken: chat.wait > 0 ? L.partyChatWaitSpoken(seconds: chat.wait) : nil,
                        focus: $focus
                    ) { chat.submit() }
                    if metrics.touch {
                        ChatSendButton(enabled: chat.canSend) { chat.submit() }
                    }
                }
            }
        }
        .foregroundStyle(.silk)
        .animation(.standard, value: chat.hidden)
        .animation(.standard, value: chat.failed)
    }
}

/// Send, as a round arrow-up button at the field's end.
struct ChatSendButton: View {
    let enabled: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: "arrow.up")
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(enabled ? Palette.silk : Palette.haze)
                .frame(width: 40, height: 40)
                .background(enabled ? Palette.raisedSelected : Palette.raised, in: .circle)
                .contentShape(.circle)
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .animation(.quick, value: enabled)
        .accessibilityLabel(L.partyChatSend)
    }
}
