//! Telling an HLS playlist from media by its first bytes, for every proxy that rewrites one.

/// How far into a body its `#EXTM3U` may start and still be read as a playlist.
///
/// A playlist may carry leading bytes before `#EXTM3U`. A few dozen bytes leaves room for
/// them and is too little for a segment that merely contains the tag to be taken for a
/// playlist.
pub const PLAYLIST_LEAD: usize = 64;

/// The content type a rewritten playlist is served with, whatever the host labelled it.
///
/// ffmpeg opens a stream as HLS only when its URL ends in `.m3u8` or its content type names a
/// playlist, and a proxied URL ends however the host's URL did.
pub const PLAYLIST_TYPE: &str = "application/vnd.apple.mpegurl";

const TAG: &[u8] = b"#EXTM3U";

/// Where the playlist in `head` starts: the offset of its `#EXTM3U`, when that is within
/// [`PLAYLIST_LEAD`] bytes of the start. `None` when it is not there.
pub fn playlist_start(head: &[u8]) -> Option<usize> {
    let lead = &head[..head.len().min(PLAYLIST_LEAD + TAG.len())];
    lead.windows(TAG.len()).position(|window| window == TAG)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_playlist_is_found_at_the_start_or_after_a_few_leading_bytes() {
        assert_eq!(playlist_start(b"#EXTM3U\n#EXT-X-ENDLIST\n"), Some(0));
        assert_eq!(playlist_start(b"\xEF\xBB\xBF#EXTM3U\n"), Some(3));
        assert_eq!(playlist_start(b"\x00\x01\x02\x03\x04\x05\x06\n#EXTM3U\n"), Some(8));
    }

    #[test]
    fn the_tag_further_in_than_the_lead_is_not_a_playlist() {
        let mut body = vec![0x47; PLAYLIST_LEAD + 1];
        body.extend_from_slice(b"#EXTM3U\n");
        assert_eq!(playlist_start(&body), None);
        let mut edge = vec![0x47; PLAYLIST_LEAD];
        edge.extend_from_slice(b"#EXTM3U\n");
        assert_eq!(playlist_start(&edge), Some(PLAYLIST_LEAD));
        assert_eq!(playlist_start(b"#EXTM"), None);
        assert_eq!(playlist_start(b""), None);
    }
}
