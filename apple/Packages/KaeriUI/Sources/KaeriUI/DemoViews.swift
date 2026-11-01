// The demo's parts that more than one screen shows: the credit block the licence asks for, and
// the About the demo detail with its links, the hosts it reaches and Remove demo.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The credit block, shown wherever the work shows as a title.
struct DemoCredit: View {
    @Environment(AppStore.self) private var store
    var size: CGFloat = 12

    var body: some View {
        let info = store.demo.info
        VStack(alignment: .leading, spacing: 4) {
            Text(L.sourcesDemoCredit)
                .font(.plat(mac: size, touch: size + 2))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 14) {
                link(L.sourcesDemoLinkLicence, info.licenceUrl)
                link(L.sourcesDemoLinkCommons, info.filePageUrl)
                link(L.sourcesDemoLinkMaker, info.makerUrl)
            }
        }
    }

    @ViewBuilder
    private func link(_ title: String, _ url: String) -> some View {
        if let url = URL(string: url) {
            Link(title, destination: url)
                .font(.plat(mac: size, touch: size + 2))
                .foregroundStyle(.water)
        }
    }
}

/// About the demo: what it is, the credit, the only hosts it reaches, and taking it out.
struct DemoAbout: View {
    @Environment(AppStore.self) private var store
    var size: CGFloat = 13

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(L.sourcesDemoAbout)
                .groupHeaderStyle()
                .accessibilityAddTraits(.isHeader)
            DemoCredit(size: size)
            Text(L.sourcesDemoHosts)
                .font(.plat(mac: size, touch: size + 2))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
            Text(L.sourcesDemoPrivate)
                .font(.plat(mac: size, touch: size + 2))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
            Button(L.sourcesDemoRemove, role: .destructive) { store.removeDemo() }
                .buttonStyle(SecondaryCapsuleStyle(height: ScreenFailed.buttonHeight, fontSize: ScreenFailed.buttonFont))
        }
    }
}
