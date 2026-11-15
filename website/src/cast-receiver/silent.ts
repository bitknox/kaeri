// Sound with no picture: the video plays on (its clock moves) but no frame is decoded. Some TVs
// play the audio of a stream whose video they can't decode and say nothing. After 3 s of playing
// with videoWidth 0, or with no new decoded frame, it is reported as a `video` error, once a load.

export const SILENT_AFTER_S = 3;

export interface Sample {
	/** The video's clock, in seconds. */
	time: number;
	videoWidth: number;
	/** Decoded frames so far (getVideoPlaybackQuality), or null where the TV can't say. */
	frames: number | null;
	/** Playing, not paused, seeking or waiting. */
	playing: boolean;
}

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
		// A frame was decoded since the last good sample (or the TV can't count frames and the
		// video has a size).
		const picture =
			s.videoWidth > 0 && (s.frames == null || this.since == null || s.frames > (this.since.frames ?? 0));
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
