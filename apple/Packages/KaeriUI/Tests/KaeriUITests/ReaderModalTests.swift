#if os(macOS)
// The Mac reader covers its window: while a book is open Tab stays in the book (Mac R8-M7:
// VoiceOver and Tab reached the covered series page and sidebar). The accessibility half of the
// fix can't be read in a test process, whose SwiftUI views give no accessibility tree; Tab can.

import AppKit
@testable import KaeriStore
@testable import KaeriUI
import SwiftUI
import Testing

extension WindowEventTests {
    @MainActor
    @Suite(.serialized)
    struct ReaderModalTests {
        private let size = CGSize(width: 800, height: 600)

        /// A window's content as a series page would be, a row to press and a field, under the reader.
        private func covered(_ model: ReaderModel) -> some View {
            VStack {
                Button("Covered series row") {}
                TextField("Covered field", text: .constant(""))
            }
            .frame(width: size.width, height: size.height)
            .hostsReader(model)
            .environment(AppStore())
            .environment(Navigator())
        }

        @Test func theOpenReaderKeepsTab() async throws {
            let model = ReaderModel()
            let window = TapWindow(covered(model), size: size)
            defer { window.close() }
            try await window.settle()

            model.open(work: "a:900304", chapter: "c1")
            try await window.wait(0.8)
            #expect(!(window.firstResponder is NSText), "the reader holds the keys, not the covered field")

            try await window.key("\t", code: 48)
            #expect(!(window.firstResponder is NSText), "Tab took the focus to the covered window's field")
        }
    }
}
#endif
