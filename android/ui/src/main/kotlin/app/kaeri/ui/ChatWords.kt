// The room chat's small shared pieces: the reactions in order, their glyphs and names, a typed
// line's look, and Send. The room's screens and the TV remote card both use them.

package app.kaeri.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import app.kaeri.design.ChatGlyphs
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.Reaction
import app.kaeri.i18n.Strings

/** The four reactions in the order the controls list them. */
val REACTIONS = listOf(Reaction.LAUGH, Reaction.GASP, Reaction.CLAP, Reaction.CRY)

fun reactionGlyph(reaction: Reaction): ImageVector = when (reaction) {
    Reaction.LAUGH -> ChatGlyphs.Laugh
    Reaction.GASP -> ChatGlyphs.Gasp
    Reaction.CLAP -> ChatGlyphs.Clap
    Reaction.CRY -> ChatGlyphs.Cry
}

fun reactionLabel(context: android.content.Context, reaction: Reaction): String = when (reaction) {
    Reaction.LAUGH -> Strings.roomReactLaugh(context)
    Reaction.GASP -> Strings.roomReactGasp(context)
    Reaction.CLAP -> Strings.roomReactClap(context)
    Reaction.CRY -> Strings.roomReactCry(context)
}

/** A typed line: the name in weight 600, then the words as written. Never a link. */
fun saidText(name: String, text: String, nameColor: Color = Color.Unspecified): AnnotatedString = buildAnnotatedString {
    withStyle(SpanStyle(fontWeight = FontWeight.SemiBold, color = nameColor)) { append(name) }
    append("  ") // i18n-skip
    append(text)
}

/** Send: a 48 circle with the paper plane, dimmed until there is something to send. */
@Composable
fun SendButton(enabled: Boolean, color: Color = ObiSilk.RaisedSelected, onClick: () -> Unit) {
    val context = LocalContext.current
    Box(
        Modifier
            .size(48.dp)
            .clip(CircleShape)
            .background(color)
            .clickable(enabled = enabled, role = androidx.compose.ui.semantics.Role.Button, onClick = onClick)
            .semantics { contentDescription = Strings.roomChatSend(context); if (!enabled) disabled() },
        contentAlignment = Alignment.Center,
    ) {
        Icon(ChatGlyphs.Send, contentDescription = null, tint = if (enabled) ObiSilk.Silk else ObiSilk.Haze.copy(alpha = 0.5f), modifier = Modifier.size(22.dp))
    }
}
