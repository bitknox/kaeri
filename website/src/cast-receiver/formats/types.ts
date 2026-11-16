// The shapes every unpacker shares: a container's tracks and the samples read from it, with
// times in seconds on the stream's own timeline (zero at its first sample).

export type Kind = 'video' | 'audio' | 'text';

/**
 * Codec names, the same whichever container carried them. Video: h264, hevc, vp8, vp9, av1,
 * mpeg2, mpeg4. Audio: aac, mp3, mp2, mp1, ac3, eac3, dts, truehd, mlp, flac, opus, vorbis, alac,
 * and pcm_* in FFmpeg's names. Text: srt, webvtt, ass, ssa, tx3g, pgs, vobsub.
 */
export type Codec = string;

export interface Track {
	/** The container's own number for the track (Matroska track number, MP4 track id, TS PID). */
	id: number;
	kind: Kind;
	codec: Codec;
	/**
	 * The codec's setup bytes: avcC, hvcC, av1C or vpcC for video; the AudioSpecificConfig, OpusHead,
	 * FLAC metadata or Vorbis headers for audio; the ASS header or the VobSub .idx for subtitles.
	 */
	config?: Uint8Array;
	width?: number;
	height?: number;
	sampleRate?: number;
	channels?: number;
	bitDepth?: number;
	language?: string;
	name?: string;
	default?: boolean;
	forced?: boolean;
	/** The codec string the container or playlist declared (avc1.64001f), when it did. */
	declared?: string;
	/** Packets aren't single frames (MPEG-TS audio): a decoder runs its parser first. */
	unframed?: boolean;
}

export interface Sample {
	track: number;
	/** Presentation time, seconds. */
	pts: number;
	/** Decode time, seconds. */
	dts: number;
	/** Seconds; 0 when the container didn't say (the muxer then uses the gap to the next one). */
	duration: number;
	key: boolean;
	data: Uint8Array;
}

export interface Attachment {
	name: string;
	mime: string;
	data: Uint8Array;
}

/** One container or playlist, read in order from a position. */
export interface Demuxer {
	readonly tracks: Track[];
	/** Seconds, or NaN when the container doesn't say. */
	readonly duration: number;
	open(): Promise<void>;
	/**
	 * Moves reading to the last keyframe at or before `time` (for the video track, or the first
	 * track when there is no video). Returns the time reading starts from.
	 */
	seek(time: number): Promise<number>;
	/** The next samples in decode order, or null at the end. */
	read(): Promise<Sample[] | null>;
	/** Fonts and other files the container carries (Matroska attachments). */
	attachments?(): Promise<Attachment[]>;
	/** Only these tracks are wanted now; segmented sources stop fetching the others. */
	select?(ids: number[]): void;
	/** Spans skipped because their segments never came, for playback to step over. */
	readonly skipped?: [number, number][];
	/** Stops a read that waits on the network, for a seek elsewhere. */
	interrupt?(): void;
	close(): void;
}
