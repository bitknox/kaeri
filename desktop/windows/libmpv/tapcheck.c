// Checks a Windows libmpv's demuxer audio tap (android/mpv/patches/mpv/0001-demux-audio-tap.patch)
// on the machine it runs on: opens a file paused, with the whole file in the demuxer cache, and
// counts what the tap hands over for two windows. .github/workflows/libmpv-windows.yml builds it
// beside each libmpv and runs it on an x64 and an Arm64 Windows runner.
//
//   tapcheck <media>
//
// The media is at least 100 s long with an audio track. It passes when the tap delivers about
// 30 s of 16-bit mono at 11025 Hz for each of 0-30 and 60-90, nothing outside them, with the tag
// it was given and a break between the two. It prints what it saw either way.
#include <math.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>

#include <mpv/client.h>

#define RATE 11025.0
#define TAG 7

static atomic_llong samples_in[2];
static atomic_llong samples_outside;
static atomic_llong calls;
static atomic_int breaks;
static atomic_int wrong_tag;
static atomic_int saw_first;
static double first_pts = -1;

static void tap(void *ctx, int64_t tag, double pts, const int16_t *samples, int count)
{
    (void)ctx;
    (void)samples;
    atomic_fetch_add(&calls, 1);
    if (tag != TAG)
        atomic_store(&wrong_tag, 1);
    if (count == 0) {
        atomic_fetch_add(&breaks, 1);
        return;
    }
    int expected = 0;
    if (atomic_compare_exchange_strong(&saw_first, &expected, 1))
        first_pts = pts;
    // Half a second of slack at each edge: the tap decides by packet, and a packet that starts
    // inside a window is delivered whole.
    double end = pts + count / RATE;
    if (pts >= -0.5 && end <= 30.5)
        atomic_fetch_add(&samples_in[0], count);
    else if (pts >= 59.5 && end <= 90.5)
        atomic_fetch_add(&samples_in[1], count);
    else
        atomic_fetch_add(&samples_outside, count);
}

static int check(int code, const char *what)
{
    if (code < 0) {
        fprintf(stderr, "tapcheck: %s: %s\n", what, mpv_error_string(code));
        exit(1);
    }
    return code;
}

int main(int argc, char **argv)
{
    if (argc != 2) {
        fprintf(stderr, "usage: tapcheck <media>\n");
        return 2;
    }
    // The tap is found as an app finds it: by name in the libmpv that is loaded.
    HMODULE lib = GetModuleHandleA("libmpv-2.dll");
    typedef int (*set_tap_fn)(mpv_handle *, mpv_audio_tap_fn, void *);
    set_tap_fn set_tap = lib ? (set_tap_fn)(void *)GetProcAddress(lib, "mpv_set_audio_tap") : NULL;
    printf("mpv_set_audio_tap: %s\n", set_tap ? "exported" : "missing");
    if (!set_tap)
        return 1;

    mpv_handle *mpv = mpv_create();
    if (!mpv) {
        fprintf(stderr, "tapcheck: mpv_create failed\n");
        return 1;
    }
    // A window that ends before it starts is refused, as on every other platform.
    int refused = mpv_set_option_string(mpv, "demuxer-audio-tap-windows", "60-10") < 0;
    printf("windows 60-10 refused: %s\n", refused ? "yes" : "no");
    const char *options[][2] = {
        {"vo", "null"}, {"ao", "null"}, {"pause", "yes"}, {"cache", "yes"},
        {"demuxer-readahead-secs", "600"}, {"config", "no"}, {"terminal", "no"},
        {"demuxer-audio-tap-tag", "7"}, {"demuxer-audio-tap-windows", "0-30,60-90"},
    };
    for (size_t i = 0; i < sizeof(options) / sizeof(options[0]); i++)
        check(mpv_set_option_string(mpv, options[i][0], options[i][1]), options[i][0]);
    check(mpv_initialize(mpv), "mpv_initialize");
    check(set_tap(mpv, tap, NULL), "mpv_set_audio_tap");
    char *version = mpv_get_property_string(mpv, "mpv-version");
    char *ffmpeg = mpv_get_property_string(mpv, "ffmpeg-version");
    printf("%s, FFmpeg %s\n", version ? version : "mpv ?", ffmpeg ? ffmpeg : "?");
    mpv_free(version);
    mpv_free(ffmpeg);

    const char *load[] = {"loadfile", argv[1], NULL};
    check(mpv_command(mpv, load), "loadfile");

    // The whole file is in the cache within seconds, paused; the tap's thread decodes it.
    const long long want = (long long)(29.0 * RATE);
    ULONGLONG deadline = GetTickCount64() + 60000;
    while (GetTickCount64() < deadline) {
        mpv_event *event = mpv_wait_event(mpv, 0.2);
        if (event->event_id == MPV_EVENT_END_FILE) {
            fprintf(stderr, "tapcheck: the file ended or failed to load\n");
            break;
        }
        if (atomic_load(&samples_in[0]) >= want && atomic_load(&samples_in[1]) >= want &&
            atomic_load(&breaks) >= 1)
            break;
    }
    mpv_terminate_destroy(mpv);

    double in0 = atomic_load(&samples_in[0]) / RATE, in1 = atomic_load(&samples_in[1]) / RATE;
    double outside = atomic_load(&samples_outside) / RATE;
    printf("calls %lld, breaks %d, first pts %.3f\n", (long long)atomic_load(&calls),
           atomic_load(&breaks), first_pts);
    printf("0-30: %.2f s, 60-90: %.2f s, outside: %.2f s, wrong tag: %s\n", in0, in1, outside,
           atomic_load(&wrong_tag) ? "yes" : "no");
    int ok = refused && fabs(in0 - 30.0) < 1.0 && fabs(in1 - 30.0) < 1.0 && outside == 0 &&
             atomic_load(&breaks) >= 1 && !atomic_load(&wrong_tag) && first_pts >= 0 &&
             first_pts < 0.5;
    printf("tapcheck: %s\n", ok ? "pass" : "FAIL");
    return ok ? 0 : 1;
}
