// Sound with no picture: the video plays on (its clock moves) but no frame is decoded. Some TVs
// play the audio of a stream whose video they can't decode and say nothing. After 3 s of the clock
// moving with videoWidth 0, or with no new decoded or shown frame, it is reported as a `video`
// error, once a load. The clock is the media element's own, so the check runs whatever state CAF
// reports (a TV with no picture can sit in BUFFERING while the sound plays).

export const SILENT_AFTER_S = 3;

export interface Sample {
	/** The video's clock, in seconds. */
	time: number;
	videoWidth: number;
	/** Decoded frames so far (getVideoPlaybackQuality, less the dropped ones), or null where the TV can't say. */
	frames: number | null;
	/** Frames shown so far (requestVideoFrameCallback), or null where the TV can't say. */
	presented?: number | null;
	/** The clock runs: not paused, seeking or ended. */
	playing: boolean;
}

const moved = (now: number | null | undefined, before: number | null | undefined) => now != null && before != null && now > before;

export class SilentPicture {
	private since: Sample | null = null;
	private reported = false;

	/** A new load starts the watch again. */
	reset() {
		this.since = null;
		this.reported = false;
	}

	/** Called about once a second. True the one time the picture is found missing. */
	check(s: Sample): boolean {
		if (this.reported || !s.playing) {
			this.since = null;
			return false;
		}
		// A frame was decoded or shown since the last good sample (or the TV can't count frames and
		// the video has a size).
		const counted = s.frames != null || s.presented != null;
		const picture =
			s.videoWidth > 0 &&
			(!counted || this.since == null || moved(s.frames, this.since.frames) || moved(s.presented, this.since.presented));
		if (!this.since || picture) {
			this.since = s;
			return false;
		}
		if (s.time - this.since.time >= SILENT_AFTER_S) {
			this.reported = true;
			return true;
		}
		return false;
	}
}

/** Counts the frames the video element shows, where requestVideoFrameCallback exists. */
export class ShownFrames {
	count: number | null = null;

	constructor(video: HTMLVideoElement) {
		type Rvfc = HTMLVideoElement & {
			requestVideoFrameCallback?: (cb: () => void) => number;
			cancelVideoFrameCallback?: (handle: number) => void;
		};
		const v = video as Rvfc;
		if (typeof v.requestVideoFrameCallback !== 'function') return;
		this.count = 0;
		let handle = 0;
		const tick = () => {
			this.count = (this.count ?? 0) + 1;
			handle = v.requestVideoFrameCallback!(tick);
		};
		handle = v.requestVideoFrameCallback(tick);
		// A new source may drop the waiting callback; one chain at a time.
		video.addEventListener('loadstart', () => {
			v.cancelVideoFrameCallback?.(handle);
			handle = v.requestVideoFrameCallback!(tick);
		});
	}
}
