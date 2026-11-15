// The people the viewer follows on AniList, on a title page: the row under the list button, which
// opens the bottom sheet with each of them, and the marks on the episodes where each stopped.

package app.kaeri.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SheetValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.design.ObiSilk
import app.kaeri.design.tabular
import app.kaeri.engine.model.FollowedEntry
import app.kaeri.engine.model.FollowedMark
import app.kaeri.i18n.R
import app.kaeri.store.FollowingWords
import kotlin.math.roundToInt

/** How wide the marks' column is at the end of an episode row: three pictures and "+N". */
internal val MarkColumn = 80.dp

/** A mark's pictures, 24 across, as the board's episode rows have them. */
internal val MarkPicture = 24.dp

/**
 * A person's picture in a circle, from the same image path and cache as the covers; with none,
 * the first letter of their name on the selected raised fill.
 */
@Composable
internal fun PersonPicture(name: String, url: String?, size: Dp, modifier: Modifier = Modifier) {
    val pixels = with(LocalDensity.current) { size.roundToPx() }
    Box(modifier.size(size).clip(CircleShape).background(ObiSilk.RaisedSelected), contentAlignment = Alignment.Center) {
        if (url != null) {
            ArtworkImage(url, plate = ObiSilk.RaisedSelected, maxPixelSize = pixels, modifier = Modifier.fillMaxSize())
        } else {
            Text(
                name.take(1).uppercase(),
                color = ObiSilk.Silk,
                fontWeight = FontWeight.SemiBold,
                fontSize = with(LocalDensity.current) { (size * 0.46f).toSp() },
                lineHeight = with(LocalDensity.current) { (size * 0.46f).toSp() },
                maxLines = 1,
            )
        }
    }
}

/**
 * Pictures overlapping by 6, each ringed 2 in [ring] so it stands off the one under it, at most
 * [max] of them, then "+N" for the rest. Decoration: what it says is said by what it is on.
 */
@Composable
internal fun FollowedStack(people: List<FollowedMark>, size: Dp, ring: Color, modifier: Modifier = Modifier, max: Int = FollowingWords.MARK_PICTURES) {
    val context = LocalContext.current
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
                    Modifier
                        .height(size)
                        .widthIn(min = size)
                        .background(ObiSilk.RaisedSelected, CircleShape)
                        .padding(start = inset + 5.dp, end = inset),
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

/** The ring around each picture of a stack. */
private val RING = 2.dp

/** The people [FollowedEntry] names, as marks for a stack. */
internal fun List<FollowedEntry>.asMarks(): List<FollowedMark> = map { FollowedMark(it.name, it.avatarUrl) }

/**
 * The marks at the end of an episode row, in their fixed column: the people who stopped at this
 * episode, fading in as they come. The sentence they stand for joins the row's description.
 */
@Composable
internal fun MarkSlot(marks: List<FollowedMark>, modifier: Modifier = Modifier) {
    Box(modifier.size(MarkColumn, MarkPicture + RING * 2).then(spokenMarks(marks)), contentAlignment = Alignment.CenterEnd) {
        MarksFading(marks) { FollowedStack(it, MarkPicture, MaterialTheme.colorScheme.surface) }
    }
}

/** [content] for [marks] while there are any, fading in on the fade-in token and out on fade-out. */
@Composable
internal fun MarksFading(marks: List<FollowedMark>, content: @Composable (List<FollowedMark>) -> Unit) {
    // The last marks shown, kept while they fade out.
    var kept by androidx.compose.runtime.remember { mutableStateOf(marks) }
    if (marks.isNotEmpty()) kept = marks
    androidx.compose.animation.AnimatedVisibility(
        marks.isNotEmpty(),
        enter = androidx.compose.animation.fadeIn(app.kaeri.design.Motion.fadeIn()),
        exit = androidx.compose.animation.fadeOut(app.kaeri.design.Motion.fadeOut()),
        label = "marks", // i18n-skip
    ) { content(kept) }
}

/**
 * What a screen reader adds to the row the marks are on, merged into its description after the
 * title and facts: "Ines, Dario and Oskar watched up to here." Nothing while there are none.
 */
@Composable
internal fun spokenMarks(marks: List<FollowedMark>): Modifier {
    if (marks.isEmpty()) return Modifier
    val sentence = FollowingWords.spoken(LocalContext.current, marks)
    return Modifier.semantics { contentDescription = sentence }
}

/**
 * The phone's People you follow row, under the list button: three pictures, the heading and the
 * first names, one button that opens the sheet with all of them.
 */
@Composable
internal fun FollowingRow(people: List<FollowedEntry>, modifier: Modifier = Modifier, inset: Dp = 16.dp) {
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    var open by rememberSaveable { mutableStateOf(false) }
    val heading = stringResource(R.string.more_following)
    val names = FollowingWords.names(context, people.map { it.name })
    val help = stringResource(R.string.following_open_help)
    val openLabel = stringResource(R.string.action_open)
    Row(
        modifier
            .padding(top = 8.dp)
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .clickable { open = true }
            .clearAndSetSemantics {
                contentDescription = listOf(heading, names, help).joinToString(". ") // i18n-skip
                role = Role.Button
                onClick(label = openLabel) { open = true; true }
            }
            .padding(horizontal = inset),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        FollowedStack(people.asMarks(), 32.dp, colors.surface, max = FollowingWords.ROW_PICTURES)
        Column(Modifier.weight(1f)) {
            Text(heading, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(names, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Icon(Icons.Chevron, contentDescription = null, tint = colors.onSurfaceVariant, modifier = Modifier.size(20.dp))
    }
    if (open) FollowingSheet(people) { open = false }
}

/**
 * Everyone followed who has the show, in the M3 bottom sheet: picture, name, where they are, and
 * their score at the end. Closed by its handle, the scrim or Back.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun FollowingSheet(people: List<FollowedEntry>, onDismiss: () -> Unit) {
    val state = rememberBottomSheetState(SheetValue.Hidden, setOf(SheetValue.Hidden, SheetValue.Expanded))
    val colors = MaterialTheme.colorScheme
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = state,
        containerColor = colors.surfaceContainerHigh,
        // As the app's other sheets: the scrim black at 32 %, the handle haze at 40 %.
        scrimColor = Color.Black.copy(alpha = 0.32f),
        dragHandle = { androidx.compose.material3.BottomSheetDefaults.DragHandle(color = colors.onSurfaceVariant.copy(alpha = 0.4f)) },
    ) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(start = 24.dp, end = 24.dp, bottom = 24.dp)) {
            Text(stringResource(R.string.more_following), style = MaterialTheme.typography.titleLarge, modifier = Modifier.semantics { heading() })
            Text(
                stringResource(R.string.more_following_note),
                style = MaterialTheme.typography.bodyMedium,
                color = colors.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp, bottom = 8.dp),
            )
            for (person in people) FollowedPersonRow(person)
        }
    }
}

/** One person in the sheet: 40 picture, name, the line under it, and the score at the end. */
@Composable
private fun FollowedPersonRow(person: FollowedEntry) {
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val said = FollowingWords.said(context, person)
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .padding(vertical = 8.dp)
            .clearAndSetSemantics { contentDescription = said },
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        PersonPicture(person.name, person.avatarUrl, 40.dp)
        Column(Modifier.weight(1f)) {
            Text(person.name, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            val line = FollowingWords.line(context, person)
            if (line.isNotEmpty()) Text(line, style = MaterialTheme.typography.bodyMedium.tabular(), color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        FollowingWords.score(person)?.let {
            Text(FollowingWords.scoreText(context, it), style = MaterialTheme.typography.bodyMedium.tabular(), color = colors.onSurfaceVariant, maxLines = 1)
        }
    }
}
