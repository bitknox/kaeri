// The receiver's audio decoder: a thin layer over FFmpeg's libavcodec, compiled to WebAssembly
// by build.sh. It turns compressed audio packets into interleaved stereo float samples for the
// AudioWorklet. One handle per audio stream. This file is MIT; the FFmpeg libraries it links stay
// under the LGPL 2.1 and ship as separate files with their notice (NOTICE.txt beside the output).

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include <libavcodec/avcodec.h>
#include <libavutil/channel_layout.h>
#include <libavutil/mem.h>
#include <libavutil/opt.h>

#define MAX_HANDLES 4

typedef struct {
	AVCodecContext *ctx;
	AVCodecParserContext *parser;
	AVPacket *pkt;
	AVFrame *frame;
	float *out;
	int out_cap;   // in stereo frames
	int out_len;   // stereo frames ready
	int rate;
	int channels;  // the source's channel count, last frame
} Handle;

static Handle handles[MAX_HANDLES];

static int grow(Handle *h, int frames) {
	if (h->out_len + frames <= h->out_cap) return 0;
	int cap = h->out_cap ? h->out_cap : 4096;
	while (cap < h->out_len + frames) cap *= 2;
	float *n = realloc(h->out, (size_t)cap * 2 * sizeof(float));
	if (!n) return -1;
	h->out = n;
	h->out_cap = cap;
	return 0;
}

static float sample(const AVFrame *f, int ch, int i) {
	int planar = av_sample_fmt_is_planar(f->format);
	int nch = f->ch_layout.nb_channels;
	const uint8_t *base = planar ? f->extended_data[ch] : f->extended_data[0];
	int idx = planar ? i : i * nch + ch;
	switch (av_get_packed_sample_fmt(f->format)) {
	case AV_SAMPLE_FMT_U8: return (((const uint8_t *)base)[idx] - 128) / 128.0f;
	case AV_SAMPLE_FMT_S16: return ((const int16_t *)base)[idx] / 32768.0f;
	case AV_SAMPLE_FMT_S32: return ((const int32_t *)base)[idx] / 2147483648.0f;
	case AV_SAMPLE_FMT_FLT: return ((const float *)base)[idx];
	case AV_SAMPLE_FMT_DBL: return (float)((const double *)base)[idx];
	default: return 0;
	}
}

// Stereo from any layout: the front pair, with the centre and the surrounds folded in at -3 dB.
// LFE is left out, as most downmixes do.
static void downmix(Handle *h, const AVFrame *f) {
	int n = f->nb_samples;
	int nch = f->ch_layout.nb_channels;
	if (grow(h, n) < 0) return;
	float *o = h->out + (size_t)h->out_len * 2;
	h->channels = nch;
	if (nch == 1) {
		for (int i = 0; i < n; i++) o[2 * i] = o[2 * i + 1] = sample(f, 0, i);
	} else if (nch == 2 || f->ch_layout.order != AV_CHANNEL_ORDER_NATIVE) {
		for (int i = 0; i < n; i++) {
			o[2 * i] = sample(f, 0, i);
			o[2 * i + 1] = sample(f, 1, i);
		}
	} else {
		const AVChannelLayout *l = &f->ch_layout;
		int fl = av_channel_layout_index_from_channel(l, AV_CHAN_FRONT_LEFT);
		int fr = av_channel_layout_index_from_channel(l, AV_CHAN_FRONT_RIGHT);
		int fc = av_channel_layout_index_from_channel(l, AV_CHAN_FRONT_CENTER);
		int sl = av_channel_layout_index_from_channel(l, AV_CHAN_SIDE_LEFT);
		int sr = av_channel_layout_index_from_channel(l, AV_CHAN_SIDE_RIGHT);
		int bl = av_channel_layout_index_from_channel(l, AV_CHAN_BACK_LEFT);
		int br = av_channel_layout_index_from_channel(l, AV_CHAN_BACK_RIGHT);
		const float c = 0.7071f;
		float scale = 1.0f / (1.0f + c);
		for (int i = 0; i < n; i++) {
			float L = fl >= 0 ? sample(f, fl, i) : 0;
			float R = fr >= 0 ? sample(f, fr, i) : 0;
			if (fc >= 0) {
				float v = c * sample(f, fc, i);
				L += v;
				R += v;
			}
			if (sl >= 0) L += c * sample(f, sl, i);
			if (sr >= 0) R += c * sample(f, sr, i);
			if (bl >= 0) L += c * sample(f, bl, i);
			if (br >= 0) R += c * sample(f, br, i);
			L *= scale;
			R *= scale;
			o[2 * i] = L > 1 ? 1 : L < -1 ? -1 : L;
			o[2 * i + 1] = R > 1 ? 1 : R < -1 ? -1 : R;
		}
	}
	h->out_len += n;
	h->rate = f->sample_rate;
}

static int drain(Handle *h) {
	for (;;) {
		int r = avcodec_receive_frame(h->ctx, h->frame);
		if (r == AVERROR(EAGAIN) || r == AVERROR_EOF) return 0;
		if (r < 0) return r;
		downmix(h, h->frame);
		av_frame_unref(h->frame);
	}
}

static int send(Handle *h, AVPacket *p) {
	int r = avcodec_send_packet(h->ctx, p);
	// A broken packet is skipped; the next one decodes.
	if (r < 0 && r != AVERROR(EAGAIN)) return drain(h) < 0 ? -1 : 0;
	return drain(h);
}

// Opens a decoder by FFmpeg name. `parse` runs the bytes through the codec's parser first, for
// streams whose packets aren't single frames (MPEG-TS). Returns a handle, or a negative number.
int kd_open(const char *name, const uint8_t *extra, int extra_len, int rate, int channels, int parse) {
	int id = -1;
	for (int i = 0; i < MAX_HANDLES; i++)
		if (!handles[i].ctx) {
			id = i;
			break;
		}
	if (id < 0) return -2;
	const AVCodec *codec = avcodec_find_decoder_by_name(name);
	if (!codec) return -3;
	Handle *h = &handles[id];
	memset(h, 0, sizeof(*h));
	h->ctx = avcodec_alloc_context3(codec);
	if (!h->ctx) return -4;
	if (extra_len > 0) {
		h->ctx->extradata = av_mallocz(extra_len + AV_INPUT_BUFFER_PADDING_SIZE);
		memcpy(h->ctx->extradata, extra, extra_len);
		h->ctx->extradata_size = extra_len;
	}
	if (rate > 0) h->ctx->sample_rate = rate;
	if (channels > 0) av_channel_layout_default(&h->ctx->ch_layout, channels);
	if (avcodec_open2(h->ctx, codec, NULL) < 0) {
		avcodec_free_context(&h->ctx);
		return -5;
	}
	if (parse) h->parser = av_parser_init(codec->id);
	h->pkt = av_packet_alloc();
	h->frame = av_frame_alloc();
	return id;
}

// Decodes one packet (or, with the parser, any run of bytes). Returns the stereo frames now
// waiting in kd_out, or a negative number when the decoder failed.
int kd_decode(int id, const uint8_t *data, int len) {
	if (id < 0 || id >= MAX_HANDLES || !handles[id].ctx) return -1;
	Handle *h = &handles[id];
	h->out_len = 0;
	if (h->parser) {
		while (len > 0) {
			uint8_t *out = NULL;
			int out_len = 0;
			int used = av_parser_parse2(h->parser, h->ctx, &out, &out_len, data, len, AV_NOPTS_VALUE,
			                            AV_NOPTS_VALUE, 0);
			if (used < 0) break;
			data += used;
			len -= used;
			if (out_len) {
				h->pkt->data = out;
				h->pkt->size = out_len;
				if (send(h, h->pkt) < 0) return -1;
			}
		}
		return h->out_len;
	}
	uint8_t *buf = av_malloc(len + AV_INPUT_BUFFER_PADDING_SIZE);
	if (!buf) return -1;
	memcpy(buf, data, len);
	memset(buf + len, 0, AV_INPUT_BUFFER_PADDING_SIZE);
	if (av_packet_from_data(h->pkt, buf, len) < 0) {
		av_free(buf);
		return -1;
	}
	int r = send(h, h->pkt);
	av_packet_unref(h->pkt);
	return r < 0 ? -1 : h->out_len;
}

float *kd_out(int id) { return handles[id].out; }
int kd_rate(int id) { return handles[id].rate ? handles[id].rate : handles[id].ctx->sample_rate; }
int kd_channels(int id) { return handles[id].channels; }

// After a seek: forget what the decoder holds.
void kd_flush(int id) {
	if (id < 0 || id >= MAX_HANDLES || !handles[id].ctx) return;
	Handle *h = &handles[id];
	avcodec_flush_buffers(h->ctx);
	if (h->parser) {
		enum AVCodecID cid = h->ctx->codec_id;
		av_parser_close(h->parser);
		h->parser = av_parser_init(cid);
	}
	h->out_len = 0;
}

void kd_close(int id) {
	if (id < 0 || id >= MAX_HANDLES || !handles[id].ctx) return;
	Handle *h = &handles[id];
	if (h->parser) av_parser_close(h->parser);
	avcodec_free_context(&h->ctx);
	av_packet_free(&h->pkt);
	av_frame_free(&h->frame);
	free(h->out);
	memset(h, 0, sizeof(*h));
}
