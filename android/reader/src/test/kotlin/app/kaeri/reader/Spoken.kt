// What TalkBack reads on a screen drawn in a test: each node in the merged tree that says
// something, with its role, state and the label of its double-tap. Written beside the shots as
// talkback-<name>.txt, so the spoken words can be checked against the board's table.

package app.kaeri.reader

import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.junit4.ComposeTestRule
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import java.io.File

fun ComposeTestRule.spoken(name: String) {
    val lines = onAllNodes(SemanticsMatcher("anything") { true }).fetchSemanticsNodes().mapNotNull { node ->
        val c = node.config
        val said = c.getOrNull(SemanticsProperties.ContentDescription)?.joinToString(", ")
            ?: c.getOrNull(SemanticsProperties.Text)?.joinToString(", ") { it.text }
            ?: return@mapNotNull null
        if (said.isBlank()) return@mapNotNull null
        buildList {
            add(said)
            c.getOrNull(SemanticsProperties.Role)?.let { add("role $it") }
            c.getOrNull(SemanticsProperties.Selected)?.let { if (it) add("selected") }
            c.getOrNull(SemanticsProperties.StateDescription)?.let { add("state $it") }
            c.getOrNull(SemanticsActions.OnClick)?.let { add("double-tap: " + (it.label ?: "activate")) }
            c.getOrNull(SemanticsProperties.LiveRegion)?.let { add("live $it") }
        }.joinToString(" | ")
    }
    File(roborazziSystemPropertyOutputDirectory(), "talkback-$name.txt").apply { parentFile?.mkdirs() }.writeText(lines.joinToString("\n") + "\n")
}
