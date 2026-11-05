// Watch together on the phone: the place, pushed from Home's people action, with Start a party,
// Join a party (a link or a code), the party you are in, and the last ten rooms; and the in-party
// bar above the navigation bar. The lobby is in PartyLobby.kt, the sheets in PartySheets.kt.

package app.kaeri.ui

import android.content.ClipData
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
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
import androidx.compose.ui.platform.ClipEntry
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import app.kaeri.design.ObiSilk
import app.kaeri.design.ShowTitle
import app.kaeri.engine.ffi.partyShareLink
import app.kaeri.engine.model.PartyMember
import app.kaeri.engine.model.PartyRecentRoom
import app.kaeri.engine.model.PartyShare
import app.kaeri.engine.model.PartyTarget
import app.kaeri.engine.model.PartyView
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.Glimpse
import app.kaeri.store.PartyModel
import app.kaeri.store.RoomKey
import app.kaeri.store.Wording
import app.kaeri.store.text

/** The place's route, in Continue's back stack: the navigation bar stays under it. */
const val TOGETHER_ROUTE = "together"

/** The lobby's route: a page over everything, with no navigation bar, so I'm ready sits low. */
const val LOBBY_ROUTE = "together/lobby"

/** The code the copy uses as its example: a word and eight letters and numbers. */
internal const val CODE_EXAMPLE = "kite-7qm4-xr9d" // i18n-skip

/** Home's people action, before the account button: opens Watch together. */
@Composable
internal fun PeopleAction() {
    val navigator = LocalNavigator.current
    val label = stringResource(R.string.together_people_button)
    IconButton(onClick = { navigator.open(TOGETHER_ROUTE) }, modifier = Modifier.semantics { contentDescription = label }) {
        Icon(Icons.People, contentDescription = null)
    }
}

/** The place: what watch together is, the party you are in, Start, Join, and the rooms kept. */
@Composable
internal fun TogetherScreen() {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val context = LocalContext.current
    val party = store.party
    val view by party.view.collectAsStateWithLifecycle()
    val members by party.members.collectAsStateWithLifecycle()
    val recent by party.recent.collectAsStateWithLifecycle()
    val glimpses by party.glimpses.collectAsStateWithLifecycle()
    var field by rememberSaveable { mutableStateOf("") }
    var problem by remember { mutableStateOf<String?>(null) }
    var picking by remember { mutableStateOf(false) }
    val clipboard = LocalClipboard.current
    val snackbars = LocalSnackbars.current
    val scope = rememberCoroutineScope()
    LaunchedEffect(Unit) { store.loadPartyRooms() }
    fun join() {
        val target = store.openParty(field)
        if (target == null) {
            problem = Strings.partyCodeShape(context, CODE_EXAMPLE)
            return
        }
        problem = null
        navigator.open(LOBBY_ROUTE)
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
            PartyHaze(stringResource(R.string.together_line), Modifier.padding(bottom = 20.dp))
            val current = view
            if (current != null) {
                InPartyBlock(current, members, onLeave = { store.leaveParty() })
                Spacer(Modifier.height(24.dp))
            }
            val startLabel = stringResource(R.string.together_start)
            if (current == null) {
                Button(onClick = { picking = true }, modifier = Modifier.height(40.dp)) {
                    Icon(Icons.People, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text(startLabel)
                }
            } else {
                FilledTonalButton(onClick = { picking = true }, modifier = Modifier.height(40.dp)) {
                    Icon(Icons.People, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text(startLabel)
                }
            }
            PartyHaze(stringResource(R.string.together_start_line), Modifier.padding(top = 8.dp, bottom = 24.dp))
            PartyHeading(stringResource(R.string.together_join), Modifier.padding(bottom = 8.dp))
            OutlinedTextField(
                value = field,
                onValueChange = {
                    field = it
                    problem = null
                },
                modifier = Modifier.fillMaxWidth(),
                placeholder = { Text(stringResource(R.string.together_join_field)) },
                singleLine = true,
                isError = problem != null,
                textStyle = MaterialTheme.typography.bodyLarge.copy(fontFamily = FontFamily.Monospace),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Go),
                keyboardActions = KeyboardActions(onGo = { if (field.isNotBlank()) join() }),
                trailingIcon = {
                    if (field.isBlank()) {
                        TextButton(colors = quietText(), onClick = {
                            scope.launch {
                                val pasted = clipboard.getClipEntry()?.clipData?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(context)?.toString()
                                if (!pasted.isNullOrBlank()) {
                                    field = pasted.trim()
                                    join()
                                }
                            }
                        }) { Text(stringResource(R.string.together_paste)) }
                    } else {
                        TextButton(colors = quietText(), onClick = ::join) { Text(stringResource(R.string.together_join_action)) }
                    }
                },
                supportingText = { Text(problem ?: stringResource(R.string.together_join_line)) },
            )
            if (problem != null) {
                val haptics = androidx.compose.ui.platform.LocalHapticFeedback.current
                LaunchedEffect(problem) { haptics.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.Reject) }
            }
            PartyHeading(stringResource(R.string.together_recent), Modifier.padding(top = 28.dp, bottom = 4.dp))
            val rooms = recent
            when {
                rooms == null -> Unit
                rooms.isEmpty() -> PartyHaze(stringResource(R.string.together_recent_empty), Modifier.padding(top = 4.dp))
                else -> for (room in rooms) {
                    val key = RoomKey(room.server, room.room)
                    RecentRoomRow(
                        room,
                        glimpse = glimpses[key],
                        onLook = { store.lookIntoRoom(room.server, room.room) },
                        onRejoin = {
                            store.openParty(PartyTarget(room.server, room.room, null, room.title, room.episode, room.id, false))
                            store.joinParty(room.server, room.room)
                            navigator.open(LOBBY_ROUTE)
                        },
                        onCopy = {
                            val link = partyShareLink(PartyShare(room.server, room.room, null, room.title, room.episode))
                            scope.launch { clipboard.setClipEntry(ClipEntry(ClipData.newPlainText("", link))) }
                            snackbars.show(Strings.partyInviteCopied(context))
                        },
                        onForget = { store.forgetPartyRoom(room.server, room.room) },
                    )
                }
            }
        }
    }
    if (picking) PartyPickSheet(onDismiss = { picking = false }, onStarted = {
        picking = false
        navigator.open(LOBBY_ROUTE)
    })
}

/** The party you are in, first on the place: its still, title, counts, Open lobby or player, Leave. */
@Composable
private fun InPartyBlock(view: PartyView, members: List<PartyMember>, onLeave: () -> Unit) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val context = LocalContext.current
    val episode = view.roomEpisode
    val entry = episode?.id?.let { store.titles[it] }
    val (ready, count) = PartyModel.counts(members)
    PartyHeading(stringResource(R.string.together_in_party), Modifier.padding(bottom = 8.dp))
    Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(96.dp, 54.dp).clip(RoundedCornerShape(8.dp)).background(ObiSilk.Raised)) {
            if (entry != null) BannerArt(entry, Modifier.fillMaxSize())
        }
        Column(Modifier.weight(1f)) {
            Text(entry?.showTitle ?: episode?.title.orEmpty(), style = ShowTitle.row, maxLines = 1, overflow = TextOverflow.Ellipsis)
            val line = if (episode != null && (view.playing || view.episode != null || !members.isEmpty())) {
                Strings.togetherInPartyLine(context, episode.episode, count, ready)
            } else {
                Strings.partyBarIdle(context)
            }
            PartyHaze(line, maxLines = 2)
        }
    }
    Row(Modifier.padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Button(onClick = {
            if (view.playing) store.openPartyPlayer() else navigator.open(LOBBY_ROUTE)
        }, modifier = Modifier.height(40.dp)) {
            Text(stringResource(if (view.playing) R.string.together_open_player else R.string.together_open_lobby))
        }
        TextButton(colors = quietText(), onClick = onLeave, modifier = Modifier.height(40.dp)) { Text(stringResource(R.string.party_leave)) }
    }
}

/**
 * A kept room: its poster, title and line, and Rejoin. A tap looks in, once, and its line becomes
 * who is there; a long press offers Rejoin, Copy invite link and Remove from recent rooms.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun RecentRoomRow(
    room: PartyRecentRoom,
    glimpse: Glimpse?,
    onLook: () -> Unit,
    onRejoin: () -> Unit,
    onCopy: () -> Unit,
    onForget: () -> Unit,
) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val entry = store.titles[room.id]
    var menu by remember { mutableStateOf(false) }
    Box {
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = 76.dp)
                .combinedClickable(onClick = onLook, onLongClick = { menu = true })
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
                // The presence line takes the row's line in one crossfade, after 600 ms of looking.
                val presence = presenceLine(glimpse)
                Crossfade(presence, animationSpec = app.kaeri.design.Motion.fadeIn(), label = "presence") { said -> // i18n-skip
                    PartyHaze(said ?: PartyModel.recentLine(context, room), maxLines = 1)
                }
            }
            TextButton(colors = quietText(), onClick = onRejoin) { Text(stringResource(R.string.together_rejoin)) }
        }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(text = { Text(stringResource(R.string.together_rejoin)) }, onClick = { menu = false; onRejoin() })
            DropdownMenuItem(text = { Text(stringResource(R.string.party_copy_invite)) }, onClick = { menu = false; onCopy() })
            DropdownMenuItem(text = { Text(stringResource(R.string.together_forget_room)) }, onClick = { menu = false; onForget() })
        }
    }
}

/** "Mika is in the room" or "Nobody here now" once a look answered; "Looking in" after 600 ms. */
@Composable
private fun presenceLine(glimpse: Glimpse?): String? {
    val context = LocalContext.current
    var late by remember(glimpse) { mutableStateOf(false) }
    LaunchedEffect(glimpse) {
        if (glimpse == Glimpse.Looking) {
            delay(600)
            late = true
        }
    }
    return when (glimpse) {
        null -> null
        Glimpse.Looking -> if (late) Strings.partyLookingIn(context) else null
        is Glimpse.Found -> {
            val names = glimpse.glimpse.members.map { it.name }
            if (names.isEmpty()) Strings.togetherNobodyHere(context) else Strings.togetherSomeoneHere(context, Wording.list(names), names.size)
        }
        is Glimpse.Failed -> glimpse.reason.text(context)
    }
}

/**
 * The in-party bar, 64 tall in Raised above the navigation bar: the still, "Natsukaze Relay ·
 * Episode 9", the counts, and Play, which opens the player. Anywhere else opens the lobby. It rises
 * from behind the navigation bar as it comes.
 */
@Composable
internal fun PartyBar(hidden: Boolean) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val context = LocalContext.current
    val view by store.party.view.collectAsStateWithLifecycle()
    val members by store.party.members.collectAsStateWithLifecycle()
    val shown = view
    AnimatedVisibility(
        shown != null && !hidden,
        enter = expandVertically(app.kaeri.design.Motion.standard(), expandFrom = Alignment.Top) +
            slideInVertically(app.kaeri.design.Motion.standard()) { it } + fadeIn(app.kaeri.design.Motion.fadeIn()),
        exit = shrinkVertically(app.kaeri.design.Motion.standard(), shrinkTowards = Alignment.Top) +
            slideOutVertically(app.kaeri.design.Motion.standard()) { it } + fadeOut(app.kaeri.design.Motion.fadeOut()),
    ) {
        val party = shown ?: return@AnimatedVisibility
        val episode = party.roomEpisode
        val entry = episode?.id?.let { store.titles[it] }
        val (ready, count) = PartyModel.counts(members)
        val title = entry?.showTitle ?: episode?.title
        val heading = if (title != null && episode != null) Strings.partyBarTitle(context, title, episode.episode) else Strings.togetherInParty(context)
        val counted = if (party.playing && party.episode == null) Strings.partyBarIdle(context) else Strings.partyBarLine(context, count, ready)
        // A new line in the room takes the count's place for 4 s, and gives it back.
        val conversation by store.party.conversation.collectAsStateWithLifecycle()
        val chatHidden by store.party.hideChat.collectAsStateWithLifecycle()
        var newest by remember { mutableStateOf<androidx.compose.ui.text.AnnotatedString?>(null) }
        var barSeen by remember { mutableStateOf(conversation.lines.lastOrNull()?.id ?: 0uL) }
        LaunchedEffect(conversation.lines.lastOrNull()?.id, chatHidden) {
            val last = conversation.lines.lastOrNull() ?: return@LaunchedEffect
            val said = (last.body as? app.kaeri.engine.model.PartyChatBody.Said)?.text
            if (last.id <= barSeen) return@LaunchedEffect
            barSeen = last.id
            if (said == null || last.muted || chatHidden) return@LaunchedEffect
            newest = saidText(chatName(context, last), said)
            kotlinx.coroutines.delay(4000)
            newest = null
        }
        val line = newest ?: androidx.compose.ui.text.AnnotatedString(counted)
        Row(
            Modifier
                .fillMaxWidth()
                .height(64.dp)
                .background(MaterialTheme.colorScheme.surfaceContainer)
                .clickable(onClickLabel = stringResource(R.string.together_open_lobby)) { navigator.open(LOBBY_ROUTE) }
                .semantics(mergeDescendants = true) {}
                .padding(start = 16.dp, end = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(Modifier.size(40.dp).clip(RoundedCornerShape(8.dp)).background(ObiSilk.RaisedSelected)) {
                if (entry != null) BannerArt(entry, Modifier.fillMaxSize())
            }
            Column(Modifier.weight(1f)) {
                Text(heading, style = ShowTitle.caption.copy(fontSize = 15.sp, lineHeight = 20.sp), maxLines = 1, overflow = TextOverflow.Ellipsis)
                Crossfade(line, animationSpec = app.kaeri.design.Motion.quick(), label = "bar line") { said -> // i18n-skip
                    Text(said, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
                }
            }
            IconButton(onClick = { if (party.playing) store.openPartyPlayer() else navigator.open(LOBBY_ROUTE) }) {
                Icon(Icons.PlayRow, contentDescription = stringResource(R.string.together_open_player))
            }
        }
    }
}

/** Supporting text in haze. */
@Composable
internal fun PartyHaze(text: String, modifier: Modifier = Modifier, maxLines: Int = Int.MAX_VALUE) {
    Text(
        text,
        modifier = modifier,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        maxLines = maxLines,
        overflow = TextOverflow.Ellipsis,
    )
}

/** A section's heading on the place and in the lobby: 16 at weight 500. */
@Composable
internal fun PartyHeading(text: String, modifier: Modifier = Modifier) {
    Text(text, modifier = modifier.semantics { heading() }, style = MaterialTheme.typography.titleMedium)
}

/** A member's initial on a Raised circle: no colour, no picture. */
@Composable
internal fun Initial(name: String, size: androidx.compose.ui.unit.Dp) {
    Box(Modifier.size(size).background(ObiSilk.RaisedSelected, CircleShape), contentAlignment = Alignment.Center) {
        Text(name.take(1).uppercase(), style = if (size >= 40.dp) MaterialTheme.typography.bodyLarge else MaterialTheme.typography.labelLarge)
    }
}

/** The button height and shape every full-width lobby action has. */
internal val LobbyButtonHeight = 56.dp
internal val LobbyButtonPadding = ButtonDefaults.ContentPadding
