// The player: Google's Cast Application Framework (CAF) on the receiver's own video element. All
// of the receiver's CAF setup is here, so the screens (main.ts) only see the hooks below and the
// few calls on `Player`. Playing more formats (feat/cast-receiver-formats) extends this module.

import type { Formats } from './formats/receiver';
import { NAMESPACE } from './protocol';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
declare const cast: any;

export interface ErrorEventLike {
	detailedErrorCode?: number;
	error?: unknown;
	reason?: string;
}

/** What the screens hear from the player and the senders. */
export interface Hooks {
	/** A LOAD arrived; return the request (changed or not) for CAF to play. */
	load(request: unknown): unknown;
	playing(): void;
	paused(): void;
	buffering(isBuffering: boolean): void;
	seeked(): void;
	/** The media ended: `end` (played to the end), `stopped`, `error` or `interrupted` (a new load). */
	finished(reason: 'end' | 'stopped' | 'error' | 'interrupted' | 'other'): void;
	/** A player error; `fatal()` tells whether it left the player idle. */
	error(event: ErrorEventLike, fatal: () => boolean): void;
	/** The sender changed the text tracks. */
	tracksChanged(): void;
	senderMessage(data: unknown, senderId: string): void;
	senderConnected(senderId: string, userAgent: string | undefined): void;
	/** The last sender left on its own word; return true to keep the receiver up. */
	lastSenderLeft(): boolean;
}

export interface Player {
	/** CAF's detailed error codes by number, for reports. */
	errorNames: Record<number, string>;
	position(): number;
	duration(): number;
	pause(): void;
	activeTextTracks(): number[];
	/** Switches the audio rendition in place; false when there is no such track. */
	switchAudio(language: string, name?: string): boolean;
	/** Sends on Kaeri's namespace; to every sender when `to` is left out. */
	send(body: Record<string, unknown>, to?: string): void;
	/** How many senders are connected now. */
	senders(): number;
	/** Closes the receiver; the TV goes back to its own screen. */
	close(): void;
}

/** `formats` unpacks loads on the receiver itself (formats/receiver.ts); without it CAF plays every load. */
/**
 * Whether the receiver stays up once its last sender has left: while an episode is loaded or
 * plays, and through Finished, it does, and the idle close in main.ts ends it once nothing plays.
 * At Ready or after an error it closes, as CAF would.
 */
export function staysWithoutSenders(phase: string): boolean {
	return phase === 'loading' || phase === 'playing' || phase === 'gone' || phase === 'finished';
}

export function startPlayer(video: HTMLVideoElement, hooks: Hooks, formats?: Formats): Player {
	const fw = cast.framework;
	const context = fw.CastReceiverContext.getInstance();
	const manager = context.getPlayerManager();
	manager.setMediaElement(video);

	const codes = fw.events.DetailedErrorCode ?? {};
	const errorNames: Record<number, string> = Object.fromEntries(
		Object.entries(codes)
			.filter(([, v]) => typeof v === 'number')
			.map(([k, v]) => [v as number, k]),
	);

	const E = fw.events.EventType;
	const M = fw.messages.MessageType;
	const ended = fw.events.EndedReason ?? {};
	manager.setMessageInterceptor(M.LOAD, async (request: unknown) => {
		const r = hooks.load(request);
		if (!formats) return r;
		const out = await formats.load(r);
		if (out !== null) return out;
		// The receiver already sent `error {part: video}`; CAF answers the LOAD as failed, so nothing
		// plays the sound without the picture.
		return new fw.messages.ErrorData(fw.messages.ErrorType.LOAD_FAILED);
	});
	manager.setMessageInterceptor(M.EDIT_TRACKS_INFO, (request: unknown) => {
		setTimeout(() => hooks.tracksChanged(), 200);
		return request;
	});
	manager.addEventListener(E.PLAYING, () => hooks.playing());
	manager.addEventListener(E.PAUSE, () => hooks.paused());
	manager.addEventListener(E.BUFFERING, (e: { isBuffering: boolean }) => hooks.buffering(!!e.isBuffering));
	manager.addEventListener(E.SEEKED, () => hooks.seeked());
	manager.addEventListener(E.MEDIA_FINISHED, (e: { endedReason?: string }) => {
		const r = e.endedReason;
		hooks.finished(
			r === (ended.END_OF_STREAM ?? 'END_OF_STREAM')
				? 'end'
				: r === (ended.STOPPED ?? 'STOPPED')
					? 'stopped'
					: r === (ended.ERROR ?? 'ERROR')
						? 'error'
						: r === (ended.INTERRUPTED ?? 'INTERRUPTED')
							? 'interrupted'
							: 'other',
		);
	});
	manager.addEventListener(E.ERROR, (e: ErrorEventLike) =>
		hooks.error(e, () => {
			try {
				return manager.getPlayerState() === fw.messages.PlayerState.IDLE;
			} catch {
				return true;
			}
		}),
	);

	context.addCustomMessageListener(NAMESPACE, (e: { data: unknown; senderId: string }) => {
		let data = e.data;
		if (typeof data === 'string') {
			try {
				data = JSON.parse(data);
			} catch {
				return;
			}
		}
		hooks.senderMessage(data, e.senderId);
	});
	context.addEventListener(fw.system.EventType.SENDER_CONNECTED, (e: { senderId: string; userAgent?: string }) =>
		hooks.senderConnected(e.senderId, e.userAgent),
	);

	// CAF closes the receiver when its last sender leaves on its own word: Play services' Cast
	// card leaving, or Kaeri's sender ended by the system. The handler takes CAF's place, so an
	// episode plays on and the sender joins it again when it comes back; nothing on screen closes
	// as CAF would.
	if (typeof context.setLastSenderDisconnectedHandler === 'function') {
		context.setLastSenderDisconnectedHandler(() => {
			if (!hooks.lastSenderLeft()) context.stop();
		});
	}

	const options = new fw.CastReceiverOptions();
	options.customNamespaces = { [NAMESPACE]: fw.system.MessageType.JSON };
	// The receiver closes itself after ten minutes with nothing to play (main.ts).
	options.disableIdleTimeout = true;
	options.supportedCommands = fw.messages.Command.ALL_BASIC_MEDIA;
	context.start(options);

	const safe = <T>(f: () => T, fallback: T): T => {
		try {
			return f() ?? fallback;
		} catch {
			return fallback;
		}
	};

	return {
		errorNames,
		position: () => safe(() => manager.getCurrentTimeSec() || 0, 0),
		duration: () => safe(() => manager.getDurationSec() || 0, 0),
		pause: () => safe(() => manager.pause(), undefined),
		activeTextTracks: () => safe(() => manager.getTextTracksManager().getActiveIds(), [] as number[]),
		switchAudio: (language, name) =>
			safe(() => {
				const inside = formats?.switchAudio(language, name);
				if (inside != null) return inside;
				const atm = manager.getAudioTracksManager();
				const tracks: { trackId: number; language?: string; name?: string }[] = atm.getTracks() ?? [];
				const pick =
					tracks.find((tr) => tr.language === language && (!name || tr.name === name)) ??
					tracks.find((tr) => tr.language === language);
				if (!pick) return false;
				atm.setActiveById(pick.trackId);
				return true;
			}, false),
		send: (body, to) => {
			try {
				context.sendCustomMessage(NAMESPACE, to, body);
			} catch (e) {
				console.warn('send', body.type, e);
			}
		},
		senders: () => safe(() => (context.getSenders() ?? []).length, 0),
		close: () => context.stop(),
	};
}
