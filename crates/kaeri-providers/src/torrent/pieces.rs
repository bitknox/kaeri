//! Which bytes of a torrent file are already verified, worked out from the piece grid.
//!
//! The thumbnail path in [`super::http`] may only hand out bytes librqbit has hash-checked,
//! and must answer at once for the rest. Deciding which is pure arithmetic over the piece
//! grid plus a parse of librqbit's have-bitfield, so it lives here, apart from the session,
//! where it can be tested exhaustively without a swarm.

use crate::torrent::http::Range;

/// Where one file sits in its torrent's piece grid.
///
/// A torrent hashes the concatenation of all its files, so "is this byte verified" is a
/// question about the piece holding it, not about the file — and a file's first and last
/// pieces are usually shared with its neighbours in a batch. Both ends of an episode can
/// therefore be waiting on bytes that belong to a different file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PieceLayout {
    /// Offset of the file's first byte within the torrent's concatenated data.
    offset: u64,
    /// Length of every piece except perhaps the torrent's last, which may be shorter. A short
    /// last piece needs no special case: it still starts at a multiple of this.
    piece_len: u64,
}

impl PieceLayout {
    pub fn new(offset_in_torrent: u64, piece_len: u32) -> Self {
        // librqbit refuses a torrent with a zero piece length; the floor only keeps the
        // division below total.
        Self { offset: offset_in_torrent, piece_len: u64::from(piece_len.max(1)) }
    }

    /// The piece holding byte `at` of the file.
    pub fn piece_of(&self, at: u64) -> u64 {
        self.offset.saturating_add(at) / self.piece_len
    }

    /// The last byte of the file, in file coordinates, that `piece` holds.
    fn last_byte_in(&self, piece: u64) -> u64 {
        piece
            .saturating_add(1)
            .saturating_mul(self.piece_len)
            .saturating_sub(1)
            .saturating_sub(self.offset)
    }

    /// The leading part of `range` whose pieces are all verified.
    ///
    /// `None` when the piece holding the first byte is not. Only a *leading* run counts: a
    /// player reads forward from where it asked, so verified bytes beyond a gap are no use to
    /// this request, and serving them would mean skipping bytes it is expecting.
    pub fn ready_prefix(&self, range: Range, have: impl Fn(u64) -> bool) -> Option<Range> {
        let first = self.piece_of(range.start);
        let last = self.piece_of(range.end);
        if !have(first) {
            return None;
        }
        let mut piece = first;
        while piece < last && have(piece + 1) {
            piece += 1;
        }
        Some(Range { start: range.start, end: self.last_byte_in(piece).min(range.end) })
    }

    /// How much of a file `length` bytes long is held, as Stats for nerds' Pieces row says it:
    /// the pieces it spans that are verified, and whole pieces held in a row from `read`, the
    /// next byte the stream server would send. The piece holding that byte counts as ahead:
    /// the player has not had it all yet. `read` at or past the end leaves nothing ahead.
    /// `None` for an empty file, which spans no pieces.
    pub fn held(
        &self,
        length: u64,
        read: Option<u64>,
        have: impl Fn(u64) -> bool,
    ) -> Option<Held> {
        let last_byte = length.checked_sub(1)?;
        let (first, last) = (self.piece_of(0), self.piece_of(last_byte));
        let total = last - first + 1;
        let held = (first..=last).filter(|&piece| have(piece)).count() as u64;
        let ahead = read.map(|at| {
            if at >= length {
                return 0;
            }
            (self.piece_of(at)..=last).take_while(|&piece| have(piece)).count() as u64
        });
        let count = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
        Some(Held { held: count(held), total: count(total), ahead: ahead.map(count) })
    }
}

/// How much of the playing file is held, in its own pieces: see [`PieceLayout::held`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Held {
    /// The file's pieces that are verified, a piece it shares with a neighbour included.
    pub held: u32,
    /// The pieces the file spans.
    pub total: u32,
    /// Whole pieces held in a row from where the player reads, once it has read.
    pub ahead: Option<u32>,
}

/// A snapshot of which pieces of a torrent have been verified.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HavePieces(Vec<bool>);

impl HavePieces {
    pub fn from_bits(bits: Vec<bool>) -> Self {
        Self(bits)
    }

    /// Whether `piece` is verified. A piece past the end is not: a snapshot that does not
    /// cover it says nothing in its favour.
    pub fn has(&self, piece: u64) -> bool {
        usize::try_from(piece).ok().and_then(|i| self.0.get(i)).copied().unwrap_or(false)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Read the text librqbit's `Api::api_dump_haves` returns.
    ///
    /// That text is the only public view of the bitfield librqbit's own file streams consult
    /// before every read, so "verified" here means exactly what it means to playback. It is
    /// the `Debug` rendering of a `bitvec` slice — a header, then one `0` or `1` per bit of
    /// the bitfield's bytes, so the list runs to a multiple of eight and the pieces are its
    /// head. Five pieces, on one line:
    ///
    /// ```text
    /// BitSlice<u8, bitvec::order::Msb0> { addr: 0x6000…, head: 000, bits: 8 }
    ///     [1, 0, 1, 1, 0, 0, 0, 0]
    /// ```
    ///
    /// `Debug` output is not a stable interface, so this is strict rather than forgiving:
    /// anything but a list of zeros and ones exactly `pieces` long, or that padded to a whole
    /// byte with zeros, is `None`, and the thumbnail path then treats every piece as missing.
    /// A librqbit upgrade that changes the shape turns thumbnails off and leaves playback
    /// alone, and the session test that reads a real torrent's dump fails in CI before anyone
    /// has to notice.
    pub fn from_librqbit_dump(dump: &str, pieces: u32) -> Option<Self> {
        let list = dump.trim_end().strip_suffix(']')?;
        let (_, list) = list.rsplit_once('[')?;
        let mut bits = if list.trim().is_empty() {
            Vec::new()
        } else {
            list.split(',')
                .map(|bit| match bit.trim() {
                    "1" => Some(true),
                    "0" => Some(false),
                    _ => None,
                })
                .collect::<Option<Vec<bool>>>()?
        };
        let pieces = usize::try_from(pieces).ok()?;
        // A set padding bit would mean the list is not what it looks like.
        let padded = bits.len() == pieces.div_ceil(8) * 8 && !bits[pieces..].contains(&true);
        if bits.len() != pieces && !padded {
            return None;
        }
        bits.truncate(pieces);
        Some(Self(bits))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Byte-level truth: is byte `at` of the file in a verified piece?
    fn byte_ready(offset: u64, piece_len: u64, have: &[bool], at: u64) -> bool {
        have.get(((offset + at) / piece_len) as usize).copied().unwrap_or(false)
    }

    /// The reference answer, worked out one byte at a time.
    fn reference(offset: u64, piece_len: u64, have: &[bool], range: Range) -> Option<Range> {
        let mut end = None;
        for at in range.start..=range.end {
            if !byte_ready(offset, piece_len, have, at) {
                break;
            }
            end = Some(at);
        }
        end.map(|end| Range { start: range.start, end })
    }

    #[test]
    fn the_ready_prefix_matches_a_byte_by_byte_reference_for_every_small_layout() {
        // Every offset, length, piece size, availability pattern and range in a small space.
        // The arithmetic is all off-by-one territory — shared boundary pieces, a run ending
        // exactly at a piece edge, a range inside one piece — so exhaustive beats examples.
        let mut checked = 0_u64;
        for piece_len in 1..=4_u64 {
            for offset in 0..=3_u64 {
                for len in 1..=8_u64 {
                    let pieces = ((offset + len - 1) / piece_len + 1) as usize;
                    for pattern in 0..(1_u32 << pieces) {
                        let have: Vec<bool> =
                            (0..pieces).map(|p| pattern & (1 << p) != 0).collect();
                        let layout = PieceLayout::new(offset, piece_len as u32);
                        for start in 0..len {
                            for end in start..len {
                                let range = Range { start, end };
                                let got = layout.ready_prefix(range, |p| {
                                    have.get(p as usize).copied().unwrap_or(false)
                                });
                                assert_eq!(
                                    got,
                                    reference(offset, piece_len, &have, range),
                                    "offset {offset}, piece {piece_len}, {have:?}, {range:?}"
                                );
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(checked > 100_000, "the sweep shrank to {checked} cases");
    }

    #[test]
    fn a_file_sharing_its_first_piece_waits_on_its_neighbour() {
        // A batch: the episode starts 100 bytes into piece 3, whose first 100 bytes belong to
        // the previous file. Until piece 3 verifies — neighbour and all — the head is missing.
        let layout = PieceLayout::new(3 * 1024 + 924, 1024);
        let have = |p: u64| p != 3;
        assert_eq!(layout.ready_prefix(Range { start: 0, end: 5000 }, have), None);
        // Past it, the next piece is ready from the byte that starts it.
        assert_eq!(
            layout.ready_prefix(Range { start: 100, end: 5000 }, have),
            Some(Range { start: 100, end: 5000 })
        );
    }

    #[test]
    fn a_run_stops_at_the_last_byte_of_the_last_verified_piece() {
        let layout = PieceLayout::new(0, 1000);
        let have = |p: u64| p < 3;
        assert_eq!(
            layout.ready_prefix(Range { start: 10, end: 9999 }, have),
            Some(Range { start: 10, end: 2999 })
        );
    }

    #[test]
    fn verified_pieces_beyond_a_gap_do_not_extend_the_run() {
        // The player reads forward from where it asked; bytes past a hole are no use to it.
        let layout = PieceLayout::new(0, 10);
        let have = |p: u64| p != 1;
        assert_eq!(
            layout.ready_prefix(Range { start: 0, end: 99 }, have),
            Some(Range { start: 0, end: 9 })
        );
    }

    #[test]
    fn huge_offsets_saturate_instead_of_overflowing() {
        let layout = PieceLayout::new(u64::MAX - 10, 1 << 20);
        let got = layout.ready_prefix(Range { start: 0, end: 100 }, |_| true);
        assert_eq!(got.map(|r| r.start), Some(0));
    }

    #[test]
    fn the_held_pieces_are_the_file_s_own_shared_ends_included() {
        // A file 2500 bytes long, 500 bytes into piece 0 of a 1000-byte grid: pieces 0 to 2.
        let layout = PieceLayout::new(500, 1000);
        let held = layout.held(2500, None, |p| p != 1).unwrap();
        assert_eq!(held, Held { held: 2, total: 3, ahead: None }, "not read yet");
        // Pieces of other files do not count: piece 3 is past this one.
        let all = layout.held(2500, None, |p| p != 3).unwrap();
        assert_eq!((all.held, all.total), (3, 3));
        assert_eq!(layout.held(0, Some(0), |_| true), None, "an empty file spans no pieces");
    }

    #[test]
    fn pieces_ahead_run_from_the_piece_being_read_to_the_first_gap() {
        let layout = PieceLayout::new(0, 100);
        // Ten pieces; 4 is missing.
        let ahead = |read| layout.held(1000, Some(read), |p| p != 4).unwrap().ahead;
        assert_eq!(ahead(0), Some(4), "pieces 0 to 3");
        assert_eq!(ahead(250), Some(2), "the piece being read counts");
        assert_eq!(ahead(450), Some(0), "reading in the gap");
        assert_eq!(ahead(500), Some(5), "to the end of the file");
    }

    #[test]
    fn at_the_end_of_the_file_nothing_is_ahead() {
        let layout = PieceLayout::new(0, 100);
        let ahead = |read| layout.held(950, Some(read), |_| true).unwrap().ahead;
        assert_eq!(ahead(949), Some(1), "the last, short piece");
        assert_eq!(ahead(950), Some(0), "everything sent");
        assert_eq!(ahead(5_000), Some(0), "a read past the end is the end");
        // A file ending inside piece 2, which the next file shares: pieces past it never count.
        let held = layout.held(250, Some(0), |_| true).unwrap();
        assert_eq!((held.total, held.ahead), (3, Some(3)));
    }

    #[test]
    fn a_piece_past_the_snapshot_is_not_verified() {
        let have = HavePieces::from_bits(vec![true, true]);
        assert!(have.has(1));
        assert!(!have.has(2));
        assert!(!have.has(u64::MAX));
    }

    #[test]
    fn librqbits_dump_parses_into_one_bit_per_piece() {
        let dump = "BitSlice<u8, bitvec::order::Msb0> { addr: 0x600003a4c010, head: 000, \
                    bits: 5 } [1, 0, 1, 1, 0]";
        let have = HavePieces::from_librqbit_dump(dump, 5).unwrap();
        assert_eq!(have, HavePieces::from_bits(vec![true, false, true, true, false]));
    }

    #[test]
    fn a_dump_padded_to_a_whole_byte_keeps_only_the_pieces() {
        // What librqbit actually renders: the bitfield's bytes, so 70 pieces come as 72 bits.
        // Missed at first because the session test's torrent happened to have eight pieces.
        let dump = "BitSlice<u8, bitvec::order::Msb0> { addr: 0x1, head: 000, bits: 8 } \
                    [1, 0, 1, 1, 0, 0, 0, 0]";
        let have = HavePieces::from_librqbit_dump(dump, 5).unwrap();
        assert_eq!(have, HavePieces::from_bits(vec![true, false, true, true, false]));
        assert!(!have.has(5), "a padding bit is not a piece");
    }

    #[test]
    fn padding_that_is_set_or_overlong_is_refused() {
        // Padding is always zero, so a one there means the list is not what it seems; and
        // more than a byte's worth of slack is not padding at all.
        assert_eq!(HavePieces::from_librqbit_dump("[1, 0, 1, 1, 0, 0, 1, 0]", 5), None);
        let sixteen = format!("[{}]", vec!["0"; 16].join(", "));
        assert_eq!(HavePieces::from_librqbit_dump(&sixteen, 5), None);
    }

    #[test]
    fn an_empty_torrent_dump_is_an_empty_snapshot() {
        let dump = "BitSlice<u8, bitvec::order::Msb0> { addr: 0x1, head: 000, bits: 0 } []";
        assert_eq!(HavePieces::from_librqbit_dump(dump, 0), Some(HavePieces::default()));
    }

    #[test]
    fn a_dump_of_the_wrong_shape_is_refused_rather_than_guessed_at() {
        // Strict on purpose: `Debug` output can change under an upgrade, and a misread
        // bitfield would hand out unverified bytes. Refusing turns thumbnails off instead.
        let cases = [
            ("[1, 0, 1]", 4),            // a piece short
            ("[1, 0, 1, 1, 1]", 4),      // a piece over
            ("[1, 0, 2, 1]", 4),         // not a bit
            ("[true, false]", 2),        // a different rendering
            ("0b1010", 4),               // no list at all
            ("[1, 0, 1, 1", 4),          // truncated
            ("BitSlice { bits: 4 }", 4), // header only
        ];
        for (dump, pieces) in cases {
            assert_eq!(HavePieces::from_librqbit_dump(dump, pieces), None, "{dump:?}");
        }
    }
}
