// "Private" beside the episode in the player's header, while what plays is private.

import KaeriDesign
import KaeriStrings
import SwiftUI

/// The eye and "Private", in the header's quieter colour, beside the episode line.
struct PlayerPrivateLabel: View {
    let size: CGFloat

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: "eye.slash")
                .font(.system(size: size - 1, weight: .medium))
                .accessibilityHidden(true)
            Text(L.privateBadge)
                .font(.system(size: size))
        }
        .fixedSize()
        .accessibilityElement(children: .combine)
        .accessibilityLabel(L.privateBadgeLabel)
    }
}
