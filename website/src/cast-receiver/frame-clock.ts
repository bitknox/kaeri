// Times every subtitle frame JASSUB draws. It stands in for JASSUB's own debug hooks
// (`_startFrame`, `_endFrame`, `_drop`), so nothing is logged. A second is behind when most of its
// frames were dropped or drawn after the video frame they belong to; five such seconds in a row
// while the video plays and the switch to WebVTT is due.

/** A frame drawn this much after its video frame counts as late. */
export const LATE_MS = 50;
export const BEHIND_SECONDS = 5;

export class FrameClock {
	private start = 0;
	private drawn = 0;
	private late = 0;
	private dropped = 0;
	private streak = 0;

	constructor(private readonly now: () => number = () => performance.now()) {}

	_startFrame() {
		this.start = this.now();
	}

	_endFrame(meta: { expectedDisplayTime: number }) {
		this.drawn++;
		if (this.now() - meta.expectedDisplayTime > LATE_MS) this.late++;
	}

	_drop() {
		this.dropped++;
	}

	/**
	 * Called once a second. `playing` is false while paused, seeking or waiting for the stream,
	 * when no frames are due. Returns true once rendering has been behind for five seconds.
	 */
	tick(playing: boolean): boolean {
		const frames = this.drawn + this.dropped;
		const bad = this.late + this.dropped;
		this.drawn = this.late = this.dropped = 0;
		if (!playing) return false;
		if (frames >= 3 && bad / frames > 0.5) this.streak++;
		else if (frames >= 3) this.streak = 0;
		return this.streak >= BEHIND_SECONDS;
	}
}
