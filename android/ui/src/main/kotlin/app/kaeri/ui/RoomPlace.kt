// Watch together on the phone: the place, pushed from Home's people action. What it is, your room
// when you are in one with the player closed (Open the player, Leave…), Join a room with an invite
// link, and Recent rooms, which keep their key so Rejoin asks nobody. Joining that fails says why
// where it failed: under the field, or under the recent room's row.

package app.kaeri.ui

import android.content.Context
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.ObiSilk
import app.kaeri.design.ShowTitle
import app.kaeri.engine.ffi.parseRoomTarget
import app.kaeri.engine.model.RoomCommand
import app.kaeri.engine.model.RoomRecent
import app.kaeri.engine.model.RoomView
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.room.RoomCopy
import app.kaeri.store.room.RoomWords
import kotlinx.coroutines.launch

const val TOGETHER_ROUTE = "together"

/** Home's people action, before the account button: opens Watch together. */
@Composable
internal fun PeopleAction() {
    val navigator = LocalNavigator.current
    val label = stringResource(R.string.together_people_button)
    IconButton(onClick = { navigator.open(TOGETHER_ROUTE) }, modifier = Modifier.semantics { contentDescription = label }) {
        Icon(Icons.People, contentDescription = null)
    }
}

/** What a pasted invite says before Join: nothing yet, a room's link, or an older version's. */
private enum class LinkShape { NONE, ROOM, OLD }

private fun linkShape(text: String): LinkShape {
    val target = text.takeIf { it.isNotBlank() }?.let { parseRoomTarget(it.trim()) } ?: return LinkShape.NONE
    return if (target.old != null) LinkShape.OLD else LinkShape.ROOM
}

@Composable
internal fun RoomPlaceScreen() {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val context = LocalContext.current
    val c = remember(context) { RoomCopy(context) }
    val room = store.room
    val view by room.view.collectAsStateWithLifecycle()
    val recent by room.recent.collectAsStateWithLifecycle()
    val joinFailed by room.joinFailed.collectAsStateWithLifecycle()
    val rejoinFailed by room.rejoinFailed.collectAsStateWithLifecycle()
    var field by rememberSaveable { mutableStateOf("") }
    var leaving by remember { mutableStateOf(false) }
    val clipboard = LocalClipboard.current
    val scope = rememberCoroutineScope()
    LaunchedEffect(Unit) { room.loadRecent() }
    val shape = linkShape(field)
    fun join() {
        if (shape == LinkShape.ROOM) room.joinLink(field.trim())
    }
    ScreenFrame(title = stringResource(R.string.together_title), onBack = navigator::back) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(top = padding.calculateTopPadding() + 8.dp, bottom = 48.dp)
                .padding(horizontal = 16.dp)
                .readingWidth(),
        ) {
            Haze(Strings.roomPlaceLine(context), Modifier.padding(bottom = 20.dp))
            view?.let { current ->
                YourRoom(current, c, onLeave = { leaving = true })
                androidx.compose.foundation.layout.Spacer(Modifier.height(24.dp))
            }
            Heading(Strings.roomPlaceJoin(context), Modifier.padding(bottom = 8.dp))
            val failedHere = joinFailed != null && joinFailed == field.trim()
            OutlinedTextField(
                value = field,
                onValueChange = { field = it },
                modifier = Modifier.fillMaxWidth(),
                placeholder = { Text(Strings.roomPlaceLink(context)) },
                singleLine = true,
                isError = shape == LinkShape.OLD || failedHere,
                textStyle = MaterialTheme.typography.bodyLarge.copy(fontFamily = FontFamily.Monospace),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Go),
                keyboardActions = KeyboardActions(onGo = { join() }),
                trailingIcon = {
                    if (field.isBlank()) {
                        TextButton(colors = quiet(), onClick = {
                            scope.launch {
                                val pasted = clipboard.getClipEntry()?.clipData?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(context)?.toString()
                                if (!pasted.isNullOrBlank()) field = pasted.trim()
                            }
                        }) { Text(Strings.roomPlacePaste(context)) }
                    } else {
                        TextButton(colors = quiet(), enabled = shape == LinkShape.ROOM, onClick = ::join) { Text(Strings.actionJoin(context)) }
                    }
                },
                supportingText = when {
                    shape == LinkShape.OLD -> ({ Text(Strings.roomErrorOldInvite(context)) })
                    failedHere -> ({ FailureLine(Strings.roomErrorUnreachableLead(context), Strings.roomErrorAskNewInvite(context)) })
                    else -> null
                },
            )
            Row(Modifier.padding(top = 28.dp, bottom = 4.dp), verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Heading(Strings.roomRecent(context))
                Haze(Strings.roomRecentNote(context), maxLines = 1)
            }
            if (recent.isEmpty()) {
                Haze(stringResource(R.string.together_recent_empty), Modifier.padding(top = 4.dp))
            } else {
                for (r in recent) {
                    RecentRow(
                        r,
                        line = c.recentLine(r),
                        failed = rejoinFailed == r.roomId,
                        context = context,
                        onRejoin = { room.rejoin(r.roomId) },
                        onForget = { room.forget(r.roomId) },
                    )
                }
            }
        }
    }
    if (leaving) {
        AlertDialog(
            onDismissRequest = { leaving = false },
            properties = AskingDialog,
            title = { Text(c.LEAVE_TITLE) },
            text = { Text(c.LEAVE_BODY) },
            dismissButton = { TextButton(onClick = { leaving = false }) { Text(c.CANCEL) } },
            confirmButton = {
                TextButton(onClick = {
                    leaving = false
                    room.send(RoomCommand.Leave)
                }) { Text(c.LEAVE) }
            },
        )
    }
}

/** The room you are in, first on the place: its still, title, the room's line, Open the player and Leave…. */
@Composable
private fun YourRoom(view: RoomView, c: RoomCopy, onLeave: () -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val members by store.room.members.collectAsStateWithLifecycle()
    val episode = view.roomEpisode
    val entry = episode?.id?.let { store.titles[it] }
    val line = RoomWords.line(c, view, members, System.currentTimeMillis(), view.position ?: 0.0)
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .background(ObiSilk.Raised)
            .padding(16.dp),
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(14.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(112.dp, 63.dp).clip(RoundedCornerShape(8.dp)).background(ObiSilk.RaisedSelected)) {
                if (entry != null) BannerArt(entry, Modifier.fillMaxSize())
            }
            Column(Modifier.weight(1f)) {
                Text(Strings.roomPlaceYours(context), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text(entry?.showTitle ?: episode?.title ?: view.name, style = ShowTitle.row, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Haze(line.title + " · " + line.detail, maxLines = 2) // i18n-skip: a separator
            }
        }
        Row(Modifier.padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = { store.room.send(RoomCommand.OpenPlayer) }, modifier = Modifier.height(40.dp)) { Text(Strings.roomOpenPlayer(context)) }
            TextButton(colors = quiet(), onClick = onLeave, modifier = Modifier.height(40.dp)) { Text(Strings.roomLeaveEllipsis(context)) }
        }
    }
}

/**
 * A recent room: its cover, title and line, and Rejoin. When nobody answered the last Rejoin, the
 * line says so under it and Rejoin reads Try again. A long press offers Remove from recent rooms.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun RecentRow(room: RoomRecent, line: String, failed: Boolean, context: Context, onRejoin: () -> Unit, onForget: () -> Unit) {
    val store = LocalAppStore.current
    val entry = store.titles[room.id]
    var menu by remember { mutableStateOf(false) }
    Box {
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = 76.dp)
                .combinedClickable(onClick = onRejoin, onLongClick = { menu = true })
                .padding(vertical = 10.dp),
            horizontalArrangement = Arrangement.spacedBy(14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(Modifier.size(36.dp, 54.dp).clip(RoundedCornerShape(4.dp)).background(ObiSilk.Raised)) {
                when {
                    entry != null -> CoverArt(entry, 36.dp, Modifier.fillMaxSize())
                    room.coverUrl != null -> ArtworkImage(room.coverUrl, plate = null, maxPixelSize = pixels(54.dp), modifier = Modifier.fillMaxSize())
                }
            }
            Column(Modifier.weight(1f)) {
                Text(entry?.showTitle ?: room.title, style = ShowTitle.row, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Haze(line, maxLines = 1)
                if (failed) FailureLine(Strings.roomErrorNobodyAnsweredLead(context), Strings.roomErrorNobodyAnsweredLine(context), Modifier.padding(top = 4.dp))
            }
            TextButton(colors = quiet(), onClick = onRejoin) { Text(if (failed) Strings.actionTryAgain(context) else Strings.roomRejoin(context)) }
        }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(text = { Text(Strings.roomRejoin(context)) }, onClick = { menu = false; onRejoin() })
            DropdownMenuItem(text = { Text(stringResource(R.string.together_forget_room)) }, onClick = { menu = false; onForget() })
        }
    }
}

/** A failure where it happened: a vermilion dot, the bold lead, then the hint. */
@Composable
private fun FailureLine(lead: String, hint: String, modifier: Modifier = Modifier) {
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.Top) {
        Box(Modifier.padding(top = 6.dp).size(7.dp).background(ObiSilk.Vermilion, CircleShape))
        Text(
            buildAnnotatedString {
                withStyle(SpanStyle(fontWeight = FontWeight.SemiBold, color = MaterialTheme.colorScheme.onSurface)) { append(lead) }
                append(" ") // i18n-skip
                append(hint)
            },
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun Haze(text: String, modifier: Modifier = Modifier, maxLines: Int = Int.MAX_VALUE) {
    Text(text, modifier = modifier, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = maxLines, overflow = TextOverflow.Ellipsis)
}

@Composable
private fun Heading(text: String, modifier: Modifier = Modifier) {
    Text(text, modifier = modifier.semantics { heading() }, style = MaterialTheme.typography.titleMedium)
}

@Composable
private fun quiet() = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface)
