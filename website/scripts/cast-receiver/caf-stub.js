// A stand-in for Google's Cast receiver SDK, for scripts/cast-receiver-check.ts. It plays on the
// page's own video element like CAF does and lets the check drive it through `window.__caf`:
// load, sender messages, errors and what the receiver sent back (`__caf.sent`).
(() => {
	const EventType = {
		PLAYING: 'PLAYING',
		PAUSE: 'PAUSE',
		BUFFERING: 'BUFFERING',
		SEEKED: 'SEEKED',
		MEDIA_FINISHED: 'MEDIA_FINISHED',
		ERROR: 'ERROR',
	};
	const DetailedErrorCode = {
		MEDIA_UNKNOWN: 100,
		MEDIA_DECODE: 102,
		MEDIA_NETWORK: 103,
		MEDIA_SRC_NOT_SUPPORTED: 104,
		LOAD_FAILED: 905,
		HLS_NETWORK_MASTER_PLAYLIST: 311,
		SEGMENT_NETWORK: 314,
	};
	const PlayerState = { IDLE: 'IDLE', PLAYING: 'PLAYING', PAUSED: 'PAUSED', BUFFERING: 'BUFFERING' };

	const listeners = {};
	const interceptors = {};
	const custom = [];
	const system = {};
	let video = null;
	let state = PlayerState.IDLE;
	let finishedSent = false;
	// While the check holds a stall, the real video's own playing and waiting events don't count.
	let held = false;
	const sent = [];
	const text = { active: [] };
	const audio = { tracks: [], active: null };

	const emit = (type, event = {}) => (listeners[type] || []).forEach((f) => f({ type, ...event }));
	const finish = (endedReason) => {
		if (finishedSent) return;
		finishedSent = true;
		state = PlayerState.IDLE;
		emit(EventType.MEDIA_FINISHED, { endedReason });
	};

	function wire(v) {
		v.addEventListener('playing', () => {
			if (held) return;
			state = PlayerState.PLAYING;
			emit(EventType.PLAYING);
		});
		v.addEventListener('pause', () => {
			if (v.ended) return;
			state = PlayerState.PAUSED;
			emit(EventType.PAUSE);
		});
		v.addEventListener('waiting', () => !held && emit(EventType.BUFFERING, { isBuffering: true }));
		v.addEventListener('seeked', () => emit(EventType.SEEKED));
		v.addEventListener('ended', () => finish('END_OF_STREAM'));
		v.addEventListener('error', () => {
			if (!v.getAttribute('src')) return;
			const code = v.error ? v.error.code : 0;
			state = PlayerState.IDLE;
			emit(EventType.ERROR, {
				detailedErrorCode: code === 4 ? DetailedErrorCode.MEDIA_SRC_NOT_SUPPORTED : DetailedErrorCode.MEDIA_UNKNOWN,
				error: { message: v.error ? v.error.message : '' },
			});
			finish('ERROR');
		});
	}

	const manager = {
		setMediaElement(v) {
			video = v;
			wire(v);
		},
		setMessageInterceptor(type, f) {
			interceptors[type] = f;
		},
		addEventListener(type, f) {
			(listeners[type] ||= []).push(f);
		},
		getCurrentTimeSec: () => (video ? video.currentTime : 0),
		getDurationSec: () => (video && Number.isFinite(video.duration) ? video.duration : 0),
		getPlayerState: () => state,
		pause: () => video && video.pause(),
		getTextTracksManager: () => ({ getActiveIds: () => text.active.slice() }),
		getAudioTracksManager: () => ({
			getTracks: () => audio.tracks,
			setActiveById: (id) => (audio.active = id),
		}),
	};

	const context = {
		getPlayerManager: () => manager,
		addCustomMessageListener(ns, f) {
			custom.push({ ns, f });
		},
		addEventListener(type, f) {
			(system[type] ||= []).push(f);
		},
		sendCustomMessage(ns, to, body) {
			sent.push({ ns, to: to ?? null, body: JSON.parse(JSON.stringify(body)) });
		},
		start(options) {
			context.options = options;
			context.started = true;
		},
		stop() {
			context.stopped = true;
		},
		started: false,
		stopped: false,
	};

	window.cast = {
		framework: {
			CastReceiverContext: { getInstance: () => context },
			CastReceiverOptions: function () {},
			events: { EventType, DetailedErrorCode, EndedReason: { END_OF_STREAM: 'END_OF_STREAM', ERROR: 'ERROR', STOPPED: 'STOPPED', INTERRUPTED: 'INTERRUPTED' } },
			messages: {
				MessageType: { LOAD: 'LOAD', EDIT_TRACKS_INFO: 'EDIT_TRACKS_INFO' },
				PlayerState,
				Command: { ALL_BASIC_MEDIA: 12303 },
			},
			system: { EventType: { SENDER_CONNECTED: 'senderconnected' }, MessageType: { JSON: 'JSON' } },
		},
	};
	window.__cafStub = true;

	window.__caf = {
		sent,
		context,
		text,
		audio,
		/** A sender's LOAD; `play: false` holds it on Starting. */
		async load(request, { play = true } = {}) {
			if (state !== PlayerState.IDLE && !finishedSent) emit(EventType.MEDIA_FINISHED, { endedReason: 'INTERRUPTED' });
			// CAF waits for an interceptor that returns a promise.
			const r = interceptors.LOAD ? await interceptors.LOAD(request) : request;
			finishedSent = false;
			state = PlayerState.BUFFERING;
			window.__caf.pending = r;
			if (play) window.__caf.play();
		},
		play() {
			const r = window.__caf.pending;
			if (window.__caf.playing !== r) {
				window.__caf.playing = r;
				video.src = r.media.contentUrl || r.media.contentId;
				if (r.currentTime) video.currentTime = r.currentTime;
			}
			video.play().catch(() => {});
		},
		editTracks(ids) {
			text.active = ids;
			if (interceptors.EDIT_TRACKS_INFO) interceptors.EDIT_TRACKS_INFO({ activeTrackIds: ids });
		},
		connect(senderId, userAgent) {
			(system.senderconnected || []).forEach((f) => f({ senderId, userAgent }));
		},
		message(data, senderId = 'sender-1') {
			custom.forEach(({ f }) => f({ data, senderId }));
		},
		error(event, { idle = true } = {}) {
			if (idle) state = PlayerState.IDLE;
			emit(EventType.ERROR, event);
		},
		buffering(isBuffering) {
			held = isBuffering;
			emit(EventType.BUFFERING, { isBuffering });
		},
	};
})();
