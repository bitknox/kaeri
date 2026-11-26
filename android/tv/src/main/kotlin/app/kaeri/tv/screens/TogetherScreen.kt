// Watch together on the TV: the place in the rail after Library. The room this TV is in, with
// Open the player and Leave; joining, which comes from a paired phone since nobody types a link
// on a remote; and the recent rooms, rejoined with OK. Rooms start from the player.

package app.kaeri.tv.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.RoomCommand
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.room.RoomCopy
import app.kaeri.store.room.RoomWords
import app.kaeri.tv.remote.LocalRemoteHost
import app.kaeri.tv.shell.TvNavigator
import app.kaeri.tv.ui.AmberButton
import app.kaeri.tv.ui.ArtCard
import app.kaeri.tv.ui.ArtImage
import app.kaeri.tv.ui.ButtonLabel
import app.kaeri.tv.ui.Canvas
import app.kaeri.tv.ui.CardText
import app.kaeri.tv.ui.EnterFocus
import app.kaeri.tv.ui.FocusMemory
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.TonalButton
import app.kaeri.tv.ui.TvIcons
import app.kaeri.tv.ui.firstFocus
import app.kaeri.tv.ui.pixels
import app.kaeri.tv.ui.remembered

/** The place: the room this TV is in, joining from a phone, and the recent rooms. */
@Composable
fun TogetherScreen(nav: TvNavigator, memory: FocusMemory) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val c = remember(context) { RoomCopy(context) }
    val remote = LocalRemoteHost.current
    val room = store.room
    val view by room.view.collectAsStateWithLifecycle()
    val members by room.members.collectAsStateWithLifecycle()
    val recent by room.recent.collectAsStateWithLifecycle()
    val rejoinFailed by room.rejoinFailed.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { room.loadRecent() }
    Column(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .verticalScroll(rememberScrollState())
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp, bottom = 48.dp),
    ) {
        Text(stringResource(R.string.together_title), style = MaterialTheme.typography.headlineMedium, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        Text(Strings.roomPlaceLine(context), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, modifier = Modifier.padding(top = 8.dp).widthIn(max = 640.dp))
        val v = view
        if (v != null) {
            Heading(Strings.roomPlaceYours(context))
            val info = v.roomEpisode
            val title = info?.id?.let { titles[it]?.title } ?: info?.title ?: v.name
            Text(title, style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk)
            val line = RoomWords.line(c, v, members, System.currentTimeMillis(), v.position ?: 0.0)
            Text(
                line.title + " · " + line.detail, // i18n-skip: a separator
                style = MaterialTheme.typography.bodyLarge,
                color = ObiSilk.Haze,
                modifier = Modifier.padding(top = 4.dp),
            )
            Row(Modifier.padding(top = 16.dp), horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
                AmberButton(onClick = { room.send(RoomCommand.OpenPlayer) }, modifier = Modifier.firstFocus(memory, "together/open")) {
                    ButtonLabel(Strings.roomOpenPlayer(context))
                }
                TonalButton(onClick = { room.send(RoomCommand.Leave) }, modifier = Modifier.remembered(memory, "together/leave")) {
                    ButtonLabel(Strings.roomLeaveMenuPlain(context))
                }
            }
        }
        // Joining is from a phone: an invite opened there offers Join on this TV.
        Heading(Strings.roomPlaceJoin(context))
        val phones = remote?.phones.orEmpty()
        if (phones.isNotEmpty()) {
            Text(phones.joinToString(", ") { it.name }, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze) // i18n-skip
        }
        if (remote != null) {
            val pair = Modifier.padding(top = 12.dp)
            TonalButton(onClick = remote::openPairing, icon = TvIcons.People, modifier = if (v == null) pair.firstFocus(memory, "together/pair") else pair.remembered(memory, "together/pair")) {
                ButtonLabel(stringResource(R.string.party_tv_pair))
            }
        }
        Heading(Strings.roomRecent(context))
        Text(Strings.roomRecentNote(context), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(bottom = 12.dp))
        val rooms = recent
        if (rooms.isEmpty()) {
            Text(stringResource(R.string.together_recent_empty), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
        } else {
            LazyRow(horizontalArrangement = Arrangement.spacedBy(20.dp), contentPadding = PaddingValues(vertical = Canvas.FocusRoom)) {
                itemsIndexed(rooms, key = { _, it -> it.roomId }) { index, kept ->
                    val entry = titles[kept.id]
                    val title = entry?.title ?: kept.title
                    val line = c.recentLine(kept)
                    val rejoin = { room.rejoin(kept.roomId) }
                    // The first card takes the first focus when nothing stands above it.
                    val key = "together/room/${kept.roomId}"
                    val focus = if (index == 0 && v == null && remote == null) Modifier.firstFocus(memory, key) else Modifier.remembered(memory, key)
                    if (entry != null) {
                        app.kaeri.tv.ui.WideCard(entry, onClick = rejoin, modifier = focus, line = line)
                    } else {
                        Column(Modifier.width(Canvas.CardWidth)) {
                            ArtCard(rejoin, Canvas.CardWidth, Canvas.CardHeight, focus, label = listOf(title, line).joinToString(", ")) { // i18n-skip
                                ArtImage(kept.coverUrl, plate = ObiSilk.Raised, maxPixelSize = pixels(Canvas.CardWidth), modifier = Modifier.fillMaxSize())
                            }
                            CardText(title, line)
                        }
                    }
                }
            }
            // Nobody answered in a recent room: the lead in weight 600, then what to do.
            rooms.firstOrNull { it.roomId == rejoinFailed }?.let { failed ->
                val title = titles[failed.id]?.title ?: failed.title
                Text(
                    buildAnnotatedString {
                        withStyle(SpanStyle(fontWeight = FontWeight.SemiBold, color = ObiSilk.Silk)) { append(Strings.roomErrorNobodyAnsweredLead(context)) }
                        append(" ") // i18n-skip
                        append(Strings.roomErrorNobodyAnsweredLine(context))
                    },
                    style = MaterialTheme.typography.bodyLarge,
                    color = ObiSilk.Haze,
                    modifier = Modifier.padding(top = 8.dp).widthIn(max = 720.dp),
                )
                Text(title, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
            }
        }
    }
    EnterFocus(memory)
}

@Composable
private fun Heading(text: String) {
    Text(text, style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.padding(top = 36.dp, bottom = 8.dp).semantics { heading() })
}
