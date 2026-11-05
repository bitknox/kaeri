//! The peer road: two devices swap what the other lacks over any ordered stream. The host wraps
//! the stream in the remote's mutual TLS (ALPN `kaeri-sync/1`); frames carry no secrets of their
//! own, so the TLS session is what keeps them private.
//!
//! Frame: `u32 length | JSON`. Each side sends `Hello` with its version vector, then `Ops` frames
//! of at most [`FRAME_OPS`] ops for what the other lacks, then `Done`.

use kaeri_store::{
    Store,
    sync::{Applied, Op, VersionVector},
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::{Result, SyncError};

pub const ALPN: &[u8] = b"kaeri-sync/1";
pub const FRAME_OPS: usize = 500;
const MAX_FRAME: usize = 8 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
enum Frame {
    Hello { device: String, vector: VersionVector },
    Ops { ops: Vec<Op> },
    Done,
}

#[derive(Debug, Default)]
pub struct PeerReport {
    pub sent: usize,
    pub received: usize,
    pub applied: Applied,
    /// What the other device sent for what this one asked, before the log.
    pub handoff: Option<crate::handoff::Received>,
}

async fn put<W: AsyncWrite + Unpin>(w: &mut W, frame: &Frame) -> Result<()> {
    let body = serde_json::to_vec(frame).map_err(|_| SyncError::Damaged("encoding"))?;
    w.write_all(&(body.len() as u32).to_be_bytes()).await.map_err(io)?;
    w.write_all(&body).await.map_err(io)?;
    w.flush().await.map_err(io)
}

async fn get<R: AsyncRead + Unpin>(r: &mut R) -> Result<Frame> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len).await.map_err(io)?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(SyncError::Damaged("oversized frame"));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).await.map_err(io)?;
    serde_json::from_slice(&body).map_err(|_| SyncError::Damaged("frame"))
}

fn io(e: std::io::Error) -> SyncError {
    SyncError::Network(e.to_string())
}

/// Run one exchange. Both ends call this; the order of writes and reads is symmetric because each
/// side writes all it has before reading, and the stream buffers (or the two run concurrently).
pub async fn exchange<S>(store: &Store, device: &str, stream: S) -> Result<PeerReport>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let (mut rd, mut wr) = tokio::io::split(stream);
    let mine = store.version_vector()?;
    let send =
        async { put(&mut wr, &Frame::Hello { device: device.to_owned(), vector: mine }).await };
    let recv = get(&mut rd);
    let (sent_hello, theirs) = tokio::join!(send, recv);
    sent_hello?;
    let Frame::Hello { device: peer, vector } = theirs? else {
        return Err(SyncError::Damaged("expected hello"));
    };
    let mut report = PeerReport::default();
    let outgoing = async {
        let mut sent = 0;
        let mut have = vector.clone();
        loop {
            let ops = store.ops_after(&have, FRAME_OPS)?;
            if ops.is_empty() {
                break;
            }
            for op in &ops {
                let e = have.entry(op.device.clone()).or_insert(0);
                *e = (*e).max(op.seq);
            }
            sent += ops.len();
            put(&mut wr, &Frame::Ops { ops }).await?;
        }
        put(&mut wr, &Frame::Done).await?;
        Ok::<_, SyncError>(sent)
    };
    let incoming = async {
        let mut received = 0;
        let mut applied = Applied::default();
        loop {
            match get(&mut rd).await? {
                Frame::Ops { ops } => {
                    received += ops.len();
                    let a = store.apply_ops(ops)?;
                    applied.ops += a.ops;
                    applied.duplicates += a.duplicates;
                    applied.gaps += a.gaps;
                    applied.unknown += a.unknown;
                    applied.effects.extend(a.effects);
                    applied.notes.extend(a.notes);
                    applied.titles.extend(a.titles);
                    applied.watches.extend(a.watches);
                }
                Frame::Done => break,
                Frame::Hello { .. } => return Err(SyncError::Damaged("second hello")),
            }
        }
        Ok::<_, SyncError>((received, applied))
    };
    let (sent, got) = tokio::join!(outgoing, incoming);
    report.sent = sent?;
    (report.received, report.applied) = got?;
    store.sync_touch_peer(&peer, now_secs())?;
    Ok(report)
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use kaeri_core::ids::AnilistId;
    use kaeri_store::WatchEvent;

    use super::*;

    fn dev(id: &str) -> Store {
        let s = Store::open_in_memory().unwrap();
        s.sync_enable(id).unwrap();
        s
    }

    fn watch(s: &Store, ep: &str, pos: f64, at: i64) {
        let mut e = WatchEvent::new(AnilistId::new(9_001), ep, pos);
        e.duration_secs = Some(1440.0);
        e.at = at;
        s.record_event(&e).unwrap();
    }

    #[tokio::test]
    async fn two_peers_swap_what_the_other_lacks() {
        let (a, b) = (dev("00000000000000a1"), dev("00000000000000b2"));
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        watch(&a, "1", 100.0, now - 20);
        watch(&b, "2", 200.0, now - 10);
        let (x, y) = tokio::io::duplex(1 << 16);
        let (ra, rb) = tokio::join!(
            exchange(&a, "00000000000000a1", x),
            exchange(&b, "00000000000000b2", y)
        );
        let (ra, rb) = (ra.unwrap(), rb.unwrap());
        assert!(ra.received > 0 && rb.received > 0);
        assert_eq!(a.resume_position(AnilistId::new(9_001), "2").unwrap(), Some(200.0));
        assert_eq!(b.resume_position(AnilistId::new(9_001), "1").unwrap(), Some(100.0));
        assert_eq!(a.version_vector().unwrap(), b.version_vector().unwrap());
    }

    #[tokio::test]
    async fn a_large_history_crosses_in_several_frames() {
        let (a, b) = (dev("00000000000000a1"), dev("00000000000000b2"));
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        for n in 0..(FRAME_OPS as i64 * 2 + 30) {
            watch(&a, &format!("{n}"), 50.0, now - 5000 + n);
        }
        let (x, y) = tokio::io::duplex(1 << 12);
        let (ra, rb) = tokio::join!(
            exchange(&a, "00000000000000a1", x),
            exchange(&b, "00000000000000b2", y)
        );
        ra.unwrap();
        assert!(rb.unwrap().received > FRAME_OPS * 2);
        assert_eq!(a.version_vector().unwrap(), b.version_vector().unwrap());
    }

    #[tokio::test]
    async fn garbage_from_a_stranger_is_refused() {
        let a = dev("00000000000000a1");
        let (x, mut y) = tokio::io::duplex(1 << 12);
        let task = tokio::spawn(async move {
            y.write_all(&[0xff; 8]).await.unwrap();
            y
        });
        let r = exchange(&a, "00000000000000a1", x).await;
        let _ = task.await;
        assert!(r.is_err());
    }
}
