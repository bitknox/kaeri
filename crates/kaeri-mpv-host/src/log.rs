//! mpv's log lines as the app's log may hold them: every address cut to its scheme, host and port,
//! as the engine's log says one, since a stream's carries its signed token in its path or query.
//! The Android app's `MpvLog`, in Rust.

/// `https://cdn.example`, from `https://cdn.example/a/b.m3u8?token=...`: the scheme, the host and
/// the port, never the credentials, the path or the query. The model's, which the player log
/// cuts its lines with too.
pub fn address(url: &str) -> String {
    kaeri_model::bug_report::cut_address(url)
}

/// `text` with every address in it said as [`address`] says one. An address is a scheme and
/// `://`, and everything up to a space or a quote.
pub fn redacting(text: &str) -> String {
    kaeri_model::bug_report::cut_addresses(text)
}

/// What libplacebo says when it gives up on a shader hook, as the engine's `HookDisabled`
/// takes it: the line, trimmed, when it is one.
pub fn disabled_hook(text: &str) -> Option<String> {
    (text.contains("hook") && text.contains("disabling")).then(|| text.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_keeps_its_host_and_port_only() {
        assert_eq!(address("https://cdn.example/a/b.m3u8?token=secret"), "https://cdn.example");
        assert_eq!(address("http://user:pw@127.0.0.1:8123/m/x"), "http://127.0.0.1:8123");
        assert_eq!(address("/home/viewer/episode.mkv"), "a local file");
        assert_eq!(address("episode.mkv"), "an address");
    }

    #[test]
    fn every_address_in_a_line_is_cut() {
        let line = "Opening https://cdn.example/seg/1.ts?sig=abc failed, retrying \"http://b.example:81/x\"";
        assert_eq!(
            redacting(line),
            "Opening https://cdn.example failed, retrying \"http://b.example:81\""
        );
        assert_eq!(redacting("no address here"), "no address here");
        assert_eq!(
            redacting("[ffmpeg] hls: https://a.example/p"),
            "[ffmpeg] hls: https://a.example"
        );
        // A `://` with no scheme before it is left as it is.
        assert_eq!(redacting("odd ://thing"), "odd ://thing");
    }

    #[test]
    fn a_disabled_hook_is_told_by_its_words() {
        assert_eq!(
            disabled_hook("Failed executing hook ArtCNN, disabling\n").as_deref(),
            Some("Failed executing hook ArtCNN, disabling")
        );
        assert_eq!(disabled_hook("Opening the file"), None);
    }
}
