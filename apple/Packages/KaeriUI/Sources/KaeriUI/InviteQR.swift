// The invite's QR code, drawn from the engine's matrix: ground-coloured modules on a silk plate
// with its quiet zone, so any phone camera reads it. Under a placeholder for the first 600 ms.

import KaeriDesign
import KaeriKit
import KaeriStrings
import SwiftUI

struct InviteQR: View {
    let link: String
    let device: Bool
    /// The invite's warning under the code; the sync sheets say their own words.
    var caption = true
    var label = L.storesQrLabel
    @State private var matrix: QrMatrix?
    @State private var late = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var side: CGFloat {
        #if os(iOS)
        200
        #else
        160
        #endif
    }

    var body: some View {
        VStack(spacing: 8) {
            ZStack {
                if let matrix {
                    Canvas { context, size in
                        let cells = CGFloat(Int(matrix.size) + 2 * Int(matrix.quietZone))
                        let cell = size.width / cells
                        context.fill(Path(CGRect(origin: .zero, size: size)), with: .color(.silk))
                        for row in 0..<Int(matrix.size) {
                            for column in 0..<Int(matrix.size) where matrix.modules[row * Int(matrix.size) + column] {
                                let at = CGPoint(x: (CGFloat(column) + CGFloat(matrix.quietZone)) * cell,
                                                 y: (CGFloat(row) + CGFloat(matrix.quietZone)) * cell)
                                context.fill(Path(CGRect(origin: at, size: CGSize(width: cell + 0.5, height: cell + 0.5))),
                                             with: .color(.ground))
                            }
                        }
                    }
                    .transition(.opacity)
                } else if late {
                    RoundedRectangle(cornerRadius: 4).fill(Palette.raised)
                }
            }
            .frame(width: side, height: side)
            .clipShape(.rect(cornerRadius: 4))
            .animation(reduceMotion ? nil : .fadeIn, value: matrix != nil)
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(label)
            if caption { Text(device ? L.storesQrWarningDevice : L.storesQrWarning)
                .font(.plat(mac: 12, touch: 15))
                .foregroundStyle(.haze)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true) }
        }
        .frame(maxWidth: .infinity)
        .task(id: link) {
            matrix = nil
            late = false
            let text = link
            let made = await Task.detached { qrMatrix(text: text) }.value
            matrix = made
        }
        .task(id: link) {
            try? await Task.sleep(for: .milliseconds(600))
            late = true
        }
    }
}
