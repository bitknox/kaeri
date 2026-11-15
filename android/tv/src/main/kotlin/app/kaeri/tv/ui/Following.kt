// The people the viewer follows on AniList, on the TV's title page: the row between the buttons and
// the episodes, the side panel it opens, and the marks on the episode stills.

package app.kaeri.tv.ui

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import app.kaeri.design.ObiSilk
import app.kaeri.design.tabular
import app.kaeri.engine.model.FollowedEntry
import app.kaeri.engine.model.FollowedMark
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.FollowingWords
import kotlin.math.roundToInt

/** The ring around each picture of a stack, so it stands off the one under it. */
private val RING = 2.dp

/** A focused item grows to this, as the buttons do. */
private const val FOCUSED_SCALE = 1.05f

/**
 * A person's picture in a circle, through the covers' image path and cache; with none, the first
 * letter of their name on the selected raised fill.
 */
@Composable
fun PersonPicture(name: String, url: String?, size: Dp, modifier: Modifier = Modifier) {
    Box(modifier.size(size).clip(CircleShape).background(ObiSilk.RaisedSelected), contentAlignment = Alignment.Center) {
        if (url != null) {
            ArtImage(url, ObiSilk.RaisedSelected, pixels(size), Modifier.fillMaxSize())
        } else {
            val letter = with(LocalDensity.current) { (size * 0.46f).toSp() }
            Text(name.take(1).uppercase(), color = ObiSilk.Silk, fontWeight = FontWeight.SemiBold, fontSize = letter, lineHeight = letter, maxLines = 1)
        }
    }
}

/**
 * Pictures overlapping by 6, each ringed in [ring], at most three, then "+N" for the rest.
 * Decoration: what it says is said by what it is on.
 */
@Composable
fun FollowedStack(people: List<FollowedMark>, size: Dp, ring: Color, modifier: Modifier = Modifier, max: Int = FollowingWords.MARK_PICTURES) {
    val context = androidx.compose.ui.platform.LocalContext.current
    val shown = people.take(max)
    val more = people.size - shown.size
    Row(modifier.clearAndSetSemantics {}, horizontalArrangement = Arrangement.spacedBy(-(6.dp + RING * 2)), verticalAlignment = Alignment.CenterVertically) {
        for (person in shown) {
            Box(Modifier.background(ring, CircleShape).padding(RING)) { PersonPicture(person.name, person.avatarUrl, size) }
        }
        if (more > 0) {
            val inset = size * 0.18f
            Box(Modifier.background(ring, CircleShape).padding(RING)) {
                Box(
                    Modifier.height(size).widthIn(min = size).background(ObiSilk.RaisedSelected, CircleShape).padding(start = inset + 5.dp, end = inset),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        FollowingWords.more(context, more),
                        style = MaterialTheme.typography.labelMedium.tabular(),
                        color = ObiSilk.Silk,
                        fontWeight = FontWeight.SemiBold,
                        fontSize = maxOf(10f, (size.value * 0.44f).roundToInt().toFloat()).sp,
                        maxLines = 1,
                    )
                }
            }
        }
    }
}

/** The marks on an episode's still, bottom right: 28 pictures ringed in the ground at 85 %. */
@Composable
fun StillMarks(marks: List<FollowedMark>, modifier: Modifier = Modifier) {
    // The last marks shown, kept while they fade out.
    var kept by remember { mutableStateOf(marks) }
    if (marks.isNotEmpty()) kept = marks
    androidx.compose.animation.AnimatedVisibility(
        marks.isNotEmpty(),
        modifier = modifier,
        enter = androidx.compose.animation.fadeIn(app.kaeri.design.Motion.fadeIn()),
        exit = androidx.compose.animation.fadeOut(app.kaeri.design.Motion.fadeOut()),
        label = "marks", // i18n-skip
    ) { FollowedStack(kept, 28.dp, ObiSilk.Ground.copy(alpha = 0.85f)) }
}

/**
 * People you follow, between the buttons and the episodes: three pictures, the heading and the
 * first names on one line, one focus stop. Focused it fills silk and grows to 1.05, as the buttons
 * do; OK opens [followingPanel].
 */
@Composable
fun FollowingRow(people: List<FollowedEntry>, onOpen: () -> Unit, modifier: Modifier = Modifier) {
    val context = androidx.compose.ui.platform.LocalContext.current
    var focused by remember { mutableStateOf(false) }
    val heading = Strings.moreFollowing(context)
    val names = FollowingWords.names(context, people.map { it.name })
    val help = Strings.followingOpenHelp(context)
    val openLabel = context.getString(R.string.action_open)
    Surface(
        onClick = onOpen,
        modifier = modifier
            .heightIn(min = 56.dp)
            .onFocusChanged { focused = it.isFocused }
            // Said as one, with what OK does; the surface's own focus and click stay on the node.
            .semantics {
                contentDescription = listOf(heading, names, help).joinToString(". ") // i18n-skip
                onClick(label = openLabel) { onOpen(); true }
            },
        shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(14.dp)),
        colors = ClickableSurfaceDefaults.colors(
            containerColor = ObiSilk.Raised,
            contentColor = ObiSilk.Silk,
            focusedContainerColor = ObiSilk.Silk,
            focusedContentColor = ObiSilk.Ground,
            pressedContainerColor = ObiSilk.Silk,
            pressedContentColor = ObiSilk.Ground,
        ),
        scale = ClickableSurfaceDefaults.scale(focusedScale = FOCUSED_SCALE),
    ) {
        Row(
            Modifier.heightIn(min = 56.dp).padding(start = 12.dp, end = 22.dp).clearAndSetSemantics {},
            horizontalArrangement = Arrangement.spacedBy(14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            FollowedStack(people.toMarks(), 32.dp, if (focused) ObiSilk.Silk else ObiSilk.Raised, max = FollowingWords.ROW_PICTURES)
            Text(heading, style = MaterialTheme.typography.titleSmall, maxLines = 1)
            Text(
                names,
                style = MaterialTheme.typography.bodyLarge.copy(fontSize = 15.sp, lineHeight = 20.sp),
                color = if (focused) ObiSilk.RaisedSelected else ObiSilk.Haze,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/** The people as marks, for a stack. */
fun List<FollowedEntry>.toMarks(): List<FollowedMark> = map { FollowedMark(it.name, it.avatarUrl) }

/**
 * Everyone followed who has the show, in the side panel: picture, name, where they are, and their
 * score. The first is focused as it opens; Back closes it.
 */
fun followingPanel(context: Context, people: List<FollowedEntry>): ActionSheet =
    ActionSheet(Strings.moreFollowing(context), width = 420.dp, body = { FollowingPanelBody(people) })

@Composable
private fun FollowingPanelBody(people: List<FollowedEntry>) {
    val context = androidx.compose.ui.platform.LocalContext.current
    val first = remember { FocusRequester() }
    Column(Modifier.fillMaxWidth()) {
        Text(
            Strings.moreFollowingNote(context),
            style = MaterialTheme.typography.bodyLarge.copy(fontSize = 15.sp, lineHeight = 20.sp),
            color = ObiSilk.Haze,
            modifier = Modifier.padding(start = 16.dp, bottom = 16.dp),
        )
        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            itemsIndexed(people, key = { _, person -> person.name }) { index, person ->
                PersonRow(person, if (index == 0) Modifier.focusRequester(first) else Modifier)
            }
        }
    }
    LaunchedEffect(Unit) {
        repeat(5) {
            androidx.compose.runtime.withFrameNanos { }
            if (first.tryFocus()) return@LaunchedEffect
        }
    }
}

/** One person in the panel, a focus stop of its own so the D-pad walks the list; OK does nothing. */
@Composable
private fun PersonRow(person: FollowedEntry, modifier: Modifier) {
    val context = androidx.compose.ui.platform.LocalContext.current
    var focused by remember { mutableStateOf(false) }
    val said = FollowingWords.said(context, person)
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 64.dp)
            .onFocusChanged { focused = it.isFocused }
            .focusable()
            .clearAndSetSemantics { contentDescription = said }
            .background(if (focused) ObiSilk.Silk else ObiSilk.Raised, RoundedCornerShape(14.dp))
            .padding(horizontal = 18.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val ink = if (focused) ObiSilk.Ground else ObiSilk.Silk
        val quiet = if (focused) ObiSilk.RaisedSelected else ObiSilk.Haze
        PersonPicture(person.name, person.avatarUrl, 40.dp)
        Column(Modifier.weight(1f)) {
            Text(person.name, style = MaterialTheme.typography.titleSmall.copy(lineHeight = 22.sp), color = ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
            val line = FollowingWords.line(context, person)
            if (line.isNotEmpty()) {
                Text(line, style = MaterialTheme.typography.bodySmall.copy(fontSize = 13.sp, lineHeight = 18.sp).tabular(), color = quiet, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        FollowingWords.score(person)?.let {
            Text(FollowingWords.scoreText(context, it), style = MaterialTheme.typography.bodyLarge.copy(fontSize = 15.sp).tabular(), color = ink, maxLines = 1)
        }
    }
}
