// The demo's own views: About the demo, with the credit block and the hosts it reaches, and the
// credit block itself, which shows wherever the work shows as a title.

package app.kaeri.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import app.kaeri.engine.ffi.demoInfo
import app.kaeri.i18n.R

/**
 * The credit block the licence asks for: the work, its makers and licence, with links to the
 * licence, the file page on Wikimedia Commons and the makers. Nothing here plays or fetches.
 */
@Composable
internal fun DemoCredit(modifier: Modifier = Modifier) {
    val info = androidx.compose.runtime.remember { demoInfo() }
    val uri = LocalUriHandler.current
    Column(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(info.credit, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            TextButton(onClick = { uri.openUri(info.licenceUrl) }) { Text(stringResource(R.string.sources_demo_link_licence)) }
            TextButton(onClick = { uri.openUri(info.filePageUrl) }) { Text(stringResource(R.string.sources_demo_link_commons)) }
            TextButton(onClick = { uri.openUri(info.makerUrl) }) { Text(stringResource(R.string.sources_demo_link_maker)) }
        }
    }
}

/** About the demo: the credit block, the hosts it reaches, that it never signs in, and Remove demo. */
@Composable
internal fun DemoAbout(name: String) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    ScreenFrame(title = stringResource(R.string.sources_demo_about), subtitle = name, onBack = navigator::back) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp)
                .readingWidth(),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            DemoCredit(Modifier.padding(horizontal = 16.dp))
            Text(
                stringResource(R.string.sources_demo_hosts),
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.padding(horizontal = 16.dp),
            )
            Text(
                stringResource(R.string.sources_demo_private),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 16.dp),
            )
            OutlinedButton(
                onClick = {
                    store.removeDemo()
                    navigator.back()
                },
                modifier = Modifier.padding(horizontal = 16.dp),
            ) { Text(stringResource(R.string.sources_demo_remove)) }
        }
    }
}
