// The lobby on the phone: what the room watches, whether your sources have it, who is in the
// room and who is ready, the code to share, and I'm ready, low where the thumb is. A link or a
// code opens it before this device is in the room, as Join watch party.

package app.kaeri.ui

import android.content.Intent
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.design.ShowTitle
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.PartyConnection
import app.kaeri.engine.model.PartyFound
import app.kaeri.engine.model.PartyGuess
import app.kaeri.engine.model.PartyMember
import app.kaeri.engine.model.PartyRoomEpisode
import app.kaeri.engine.model.PartySources
import app.kaeri.engine.model.PartyView
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.Glimpse
import app.kaeri.store.PartyModel
import app.kaeri.store.Wording
import app.kaeri.store.text

/**
 * What the lobby shows, worked out from the room this device is in, or the one a link opened and
 * what a look into it found.
 */
private data class LobbyFacts(
    val label: String,
    val title: String?,
    val entry: Entry?,
    val episode: String?,
    /** A line under the title, in haze: how the room's file was matched, a code's empty room. */
    val note: String?,
    val members: List<PartyMember>,
    val inRoom: Boolean,
)

@Composable
internal fun PartyLobbyScreen() {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val context = LocalContext.current
    val party = store.party
    val view by party.view.collectAsStateWithLifecycle()
    val members by party.members.collectAsStateWithLifecycle()
    val lobby by party.target.collectAsStateWithLifecycle()
    val glimpses by party.glimpses.collectAsStateWithLifecycle()
    val noSource by party.noSource.collectAsStateWithLifecycle()
    val lastOut by party.lastOut.collectAsStateWithLifecycle()
    val failure by party.failure.collectAsStateWithLifecycle()
    val joining by party.joining.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    var picking by remember { mutableStateOf(false) }
    val room = view
    val target = lobby?.target
    val glimpse = target?.let { glimpses[app.kaeri.store.RoomKey(it.server, it.room)] }
    // Nothing to show: the party ended and no link is open. Back to where the lobby came from.
    LaunchedEffect(room == null, target == null) {
        if (room == null && target == null) navigator.back()
    }
    if (room == null && target == null) return
    val facts = if (room != null) {
        val episode = room.roomEpisode
        val entry = episode?.id?.let { titles[it] }
        LobbyFacts(
            label = Strings.partyRoomWatching(context),
            title = entry?.showTitle ?: episode?.title,
            entry = entry,
            episode = episode?.let { Strings.partyFormEpisode(context, it.episode.toDoubleOrNull()?.toInt() ?: 0) },
            note = episode?.let { matchNote(context, it) },
            members = members,
            inRoom = true,
        )
    } else {
        val seen = (glimpse as? Glimpse.Found)?.glimpse
        val episode = seen?.roomEpisode
        val id = episode?.id ?: target?.id
        val entry = id?.let { titles[it] }
        val code = target?.takeIf { it.fromCode }?.room
        LobbyFacts(
            label = target?.by?.let { Strings.partyInvitedBy(context, it) } ?: Strings.partyRoomWatching(context),
            title = entry?.showTitle ?: episode?.title ?: target?.title,
            entry = entry,
            episode = (episode?.episode ?: target?.episode)?.let { Strings.partyFormEpisode(context, it.toDoubleOrNull()?.toInt() ?: 0) },
            note = when {
                seen != null && seen.members.isEmpty() && code != null -> Strings.partyCodeEmptyRoom(context, code)
                seen != null && seen.members.isNotEmpty() && seen.roomEpisode == null -> Strings.partyRoomNothing(context)
                else -> null
            },
            members = seen?.members.orEmpty(),
            inRoom = false,
        )
    }
    Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .statusBarsPadding()
                .padding(top = 8.dp, bottom = 180.dp)
                .readingWidth(),
        ) {
            LobbyBanner(facts.entry, room, onBack = {
                if (room == null) store.closePartyLobby()
                navigator.back()
            }, onPick = { picking = true })
            Column(Modifier.padding(horizontal = 16.dp)) {
                Text(facts.label, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 20.dp))
                when {
                    facts.title != null -> {
                        Text(facts.title, style = ShowTitle.page.copy(fontSize = 24.sp, lineHeight = 32.sp), modifier = Modifier.padding(top = 4.dp), maxLines = 2, overflow = TextOverflow.Ellipsis)
                        facts.episode?.let { PartyHaze(it, Modifier.padding(top = 2.dp)) }
                    }
                    room?.looking == true || glimpse == Glimpse.Looking -> LookingIn()
                    room != null -> {
                        Text(Strings.partyRoomNothing(context), style = ShowTitle.page.copy(fontSize = 24.sp, lineHeight = 32.sp), modifier = Modifier.padding(top = 4.dp))
                        PartyHaze(Strings.partyRoomNothingLine(context), Modifier.padding(top = 2.dp))
                        FilledTonalButton(onClick = { picking = true }, modifier = Modifier.padding(top = 12.dp).height(40.dp)) {
                            Text(stringResource(R.string.party_room_pick))
                        }
                    }
                }
                facts.note?.let { PartyHaze(it, Modifier.padding(top = 8.dp)) }
                if (room != null) {
                    RoomMatch(room, onPick = { picking = true })
                    SourcesLine(room, noSource?.reason?.text(context), lastOut, onPick = { picking = true })
                    ConnectionLine(room.connection)
                } else {
                    when (val seen = glimpse) {
                        is Glimpse.Failed -> Problem(seen.reason.text(context))
                        else -> failure?.let { Problem(it.text(context)) }
                    }
                }
                Members(facts.members, inRoom = facts.inRoom)
                if (room != null) CodeRow(room, facts.title, facts.episode)
                if (room != null) {
                    PartyHeading(Strings.partyChatTitle(context), Modifier.padding(top = 28.dp, bottom = 8.dp))
                    ChatBlock(privacy = true)
                }
            }
        }
        LobbyFoot(
            room = room,
            members = members,
            joining = joining,
            failed = glimpse is Glimpse.Failed,
            emptyCode = facts.note != null && room == null && target?.fromCode == true && (glimpse as? Glimpse.Found)?.glimpse?.members?.isEmpty() == true,
            onJoin = { target?.let { store.joinParty(it.server, it.room) } },
            onRetry = { target?.let { store.lookIntoRoom(it.server, it.room) } },
            modifier = Modifier.align(Alignment.BottomCenter),
        )
    }
    if (picking) PartyPickSheet(onDismiss = { picking = false }, changing = room != null, onStarted = { picking = false })
}

/** How the room's episode was found here, when it was not by its tag. */
private fun matchNote(context: android.content.Context, episode: PartyRoomEpisode): String? = when (episode.found) {
    PartyFound.TAG -> null
    PartyFound.NAME -> Strings.partyRoomFoundByName(context)
    PartyFound.FILE_NAME -> Strings.partyRoomMatched(context)
    PartyFound.UNMATCHED -> episode.plainBy?.let { Strings.partyRoomPlainFile(context, it, episode.file) } ?: episode.file
}

/** The still across the top, with back on its left and the room's menu on its right. */
@Composable
private fun LobbyBanner(entry: Entry?, room: PartyView?, onBack: () -> Unit, onPick: () -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    var menu by remember { mutableStateOf(false) }
    Box(
        Modifier
            .padding(horizontal = 16.dp)
            .fillMaxWidth()
            .aspectRatio(380f / 214f)
            .clip(RoundedCornerShape(16.dp))
            .background(ObiSilk.Raised),
    ) {
        if (entry != null) BannerArt(entry, Modifier.fillMaxSize())
        val plate = IconButtonDefaults.filledTonalIconButtonColors(containerColor = ObiSilk.Ground.copy(alpha = 0.72f))
        androidx.compose.material3.FilledTonalIconButton(onClick = onBack, colors = plate, modifier = Modifier.padding(8.dp).align(Alignment.TopStart)) {
            Icon(Icons.Back, contentDescription = stringResource(R.string.action_back))
        }
        if (room != null) {
            Box(Modifier.align(Alignment.TopEnd)) {
                androidx.compose.material3.FilledTonalIconButton(onClick = { menu = true }, colors = plate, modifier = Modifier.padding(8.dp)) {
                    Icon(Icons.More, contentDescription = stringResource(R.string.action_more))
                }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(text = { Text(stringResource(R.string.party_change_episode)) }, onClick = { menu = false; onPick() })
                    val link = room.invite ?: room.link
                    if (link != null) {
                        val clipboard = androidx.compose.ui.platform.LocalClipboard.current
                        val snackbars = LocalSnackbars.current
                        val scope = androidx.compose.runtime.rememberCoroutineScope()
                        DropdownMenuItem(text = { Text(stringResource(R.string.party_copy_invite)) }, onClick = {
                            menu = false
                            scope.launchCopy(clipboard, link)
                            snackbars.show(Strings.partyInviteCopied(context))
                        })
                    }
                    DropdownMenuItem(text = { Text(stringResource(R.string.party_leave)) }, onClick = { menu = false; store.leaveParty() })
                }
            }
        }
    }
}

/** Put [text] on the clipboard. */
internal fun kotlinx.coroutines.CoroutineScope.launchCopy(clipboard: androidx.compose.ui.platform.Clipboard, text: String) {
    launch { clipboard.setClipEntry(androidx.compose.ui.platform.ClipEntry(android.content.ClipData.newPlainText("", text))) }
}

/** "Looking in" after 600 ms of looking, with why the room shows you for a moment. */
@Composable
private fun LookingIn() {
    var late by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        delay(600)
        late = true
    }
    AnimatedVisibility(late, enter = fadeIn(Motion.fadeIn())) {
        Column(Modifier.padding(top = 4.dp)) {
            Text(stringResource(R.string.party_looking_in), style = ShowTitle.page.copy(fontSize = 24.sp, lineHeight = 32.sp))
            PartyHaze(stringResource(R.string.party_looking_in_note), Modifier.padding(top = 2.dp))
        }
    }
}

/** A plain file's guesses, or Not this when the room's file was matched by its name. */
@Composable
private fun RoomMatch(room: PartyView, onPick: () -> Unit) {
    val store = LocalAppStore.current
    val episode = room.roomEpisode ?: return
    when (episode.found) {
        PartyFound.TAG -> Unit
        PartyFound.NAME, PartyFound.FILE_NAME -> TextButton(colors = quietText(), onClick = { store.choosePartyTitle(null) }, modifier = Modifier.padding(top = 4.dp)) {
            Text(stringResource(R.string.party_room_not_this))
        }
        PartyFound.UNMATCHED -> if (room.guesses.isEmpty()) {
            PartyHaze(stringResource(R.string.party_room_not_found), Modifier.padding(top = 8.dp))
            FilledTonalButton(onClick = onPick, modifier = Modifier.padding(top = 12.dp).height(40.dp)) {
                Text(stringResource(R.string.party_pick_search))
            }
        } else {
            PartyHeading(stringResource(R.string.party_room_guesses), Modifier.padding(top = 20.dp, bottom = 4.dp))
            for (guess in room.guesses) GuessRow(guess) { store.choosePartyTitle(guess.id) }
        }
    }
}

@Composable
private fun GuessRow(guess: PartyGuess, onPick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 64.dp).clickable(onClick = onPick).padding(vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(36.dp, 54.dp).clip(RoundedCornerShape(4.dp)).background(ObiSilk.Raised)) {
            ArtworkImage(guess.coverUrl, plate = null, maxPixelSize = pixels(54.dp), modifier = Modifier.fillMaxSize())
        }
        Text(guess.title, style = ShowTitle.row, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
    }
}

/**
 * Whether this device's sources have the room's episode: a water dot and "Plays from your
 * sources", or why not, with Open sources; the last episode out, with Watch something else.
 */
@Composable
private fun SourcesLine(room: PartyView, missing: String?, lastOut: app.kaeri.store.PartyLastOut?, onPick: () -> Unit) {
    val navigator = LocalNavigator.current
    val context = LocalContext.current
    if (lastOut != null) {
        val airs = lastOut.airs?.text(context)
        Text(
            if (airs != null) Strings.partyLastOutAirs(context, lastOut.episode, airs) else Strings.partyLastOut(context),
            style = MaterialTheme.typography.bodyLarge,
            modifier = Modifier.padding(top = 16.dp),
        )
        FilledTonalButton(onClick = onPick, modifier = Modifier.padding(top = 12.dp).height(40.dp)) {
            Text(stringResource(R.string.party_watch_something_else))
        }
        return
    }
    val sources = room.sources ?: return
    Crossfade(sources, animationSpec = Motion.fadeIn(), label = "sources") { now -> // i18n-skip
        Row(Modifier.padding(top = 16.dp).semantics { liveRegion = LiveRegionMode.Polite }, verticalAlignment = Alignment.CenterVertically) {
            when (now) {
                PartySources.Checking -> PartyHaze(Strings.partyLookingSources(context))
                PartySources.Found -> {
                    Box(Modifier.size(8.dp).background(ObiSilk.Water, CircleShape))
                    Spacer(Modifier.width(8.dp))
                    Text(Strings.partyLobbySourcesFound(context), style = MaterialTheme.typography.bodyMedium)
                }
                is PartySources.Missing -> Column {
                    Text(Strings.partyLobbySourcesMissing(context), style = MaterialTheme.typography.bodyMedium)
                    PartyHaze(missing ?: now.reason.text(context), Modifier.padding(top = 2.dp))
                    if (now.why != app.kaeri.engine.model.NoSourceWhy.NOT_AIRED) {
                        TextButton(colors = quietText(), onClick = { navigator.open(PROVIDERS_ROUTE) }, contentPadding = androidx.compose.foundation.layout.PaddingValues(0.dp)) {
                            Text(Strings.partyOpenSources(context))
                        }
                    }
                }
            }
        }
    }
}

/** "Connection lost. Trying again (2 of 5)" while the party reconnects. */
@Composable
private fun ConnectionLine(connection: PartyConnection) {
    val context = LocalContext.current
    AnimatedVisibility(connection is PartyConnection.Reconnecting, enter = expandVertically(Motion.standard()) + fadeIn(Motion.fadeIn()), exit = shrinkVertically(Motion.standard()) + fadeOut(Motion.fadeOut())) {
        val lost = connection as? PartyConnection.Reconnecting
        if (lost != null) Problem(Strings.partyConnectionLost(context, lost.attempt.toInt(), lost.of.toInt()))
    }
}

/** A problem said in vermilion, with the reject haptic. */
@Composable
private fun Problem(text: String) {
    SheetProblem(text, Modifier.padding(top = 16.dp))
}

/** "In the room · 2 of 3 ready" and a row a member. */
@Composable
private fun Members(members: List<PartyMember>, inRoom: Boolean) {
    val context = LocalContext.current
    if (members.isEmpty()) return
    val store = LocalAppStore.current
    val chat by store.party.chat.collectAsStateWithLifecycle()
    val muted = chat?.muted.orEmpty()
    val (ready, count) = PartyModel.counts(members)
    val heading = if (inRoom) Strings.partyLobbyCount(context, ready, count) else Strings.togetherSomeoneHere(context, Wording.list(members.map { it.name }), count)
    PartyHeading(heading, Modifier.padding(top = 28.dp, bottom = 4.dp))
    for (member in members) {
        MemberRow(member, inRoom, muted = member.name in muted, onMute = if (inRoom && !member.you) { on -> store.partyMute(member.name, on) } else null)
    }
}

@Composable
private fun MemberRow(member: PartyMember, inRoom: Boolean, muted: Boolean = false, onMute: ((Boolean) -> Unit)? = null) {
    val context = LocalContext.current
    var menu by remember { mutableStateOf(false) }
    val muteWords = if (muted) Strings.partyUnmute(context, member.name) else Strings.partyMute(context, member.name)
    Row(
        Modifier.fillMaxWidth().height(56.dp)
            .let { if (onMute != null) it.combinedClickable(onClick = {}, onLongClick = { menu = true }, onLongClickLabel = muteWords) else it }
            .semantics(mergeDescendants = true) {
                if (onMute != null) customActions = listOf(CustomAccessibilityAction(muteWords) { onMute(!muted); true })
            },
        horizontalArrangement = Arrangement.spacedBy(14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Initial(member.name, 40.dp)
        Column(Modifier.weight(1f)) {
            Text(if (member.you) Strings.partyMemberYou(context, member.name) else member.name, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            val line = when {
                !inRoom -> null
                member.file == null -> Strings.partyMemberNoFile(context)
                member.fileMatches == false -> Strings.partyMemberOtherFile(context)
                member.ready == null -> Strings.partyMemberNoReadyState(context)
                member.ready == false -> Strings.partyMemberNotReady(context)
                else -> null
            }
            if (line != null) PartyHaze(line, maxLines = 1)
        }
        if (muted) PartyHaze(Strings.partyMemberMuted(context))
        if (onMute != null) Menu(expanded = menu, onDismissRequest = { menu = false }) {
            androidx.compose.material3.DropdownMenuItem(text = { Text(muteWords) }, onClick = { menu = false; onMute(!muted) })
        }
        // Ready reads in silk, by its mark and word: the amber stays on the main action.
        AnimatedVisibility(inRoom && member.ready == true, enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut())) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Check, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text(Strings.partyReadyOn(context), style = MaterialTheme.typography.bodyMedium)
            }
        }
    }
}

/** The room's code, in mono, and Share invite. */
@Composable
private fun CodeRow(room: PartyView, title: String?, episode: String?) {
    val context = LocalContext.current
    val link = room.invite ?: room.link ?: return
    Row(Modifier.padding(top = 24.dp).fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            PartyHaze(Strings.partyCode(context))
            Text(room.code ?: room.room, style = MaterialTheme.typography.bodyLarge.copy(fontFamily = FontFamily.Monospace, fontSize = 18.sp), maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        FilledTonalButton(onClick = { share(context, room, title, link) }, modifier = Modifier.height(40.dp)) {
            Icon(Icons.Share, contentDescription = null, modifier = Modifier.size(18.dp))
            Spacer(Modifier.width(8.dp))
            Text(Strings.partyShareInvite(context))
        }
    }
}

/** The share sheet with "Watch Natsukaze Relay episode 9 with me" and the link. */
internal fun share(context: android.content.Context, room: PartyView, title: String?, link: String) {
    val episode = room.roomEpisode?.episode
    val text = if (title != null && episode != null) Strings.partyShareText(context, title, episode) + "\n" + link else link // i18n-skip
    val send = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, text) // i18n-skip
    context.startActivity(Intent.createChooser(send, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
}

/**
 * The foot, 24 above the bottom: the main action full width at 56, then the wait line, centred,
 * while the room waits, and Start anyway when this device may.
 */
@Composable
private fun LobbyFoot(
    room: PartyView?,
    members: List<PartyMember>,
    joining: Boolean,
    failed: Boolean,
    emptyCode: Boolean,
    onJoin: () -> Unit,
    onRetry: () -> Unit,
    modifier: Modifier,
) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    Column(
        modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.background)
            .navigationBarsPadding()
            .padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 24.dp)
            .readingWidth(),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        val full = Modifier.fillMaxWidth().height(LobbyButtonHeight)
        when {
            room == null && failed -> FilledTonalButton(onClick = onRetry, modifier = full) { Text(Strings.actionTryAgain(context)) }
            room == null -> Button(onClick = onJoin, enabled = !joining, modifier = full) {
                Text(if (emptyCode) Strings.partyCodeWait(context) else Strings.partyJoin(context))
            }
            room.playing -> Button(onClick = { store.openPartyPlayer() }, modifier = full) { Text(Strings.togetherOpenPlayer(context)) }
            room.startingIn != null -> FilledTonalButton(onClick = {}, modifier = full) {
                AnimatedContent(room.startingIn, transitionSpec = { fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeOut()) }, label = "count") { seconds -> // i18n-skip
                    Text(Strings.partyStarting(context, (seconds ?: 0u).toInt()))
                }
            }
            room.ready -> FilledTonalButton(onClick = { store.partyReady(false) }, modifier = full) {
                Icon(Icons.Check, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text(Strings.partyReadyOn(context))
            }
            else -> Button(onClick = { store.partyReady(true) }, enabled = room.roomEpisode != null, modifier = full) {
                Text(Strings.partyReadyAction(context))
            }
        }
        val line = when {
            room == null || room.playing -> null
            room.startingIn != null -> Strings.partyStartsLine(context)
            else -> PartyModel.waitLine(context, members)
        }
        Crossfade(line, animationSpec = Motion.fadeIn(), label = "wait") { said -> // i18n-skip
            if (said != null) {
                Text(
                    said,
                    style = MaterialTheme.typography.bodySmall.copy(fontSize = 12.sp),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.fillMaxWidth().padding(top = 10.dp).semantics { liveRegion = LiveRegionMode.Polite },
                )
            }
        }
        if (room != null && room.canStartAnyway && room.startingIn == null && !room.playing) {
            TextButton(colors = quietText(), onClick = { store.startPartyAnyway() }) { Text(Strings.partyStartAnyway(context)) }
        }
    }
}

/**
 * The player's People sheet: who is in the room and who play waits for, the invite to copy, and
 * Leave watch party. The room's own words show on the video.
 */
@Composable
fun PartyPeopleContent(onDone: () -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val view by store.party.view.collectAsStateWithLifecycle()
    val members by store.party.members.collectAsStateWithLifecycle()
    val room = view ?: return
    val configuration = androidx.compose.ui.platform.LocalConfiguration.current
    // Held sideways, the sheet is wide: members on the left, the conversation and field on the right.
    val wide = configuration.screenWidthDp > configuration.screenHeightDp
    // The column over the video rests while this is open: what it holds is in the list.
    LaunchedEffect(Unit) { store.party.seenThrough(store.party.conversation.value.lines.lastOrNull()?.id ?: 0uL) }
    val actions: @Composable () -> Unit = {
        Row(Modifier.padding(top = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            val link = room.invite ?: room.link
            if (link != null) {
                FilledTonalButton(onClick = { share(context, room, room.roomEpisode?.title, link) }) {
                    Icon(Icons.Share, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text(Strings.partyShareInvite(context))
                }
            }
            TextButton(colors = quietText(), onClick = {
                store.leaveParty()
                onDone()
            }) { Text(Strings.partyLeave(context)) }
        }
    }
    Column(Modifier.padding(horizontal = 24.dp)) {
        Text(Strings.togetherInParty(context), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk)
        PartyHaze(PartyModel.waitLine(context, members), Modifier.padding(top = 4.dp))
        if (wide) {
            Row(horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                Column(Modifier.weight(1f)) {
                    Members(members, inRoom = true)
                    actions()
                }
                ChatBlock(Modifier.weight(1f).padding(top = 28.dp), maxHeight = 132.dp)
            }
        } else {
            Members(members, inRoom = true)
            ChatBlock(Modifier.padding(top = 20.dp), maxHeight = 200.dp)
            actions()
        }
    }
}

/** A text button in silk: the amber stays on the view's one main action. */
@Composable
internal fun quietText() = androidx.compose.material3.ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface)
