// The poster pieces several screens share: a poster with its caption, its placeholder, and the
// quiet line a list leaves when it has nothing else to say.

package app.kaeri.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.kaeri.design.ArtShape
import app.kaeri.design.ShowTitle
import app.kaeri.engine.model.CountLine
import app.kaeri.engine.model.Entry
import app.kaeri.i18n.R

/** A plain line of haze text where a screen has nothing else to say. */
@Composable
internal fun Quiet(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodyLarge,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

/**
 * A poster with its title in two lines beneath, and a detail line: the first genre unless [detail]
 * says otherwise, with a water dot when [fresh]. Opens the title, flying from [from].
 */
@Composable
internal fun PosterCell(
    entry: Entry,
    modifier: Modifier = Modifier,
    from: String = "poster",
    detail: String? = entry.genreLine,
    fresh: Boolean = false,
    /** Said as the title opens, before the page goes up. */
    onOpen: (() -> Unit)? = null,
    /** Lines the detail may take; Search's, which can end in where the title plays, takes two. */
    detailLines: Int = 1,
    /** The count line, first under the title: "12 ep · Dub 8". Search, its rails and the Library say it. */
    count: CountLine? = null,
    /** Whether the cell is narrow, so its count takes the short form. */
    narrow: Boolean = true,
) {
    val navigator = LocalNavigator.current
    val newWord = stringResource(R.string.poster_new)
    Column(
        modifier
            .shared(Shared.Page, entry.id, from)
            // The water dot is colour alone; the state says it too.
            .semantics { if (fresh) stateDescription = newWord }
            .clickable(onClickLabel = stringResource(R.string.title_open), role = Role.Button) {
                onOpen?.invoke()
                navigator.openTitle(entry.id, from)
            },
    ) {
        androidx.compose.foundation.layout.BoxWithConstraints(Modifier.fillMaxWidth()) {
            CoverArt(entry, maxWidth, Modifier.size(maxWidth, maxWidth * 1.5f).shared(Shared.Cover, entry.id, from))
        }
        Text(
            entry.showTitle,
            style = ShowTitle.caption,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 8.dp),
        )
        count?.let { CountText(it, narrow, Modifier.padding(top = 2.dp)) }
        detail?.let {
            Row(
                Modifier.padding(top = 2.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (fresh) Box(Modifier.size(6.dp).background(MaterialTheme.colorScheme.secondary, CircleShape))
                Text(
                    it,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = detailLines,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

/**
 * A poster's place before its title arrives: the plate, and its caption's lines as bars. Each
 * line is as high as its text style's line, in sp, so the placeholder grows with the font scale
 * just as the poster will: [titleLines] title lines in the show face, [lines] lines under them.
 */
@Composable
internal fun PosterPlaceholder(titleLines: Int = 1, lines: Int = 1) {
    val density = androidx.compose.ui.platform.LocalDensity.current
    val titleLine = with(density) { app.kaeri.design.ShowTitle.caption.lineHeight.toDp() }
    val line = with(density) { MaterialTheme.typography.bodySmall.lineHeight.toDp() }
    Column {
        androidx.compose.foundation.layout.BoxWithConstraints(Modifier.fillMaxWidth()) {
            app.kaeri.design.CoverPlate(null, Modifier.size(maxWidth, maxWidth * 1.5f).clip(ArtShape.cover(maxWidth)))
        }
        TextPlaceholder(0.8f, titleLine, Modifier.padding(top = 8.dp))
        repeat(titleLines - 1) { TextPlaceholder(0.6f, titleLine) }
        TextPlaceholder(0.5f, line, Modifier.padding(top = 2.dp))
        repeat(lines - 1) { TextPlaceholder(0.4f, line) }
    }
}
