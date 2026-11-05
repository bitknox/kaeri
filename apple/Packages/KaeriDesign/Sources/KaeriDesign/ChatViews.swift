// Watch party chat, the parts every screen shares: a line of the conversation, the conversation as
// a list, and the field with its wait and its count. Plain data in, so the player, the popover and
// sheet and the lobby draw them the same way.

import SwiftUI

/// A line of the conversation, as a list draws it.
public struct ChatLineData: Identifiable, Equatable, Sendable {
    public let id: UInt64
    /// Who said it, nil for a reaction's line.
    public let name: String?
    public let text: String
    /// This device's own line: its name reads "You" in haze.
    public let you: Bool

    public init(id: UInt64, name: String?, text: String, you: Bool = false) {
        self.id = id
        self.name = name
        self.text = text
        self.you = you
    }
}

/// One line: the name in semibold, then what was said. A reaction's line is haze alone.
public struct ChatLineText: View {
    let line: ChatLineData

    public init(_ line: ChatLineData) { self.line = line }

    public var body: some View {
        Group {
            if let name = line.name {
                Text(name).fontWeight(.semibold).foregroundStyle(line.you ? Palette.haze : Palette.silk)
                    + Text("  ")
                    + Text(line.text).foregroundStyle(Palette.silk)
            } else {
                Text(line.text).foregroundStyle(Palette.haze)
            }
        }
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(line.name.map { "\($0), \(line.text)" } ?? line.text)
    }
}

/// The conversation, newest last, as tall as its lines up to `maxHeight`, then scrolling, kept at
/// the end as lines come.
public struct ChatConversation: View {
    let lines: [ChatLineData]
    let size: CGFloat
    let spacing: CGFloat
    let maxHeight: CGFloat
    let font: Font?
    @State private var content: CGFloat = 0

    /// `font`, when given, stands in for the system font at `size`.
    public init(
        _ lines: [ChatLineData], size: CGFloat = 12, spacing: CGFloat = 7, maxHeight: CGFloat = 200, font: Font? = nil
    ) {
        self.lines = lines
        self.size = size
        self.font = font
        self.spacing = spacing
        self.maxHeight = maxHeight
    }

    public var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: spacing) {
                    ForEach(lines) { line in
                        ChatLineText(line).id(line.id)
                    }
                }
                .font(font ?? .ui(size))
                .frame(maxWidth: .infinity, alignment: .leading)
                .onGeometryChange(for: CGFloat.self, of: { $0.size.height }) { content = $0 }
            }
            .scrollBounceBehavior(.basedOnSize)
            .frame(height: min(max(content, 1), maxHeight))
            .onChange(of: lines.last?.id) { _, last in
                guard let last else { return }
                withAnimation(.easeOut(duration: 0.2)) { proxy.scrollTo(last, anchor: .bottom) }
            }
            .onAppear {
                if let last = lines.last?.id { proxy.scrollTo(last, anchor: .bottom) }
            }
        }
    }
}

/// How the field is drawn.
public enum ChatFieldStyle: Sendable {
    /// Over the video: glass, a capsule, silk.
    case overVideo
    /// In a popover or sheet: the raised fill and a hairline, 12 pt.
    case panel
    /// In the lobby and on iPhone's keyboard bar: the same, larger.
    case roomy
}

/// The chat field. Return sends. Near the limit it shows the characters left at its end, and while
/// a wait runs, "Wait 3 s" in its place.
public struct ChatTextField: View {
    @Binding var text: String
    let placeholder: String
    let style: ChatFieldStyle
    let limit: Int
    let remaining: Int?
    let waitLabel: String?
    let waitSpoken: String?
    let submit: () -> Void
    var focus: FocusState<Bool>.Binding?

    public init(
        text: Binding<String>, placeholder: String, style: ChatFieldStyle, limit: Int, remaining: Int?,
        waitLabel: String?, waitSpoken: String?, focus: FocusState<Bool>.Binding? = nil, submit: @escaping () -> Void
    ) {
        _text = text
        self.placeholder = placeholder
        self.style = style
        self.limit = limit
        self.remaining = remaining
        self.waitLabel = waitLabel
        self.waitSpoken = waitSpoken
        self.focus = focus
        self.submit = submit
    }

    private var size: CGFloat {
        switch style {
        case .overVideo: 13
        case .panel: 12
        case .roomy: 15
        }
    }

    public var body: some View {
        HStack(spacing: 8) {
            field
            if let waitLabel {
                Text(waitLabel)
                    .font(.ui(size - 1))
                    .foregroundStyle(Palette.haze)
                    .monospacedDigit()
                    .transition(.opacity)
                    .accessibilityLabel(waitSpoken ?? waitLabel)
            } else if let remaining {
                Text("\(remaining)")
                    .font(.ui(size - 1))
                    .foregroundStyle(Palette.haze)
                    .monospacedDigit()
                    .transition(.opacity)
                    .accessibilityHidden(true)
            }
        }
        .padding(.horizontal, style == .overVideo ? 14 : 10)
        .frame(minHeight: style == .roomy ? 40 : 32)
        .background { backdrop }
        .animation(.easeOut(duration: 0.2), value: waitLabel)
        .animation(.easeOut(duration: 0.2), value: remaining)
    }

    @ViewBuilder
    private var field: some View {
        let base = TextField(placeholder, text: $text)
            .textFieldStyle(.plain)
            .font(.ui(size))
            .foregroundStyle(Palette.silk)
            .tint(Palette.silk)
            .onSubmit(submit)
            .submitLabel(.send)
            .onChange(of: text) { _, new in
                if new.count > limit { text = String(new.prefix(limit)) }
            }
        if let focus {
            base.focused(focus)
        } else {
            base
        }
    }

    @ViewBuilder
    private var backdrop: some View {
        switch style {
        case .overVideo:
            Capsule().fill(.overVideo(0.72))
                .overlay(Capsule().strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
        case .panel, .roomy:
            RoundedRectangle(cornerRadius: style == .roomy ? 20 : 8)
                .fill(Palette.ground.opacity(0.5))
                .overlay(
                    RoundedRectangle(cornerRadius: style == .roomy ? 20 : 8)
                        .strokeBorder(focused ? Palette.silk.opacity(0.55) : Palette.rule, lineWidth: focused ? 2 : 1))
        }
    }

    private var focused: Bool { focus?.wrappedValue ?? false }
}
