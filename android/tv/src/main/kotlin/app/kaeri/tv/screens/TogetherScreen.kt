// Watch together on the TV: the place in the rail after Library (Start a party, the party this TV
// is in, joining from a phone, and the rooms kept), the lobby with its QR code and code, and what
// a party starts on. Each viewer plays from their own sources; the TV joins from a paired phone
// or a kept room, since nobody types a link on a remote.

package app.kaeri.tv.screens

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.foundation.Canvas as DrawCanvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.engine.ffi.qrMatrix
import app.kaeri.engine.model.PartyMember
import app.kaeri.engine.model.PartySources
import app.kaeri.engine.model.PartyTarget
import app.kaeri.engine.model.PartyView
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.PartyModel
import app.kaeri.store.text
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
import app.kaeri.tv.ui.TvListRow
import app.kaeri.tv.ui.pixels
import app.kaeri.tv.ui.remembered
import app.kaeri.tv.ui.firstFocus

/** The place: Start a party first, the party this TV is in, phones, and the rooms kept. */
@Composable
fun TogetherScreen(nav: TvNavigator, memory: FocusMemory) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val remote = LocalRemoteHost.current
    val view by store.party.view.collectAsStateWithLifecycle()
    val members by store.party.members.collectAsStateWithLifecycle()
    val recent by store.party.recent.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) {
        store.loadPartyRooms()
        store.loadPartyForm()
    }
    Column(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .verticalScroll(rememberScrollState())
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp, bottom = 48.dp),
    ) {
        Text(stringResource(R.string.together_title), style = MaterialTheme.typography.headlineMedium, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        Text(stringResource(R.string.together_line), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, modifier = Modifier.padding(top = 8.dp).widthIn(max = 640.dp))
        val room = view
        Row(Modifier.padding(top = 28.dp), horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
            if (room == null) {
                AmberButton(onClick = nav::openPartyPick, icon = TvIcons.People, modifier = Modifier.firstFocus(memory, "together/start")) {
                    ButtonLabel(stringResource(R.string.together_start))
                }
            } else {
                AmberButton(onClick = nav::openLobby, icon = TvIcons.People, modifier = Modifier.firstFocus(memory, "together/start")) {
                    ButtonLabel(stringResource(R.string.together_open_lobby))
                }
                TonalButton(onClick = { store.leaveParty() }, modifier = Modifier.remembered(memory, "together/leave")) {
                    ButtonLabel(stringResource(R.string.party_leave))
                }
            }
        }
        if (room != null) {
            val episode = room.roomEpisode
            val (ready, count) = PartyModel.counts(members)
            val title = episode?.id?.let { titles[it]?.title } ?: episode?.title
            Text(
                listOfNotNull(title, episode?.let { Strings.togetherInPartyLine(context, it.episode, count, ready) }).joinToString(" · "), // i18n-skip
                style = MaterialTheme.typography.bodyLarge,
                color = ObiSilk.Haze,
                modifier = Modifier.padding(top = 12.dp),
            )
        } else {
            Text(stringResource(R.string.together_start_line), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, modifier = Modifier.padding(top = 12.dp))
        }
        // Joining is from a phone: a link opened there offers Join on this TV.
        Text(stringResource(R.string.together_join), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.padding(top = 36.dp).semantics { heading() })
        Text(stringResource(R.string.party_tv_join_from_phone), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, modifier = Modifier.padding(top = 6.dp).widthIn(max = 640.dp))
        val phones = remote?.phones.orEmpty()
        if (phones.isNotEmpty()) {
            Text(phones.joinToString(", ") { it.name }, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(top = 4.dp)) // i18n-skip
        }
        if (remote != null) {
            TonalButton(onClick = remote::openPairing, modifier = Modifier.padding(top = 12.dp).remembered(memory, "together/pair")) {
                ButtonLabel(stringResource(R.string.party_tv_pair))
            }
        }
        Text(stringResource(R.string.together_recent), style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk, modifier = Modifier.padding(top = 36.dp, bottom = 12.dp).semantics { heading() })
        val rooms = recent
        when {
            rooms == null -> Unit
            rooms.isEmpty() -> Text(stringResource(R.string.together_recent_empty), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            else -> LazyRow(horizontalArrangement = Arrangement.spacedBy(20.dp), contentPadding = PaddingValues(vertical = Canvas.FocusRoom)) {
                items(rooms, key = { "${it.server}/${it.room}" }) { kept -> // i18n-skip
                    val entry = titles[kept.id]
                    val title = entry?.title ?: kept.title
                    val line = PartyModel.recentLine(context, kept)
                    val rejoin = {
                        store.openParty(PartyTarget(kept.server, kept.room, null, kept.title, kept.episode, kept.id, false))
                        store.joinParty(kept.server, kept.room)
                        nav.openLobby()
                    }
                    val focus = Modifier.remembered(memory, "together/room/${kept.server}/${kept.room}")
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
        }
    }
    EnterFocus(memory)
}

/**
 * What the party starts on: the titles this TV is watching, at the episode next. The server is
 * the last one used or the suggested one, and the name the settings' name.
 */
@Composable
fun PartyPickScreen(nav: TvNavigator, memory: FocusMemory) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val form by store.party.form.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) {
        store.loadPartyForm()
        store.loadContinue()
    }
    BackHandler { nav.pop() }
    val rows = remember(titles) { store.continuing.queue(titles).all }
    Column(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp),
    ) {
        Text(stringResource(R.string.party_pick_title), style = MaterialTheme.typography.headlineMedium, color = ObiSilk.Silk, modifier = Modifier.semantics { heading() })
        Spacer(Modifier.height(20.dp))
        LazyColumn(Modifier.widthIn(max = 720.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            items(rows, key = { it.entry.id.toString() }) { item ->
                TvListRow(
                    item.entry.title,
                    onClick = {
                        val shown = form ?: return@TvListRow
                        val server = shown.recentServers.firstOrNull() ?: shown.suggestedServer
                        store.startParty(server, shown.room, null, item.entry.id, item.episode.toString())
                        nav.openLobby()
                    },
                    supporting = Strings.partyPickNext(context, item.episode.toString()),
                    enabled = form != null,
                    modifier = Modifier.firstFocus(memory, "pick/${item.entry.id}", isFirst = item == rows.first()),
                )
            }
        }
    }
    EnterFocus(memory, entry = rows.size)
}

/**
 * The lobby on the TV: the label, title and episode, whether this TV's sources have it, who is
 * in the room, I'm ready and Leave; and on the right the QR code that joins from a phone, with
 * the code to type instead.
 */
@Composable
fun PartyLobbyScreen(nav: TvNavigator, memory: FocusMemory) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val view by store.party.view.collectAsStateWithLifecycle()
    val members by store.party.members.collectAsStateWithLifecycle()
    val joining by store.party.joining.collectAsStateWithLifecycle()
    val failure by store.party.failure.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    BackHandler { nav.pop() }
    val room = view
    // Left, or never in: back to the place, once the join has had its answer.
    LaunchedEffect(room == null, joining, failure) {
        if (room == null && !joining && failure == null) nav.pop()
    }
    val primary = remember { FocusRequester() }
    Row(
        Modifier
            .fillMaxSize()
            .background(ObiSilk.Ground)
            .padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 72.dp),
        horizontalArrangement = Arrangement.spacedBy(64.dp),
    ) {
        Column(Modifier.weight(1f)) {
            val episode = room?.roomEpisode
            val entry = episode?.id?.let { titles[it] }
            Text(stringResource(R.string.party_room_watching), style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze)
            Text(
                entry?.title ?: episode?.title ?: stringResource(if (room?.looking == true || joining) R.string.party_looking_in else R.string.party_room_nothing),
                style = MaterialTheme.typography.headlineMedium,
                color = ObiSilk.Silk,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(top = 6.dp),
            )
            episode?.let {
                Text(Strings.partyFormEpisode(context, it.episode.toDoubleOrNull()?.toInt() ?: 0), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, modifier = Modifier.padding(top = 4.dp))
            }
            if (room != null) TvSources(room)
            failure?.let { Text(it.text(context), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Vermilion, modifier = Modifier.padding(top = 16.dp)) }
            if (members.isNotEmpty() && room != null) {
                val (ready, count) = PartyModel.counts(members)
                Text(Strings.partyLobbyCount(context, ready, count), style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk, modifier = Modifier.padding(top = 28.dp, bottom = 8.dp))
                for (member in members.take(6)) TvMemberRow(member)
            }
            Row(Modifier.padding(top = 24.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                when {
                    room == null -> Unit
                    room.playing -> AmberButton(onClick = { store.openPartyPlayer() }, modifier = Modifier.focusRequester(primary).remembered(memory, "lobby/main")) {
                        ButtonLabel(stringResource(R.string.together_open_player))
                    }
                    room.startingIn != null -> TonalButton(onClick = {}, modifier = Modifier.focusRequester(primary).remembered(memory, "lobby/main")) {
                        ButtonLabel(Strings.partyStarting(context, room.startingIn!!.toInt()))
                    }
                    room.ready -> TonalButton(onClick = { store.partyReady(false) }, icon = TvIcons.Check, modifier = Modifier.focusRequester(primary).remembered(memory, "lobby/main")) {
                        ButtonLabel(stringResource(R.string.party_ready_on))
                    }
                    else -> AmberButton(onClick = { store.partyReady(true) }, icon = null, enabled = room.roomEpisode != null, modifier = Modifier.focusRequester(primary).remembered(memory, "lobby/main")) {
                        ButtonLabel(stringResource(R.string.party_ready_action))
                    }
                }
                if (room != null && room.canStartAnyway && room.startingIn == null && !room.playing) {
                    TonalButton(onClick = { store.startPartyAnyway() }, modifier = Modifier.remembered(memory, "lobby/anyway")) {
                        ButtonLabel(stringResource(R.string.party_start_anyway))
                    }
                }
                TonalButton(onClick = { store.leaveParty() }, modifier = Modifier.remembered(memory, "lobby/leave")) {
                    ButtonLabel(stringResource(R.string.party_leave))
                }
            }
            if (room != null) {
                val line = if (room.startingIn != null) Strings.partyStartsLine(context) else PartyModel.waitLine(context, members)
                Crossfade(line, animationSpec = Motion.fadeIn(), label = "wait") { said -> // i18n-skip
                    Text(said, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(top = 12.dp))
                }
            }
        }
        val link = room?.invite ?: room?.link
        AnimatedVisibility(link != null) {
            Column(Modifier.width(240.dp), horizontalAlignment = Alignment.Start) {
                if (link != null) PartyQr(link)
                Text(stringResource(R.string.party_tv_scan), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.padding(top = 16.dp))
                Text(stringResource(R.string.party_tv_or_code), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(top = 12.dp))
                Text(room?.code ?: room?.room.orEmpty(), style = MaterialTheme.typography.bodyLarge.copy(fontFamily = FontFamily.Monospace, fontSize = 26.sp), color = ObiSilk.Silk, modifier = Modifier.padding(top = 4.dp))
            }
        }
    }
    LaunchedEffect(room != null) { if (room != null) runCatching { primary.requestFocus() } }
    EnterFocus(memory, entry = room != null)
}

/** "Plays from your sources" with a water dot, or why not. */
@Composable
private fun TvSources(room: PartyView) {
    val context = LocalContext.current
    val sources = room.sources ?: return
    Row(Modifier.padding(top = 16.dp), verticalAlignment = Alignment.CenterVertically) {
        when (sources) {
            PartySources.Checking -> Text(Strings.partyLookingSources(context), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
            PartySources.Found -> {
                Box(Modifier.size(10.dp).background(ObiSilk.Water, CircleShape))
                Spacer(Modifier.width(10.dp))
                Text(Strings.partyLobbySourcesFound(context), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
            }
            is PartySources.Missing -> Text(sources.reason.text(context), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
        }
    }
}

@Composable
private fun TvMemberRow(member: PartyMember) {
    val context = LocalContext.current
    Row(Modifier.height(48.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
        Box(Modifier.size(36.dp).background(ObiSilk.RaisedSelected, CircleShape), contentAlignment = Alignment.Center) {
            Text(member.name.take(1).uppercase(), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk)
        }
        Text(if (member.you) Strings.partyMemberYou(context, member.name) else member.name, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.widthIn(min = 200.dp))
        Text(
            if (member.ready == true) Strings.partyReadyOn(context) else if (member.file == null) Strings.partyMemberNoFile(context) else Strings.partyMemberNotReady(context),
            style = MaterialTheme.typography.bodyMedium,
            color = if (member.ready == true) ObiSilk.Silk else ObiSilk.Haze,
        )
    }
}

/** The invite as a 200 QR code, dark modules on a silk plate so any phone camera reads it. */
@Composable
private fun PartyQr(link: String) {
    val matrix = remember(link) { qrMatrix(link) }
    val label = stringResource(R.string.party_tv_scan)
    Box(Modifier.size(200.dp).clip(RoundedCornerShape(12.dp)).background(ObiSilk.Silk).padding(10.dp).semantics { contentDescription = label }) {
        DrawCanvas(Modifier.fillMaxSize()) {
            val side = matrix.size.toInt()
            val zone = matrix.quietZone.toInt()
            val cell = size.width / (side + 2 * zone)
            for (y in 0 until side) {
                for (x in 0 until side) {
                    if (matrix.modules[y * side + x]) drawRect(ObiSilk.Ground, Offset((x + zone) * cell, (y + zone) * cell), Size(cell + 0.5f, cell + 0.5f))
                }
            }
        }
    }
}
