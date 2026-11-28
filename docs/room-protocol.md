# The room protocol

Watching together happens in a room: the video, the people and the chat in one place. A room has
no host and no server of Kaeri's. Each device plays the episode from its own sources, and the room
carries only what is playing (an AniList id and an episode), who is in it, where they are and
what they say. Events travel end to end encrypted over public Nostr relays run by others. The
crate is `kaeri-room`; the engine's side is `crates/kaeri-engine/src/room.rs`, built with the
`room` feature; the front end's surface is `RoomCommand` and the `Room*` events in
`crates/kaeri-model/src/room.rs`.

This replaces watch parties on Syncplay, which are gone with their servers, codes and lobby. Old
links and codes still parse, and say "This invite is from an older version of Kaeri. Ask for a
new one."

## The shape

- An invite holds a 128-bit room secret in the URL fragment. Every key comes from it.
- Each device joins with a fresh signing key for that session only, and signs every event with
  it. Every body is sealed with the room key and repeats the signer's key. A relay, or anyone
  without the link, can't read or forge events.
- A per-room identity key, seen only inside sealed bodies, ties each member id to its session
  key, so one member can't speak as another.
- Every event is one ephemeral kind (relays pass it on and keep nothing), tagged with a room tag
  that changes each UTC day, and sent to the room's three relays. Receivers drop duplicates by
  event id.
- The room's playhead (episode, position, playing or paused) is a register ordered by a Lamport
  counter, then the sender's clock, then its key. Every device lands on the same playhead
  whatever order events arrive in.
- Each device sends what changes, as it changes, and a heartbeat every 15 s. Every event carries
  the sender's status. A newcomer asks for the room and one member answers.
- Waits aren't messages. Every device works them out from the same statuses: the room plays when
  nobody it waits for is still loading, or when someone carries on.

## Invites and keys

```
https://kaeri.app/room#v=3&k=<22 chars base64url>&by=Mika&n=<title>&e=9[&rl=<relays>]
```

Browsers never send the fragment; the page reads it and opens `kaeri://room#…`, loading no
analytics and sending nothing. The watch party's old page, `kaeri.app/party`, forwards to it with
the fragment, and the apps read any `/party` or `kaeri://party` link as from an older version. `by`, `n` and `e` are only for the page's "Mika invites you" line. `rl` lists the
room's relays when they aren't the built-in set. Where the link is shared, one line says
"Anyone with this link can join."

```
secret S          128 random bits (the invite's k)
room_id           HKDF-SHA256(S, salt "kaeri-room-v1", info "id")[..16]   local only, never sent
K_msg             HKDF(S, salt "kaeri-room-v1", info "msg")              seals every body
K_tag             HKDF(S, salt "kaeri-room-v1", info "tag")
room tag (day d)  hex(HMAC-SHA256(K_tag, "tag" ‖ d)[..16])               d = days since epoch, UTC, in decimal
```

Bodies use NIP-44 v2's construction with `K_msg` as the conversation key: ChaCha20, HMAC-SHA256,
a fresh 32-byte nonce per message, NIP-44's padding. From 23:58 to 00:02 UTC devices listen on
both days' tags and send on the new day's from midnight.

A receiver takes an event only when its signature is valid, the body opens under `K_msg`,
`body.from` is the event's key, that key is bound to `body.mid` (or the body is the Hello that
binds it), `ctr` wasn't seen from that key yet and is less than 64 below the highest that was
(events come by several relays, each in its own time), and `created_at` is within 120 s of the
receiver's corrected clock.

**New invite** (anyone can press it) rotates the secret. The new one goes to every member present,
sealed to each session key, inside one event on the old tag, and everyone moves. It stops links
nobody has used; it can't shut out someone already in the room. That is what Move to a new room
is for (below). There is no kick.

## Events

One ephemeral kind, `23517`, for every message. Tags: `["z", <room tag>]` and
`["expiration", created_at + 60]`. The body is JSON with short keys, padded to a bucket before
sealing (512 B for nearly everything, 1–2 KiB for long chat, 4 KiB for sync answers, rekeys and
moves), so a chat line and a heartbeat look alike.

```
{ v: 1, from, mid, ctr, t,            // t: sender's corrected clock in ms, for ties only
  st: Status,                         // in every message
  m: Message }

Status   = { s, ep: [anilist_id, "9"], pos, play, age, rdy, pe, ue, rejoin, dw, bg, dg, ok, rtt, hb }
Message  = Heartbeat | Hello | Status | Playhead | SyncReq | SyncAns | Chat | React
         | Relays | Rekey | Move | Leave
```

`s` is what the device's own player is doing: Watching, Loading, Buffering, Behind, Choosing,
NoSource or PlayerClosed. `age` is how long it has been so, so receivers count seconds locally;
`rdy` is the wait this device is ready for; `dw` is Don't wait for me; `bg` the app in the
background; `dg` a digest (playhead seq, relays seq, key epoch) that tells a device it missed a
write. The wire has no field for a source, a link, a provider or a plugin, and a test checks the
schema for it. `crates/kaeri-room/src/wire.rs` is the exact form.

## Rates

| What | Rate |
|---|---|
| Heartbeat | Every 15 s ±2 s, skipped when anything else went out in the last 10 s; every 5 s while the room waits for this device; every 30 s in the background |
| Status changes | At once, changes within 500 ms folded into one; Buffering only after 1.5 s of stall |
| Chat and reactions | 3 messages in 5 s; one reaction per 2 s |
| Sync answers | At most one per joiner across the room |
| Per device per relay | 20 events a minute, bursts of 12 in 10 s: heartbeats drop first |

Steady state is 4 events a minute per member. A `rate-limited:` refusal stops heartbeats to that
relay for 60 s and halves its cap for 10 minutes, and a refused event is never sent again to the
same relay. Receivers drop more than 60 events a minute from one member.

## Relays

Three per room, all read and written. One is enough to work; two is the target. The built-in set
is `wss://relay.primal.net`, `wss://relay.snort.social` and `wss://nostr.data.haus`, with spares
`wss://relay.nostr.net`, `wss://nos.lol` and `wss://relay.ditto.pub`. None of them is Kaeri's;
Settings calls them "public relays run by others". Settings ▸ Playback ▸ Relays sets the list for
new rooms; a room's Details ▸ Edit relays changes that room's list for everyone in it. Plain
`ws://` works only for `localhost` and `.local`.

- A relay is healthy when its socket is open and the device's own events come back on its
  subscription, and it hasn't refused three of them running (for any reason but `rate-limited:`
  or `duplicate:`: a web of trust or PoW the fresh session keys never meet). Every status carries
  a bit per relay.
- Leaving waits up to 2 s for each relay to answer or echo the last events before closing: a
  public relay takes an event a moment after it comes, and drops it if the socket is gone.
- A relay is swapped for the first spare only when more than half the members present have found
  it unhealthy for 60 s. A device with fewer than two healthy relays may add a spare, never remove
  one.
- AUTH (NIP-42) is answered with the session key.
- The clock is checked against each relay's HTTP `Date`; the median offset applies when two
  relays agree within 5 s and it is over 30 s.
- Joining tries the link's relays, then the spares, for 8 s each, then says "Couldn't reach this
  room's relays. Ask for a new invite."

## Sync

### The playhead

| Register | Holds | Written by |
|---|---|---|
| Playhead | episode, next, anchor position, play intent, cause, by, `wseq` (the write that started the current wait), `carry` (those carried on past), Up next | Anyone's play, pause, seek, Skip opening, episode change, Up next, Cancel, a stall, Carry on |
| Relays | The list | The majority rule, or Edit relays |
| Key epoch | The current secret | New invite |

A writer uses the highest seq it has seen plus one. Ties go to the lower `t`, then the lower key,
on every device. When Ada pauses and Mika plays at once, Ada's pause wins everywhere and Mika's
chat says "Ada already paused". A seq more than 1000 ahead is dropped.

### Position

The room's position is the playhead's anchor moved on by the time the room has played. A device
that started late moves its start back to match a peer's `pe`, by up to 15 s. Each device also
takes the median of the Watching members' positions and moves to it when it is more than 1 s off
on two statuses running. Drift correction then brings its player along: rewind past 4 s ahead,
fast-forward past 5 s behind, 0.95× past 1.5 s ahead, with a 2 s settle after a change.

### Waits

A write with cause `episode`, `move_on`, `seek`, `skip_opening` or `stall` starts a wait.

```
taking part = present (not Left, not Reconnecting)
            ∧ on the room's episode with the player open (Watching, Loading, Buffering, Behind)
            ∧ not catching up (rejoin) ∧ not Don't wait for me
waited      = those taking part whose rdy isn't this wait, minus carry
              (for a stall: only the one who stalled)
the room plays ⇔ its intent is play ∧ nobody is waited for
```

- A peer that hasn't heard the write yet is still waited for.
- At an episode's start every device opens it paused and sends `rdy` at its first frame; when the
  last does, everyone plays. Nobody sends play.
- Before the room has played an episode, it shows the episode with Play for everyone; people read
  Ready at their first frame, and once everyone is ready a count of 3 runs ("Starting in 3").
- After a seek the room's line shows "Paused for Ren" only once the wait passes 1 s.
- A stall waits only when the staller's own device writes it, after 1.5 s, in a room of 5 or
  fewer, and not again for someone who stalled twice in 10 minutes. In bigger rooms the row reads
  Buffering, then Behind while they catch up.
- **Nobody ever pauses for someone else's dropped connection.** A waited member who goes
  Reconnecting leaves the wait.
- **Carry on**, by anyone at any time: Play during a wait is Carry on. The apps bring the button
  forward at 20 s. Those carried past keep loading and come in at the room's place.
- **Don't wait for me** is the loader's own, from second 0.
- A person's pause during a wait sets the intent to pause: the room stays paused when it clears.

### Joining late

The newcomer sends a sync request. Members present answer in an order every device works out from
the request's nonce, 400 ms apart, and stay quiet once one has answered. The answer brings the
registers, the answerer's position and whether the room plays, and the roster. The newcomer takes
a free name ("Ren", else "Ren 2"), says Hello and comes in at the room's place. With no answer in
3 s the room is empty and the newcomer is the room. Rooms work best with 10 or fewer: past that,
the app warns before joining and Join anyway lets the person in.

A look-in (`peek`) gets the same answer without the looker ever appearing: an opened invite shows
who is there before Join the room, and a recent room shows the same before Rejoin.

### Up next and Skip opening

Both belong to the room; personal auto-next and auto-skip are off in one. Each device sets `ue`
when its own Up next point arrives. When everyone taking part has, any device writes the count:
10 s from the room's position, the same everywhere. A viewer whose file ends sooner sees "Waiting
for Ren to finish", with Play next now. Cancel and moving on share one order, so whichever was
pressed first wins. Skip opening is a seek with its own cause, to the presser's own skip data:
"Mika skipped the opening".

### Reconnecting and the background

Each relay reconnects on its own after 1, 2, 4, 8 and then every 15 s. Back on any relay the
device subscribes again, sends its status and always asks for a sync answer. Sockets are opened
afresh when the app comes to the foreground and on any network change.

Pauses the system makes (a call, going to the background) never reach the room. A phone going to
the background sends a last status that says Player closed, and the app ends its background task
on `RoomFlushed`. It reads Reconnecting after 75 s of silence and Left after 10 minutes. With
Picture in Picture or AirPlay the phone keeps playing in the room as usual.

### Move to a new room

For someone already in the room, which New invite can't remove. Anyone can start it and tick who
comes along; everyone present starts ticked except people muted on that device. The mover's device
makes a new secret and sends one Move event on the old room, with a payload for each ticked member
sealed between the two session keys. Those not ticked see that a move happened, and to whom, but
can't open any payload.

The mover goes first, with an ordinary Leave, and is the new room's first member from the same
place: the player keeps playing. Each ticked member sees "Mika moved the room. Go along?" with Go
along and Stay. Go along leaves the old room and joins the new one at its place, with no reload.
The offer lasts until it is answered, the member leaves, or 10 minutes pass. Nobody is pushed out:
the old room carries on for whoever stays, and says who moved and who went along.

## States

What shows on a row, highest first: Left, Reconnecting, the member's own reported state, Behind,
Paused, Watching.

| State | Comes from | Ends when |
|---|---|---|
| Watching | Their player plays the room's episode, ready for the current wait, the room playing | Any other state |
| Ready | Before play: their first frame is ready | The room plays |
| Paused, Paused it | The room's intent is pause; the one who paused reads Paused it | A play or Carry on |
| Loading (seconds) | Their device, opening the episode until the first frame | Ready, or No source |
| Buffering (seconds) | Their player stalls for more than 1.5 s | Playing again |
| Behind (seconds) | Their device: more than 3 s behind the room for 4 s | Within 1 s for 2 s |
| On episode 8 | Their player has another episode open | The episodes match |
| Choosing a source | Their picker for the room's episode is open | Picked: Loading |
| No source | Their sources check failed (the reason stays on their device) | A source is chosen, or the episode changes |
| Player closed | Player closed while in the room, or the app in the background | The player opens: Loading |
| Reconnecting | No event from them for 40 s (75 s at a 30 s heartbeat) | Any event from them |
| Left | A Leave, or silence for 3 minutes (10 in the background) | A Hello proving the same member id |

Timeouts in one place: heartbeat 15 s (5 s while waited for, 30 s in the background); stall grace
1.5 s; Behind 3 s for 4 s; a wait shows after 1 s; Carry on at 20 s; the count before play 3 s;
Reconnecting after 40 s (75 s); Left after 3 minutes (10); sync answer 3 s; relay join 8 s; the old
tag kept 2 minutes after a rekey; relay echo 20 s; relay swap after 60 s of majority reports;
background flush at most 2 s; Up next 10 s from the room's place; a Move offer 10 minutes.

Times the engine sends (`since_ms`, `ends_at_ms`, `starts_at_ms`, `carry_on_at_ms`) are Unix
epoch milliseconds on the device's clock. Apps show seconds as `now − since_ms` on their own
one-second tick.

## What members and relays see

Members see each other's names, a stable random member id, each person's state and position, the
room's episode, the chat and reactions, and who pressed what. They never see each other's IP
addresses, sources, plugins, or why a source failed.

A relay sees each member's IP address, one throwaway key per device per session, the day's room
tag, timing and padded sizes. It never sees names, the title or episode, positions, states or chat.
Room traffic follows the app's proxy setting when one is set. A room picked up again on another
day has a new tag and new keys, so a relay can't tell it is the same room. There is no forward
secrecy within one secret's life: someone who recorded the traffic and later gets the link could
read that room's chat. New invite limits that to one secret's life.

## Testing

No test ever reaches a public relay.

- `kaeri_room::fake` is an in-process room with scripted members who run the same room logic as a
  device: they join, answer sync requests, write playheads, load, stall, drop, chat, move and go
  along. A testing build's new rooms go to its fake relays.
- The Fixtures apps stage each board frame with one call, `FfiFakeRoom::stage(FfiRoomScene)`, as
  Ada with Mika and Ren in a room watching Stardust Wayfarer episode 9.
- The pure room logic (`kaeri_room::room`) has table tests for every row above and every wait
  rule.
- The real-relay checks (a gentle probe of each default relay and a six-device evening on the
  defaults) are run by hand with throwaway keys and Fixtures apps, never in CI.
