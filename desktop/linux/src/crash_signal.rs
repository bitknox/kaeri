//! A best-effort crash record for a fatal signal, while Save crash reports is on. The panic hook
//! covers Rust panics; this covers SIGSEGV, SIGABRT and the like in the libraries underneath.
//!
//! A file is opened when saving is on, and the handler writes the signal and the module and
//! offset of each frame into it. A signal handler can do very little safely, so it formats into
//! a stack buffer and writes with write(2). The next launch turns a file that holds anything into
//! a report, redacted like any other, and removes it. While saving is off nothing is opened,
//! installed or written.

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod imp {
    use std::{
        ffi::{CStr, c_int, c_void},
        os::fd::IntoRawFd,
        path::{Path, PathBuf},
        sync::{
            OnceLock,
            atomic::{AtomicBool, AtomicI32, Ordering},
        },
    };

    use kaeri_model::crash::{CrashFrame, CrashKind, CrashPlace, CrashRecord};

    const FILE: &str = "crash-signal.txt";
    const SIGNALS: [c_int; 5] =
        [libc::SIGSEGV, libc::SIGABRT, libc::SIGBUS, libc::SIGFPE, libc::SIGILL];
    const FRAMES: usize = 48;

    static DIR: OnceLock<PathBuf> = OnceLock::new();
    static FD: AtomicI32 = AtomicI32::new(-1);
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    static COLLECTED: AtomicBool = AtomicBool::new(false);

    pub fn set_dir(dir: PathBuf) {
        let _ = DIR.set(dir);
    }

    /// Bring the handler in line with the saving setting: installed and ready while it is on,
    /// restored and closed while it is off. The first call also turns the last run's file into a
    /// report.
    pub fn sync() {
        let Some(dir) = DIR.get() else { return };
        let saving = kaeri_engine::crash::saving();
        if !COLLECTED.swap(true, Ordering::Relaxed) {
            collect(dir, saving);
        }
        if saving && !INSTALLED.load(Ordering::Relaxed) {
            install(dir);
        } else if !saving && INSTALLED.swap(false, Ordering::Relaxed) {
            uninstall(dir);
        }
    }

    fn collect(dir: &Path, saving: bool) {
        let path = dir.join(FILE);
        let Ok(text) = std::fs::read_to_string(&path) else { return };
        let at = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or_else(kaeri_engine::crash::now_ms, |d| {
                i64::try_from(d.as_millis()).unwrap_or(0)
            });
        let _ = std::fs::remove_file(&path);
        if !saving {
            return;
        }
        if let Some(record) = record_of(&text, at) {
            kaeri_engine::crash::save_record(&record);
        }
    }

    /// The report for a file the handler wrote: `signal SIGSEGV`, then `module offset` lines.
    pub fn record_of(text: &str, at_ms: i64) -> Option<CrashRecord> {
        let mut lines = text.lines();
        let reason = lines.next()?.strip_prefix("signal ")?.trim().to_owned();
        let frames = lines
            .filter_map(|line| {
                let (module, offset) = line.rsplit_once(' ')?;
                Some(CrashFrame {
                    module: module.to_owned(),
                    symbol: None,
                    offset: u64::from_str_radix(offset.trim_start_matches("0x"), 16).ok()?,
                })
            })
            .collect();
        Some(CrashRecord {
            at_ms,
            platform: "Linux".to_owned(), // i18n-skip
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            place: CrashPlace::Other,
            kind: CrashKind::Signal,
            reason,
            frames,
            ..CrashRecord::default()
        })
    }

    fn install(dir: &Path) {
        let _ = std::fs::create_dir_all(dir);
        let Ok(file) = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(dir.join(FILE))
        else {
            return;
        };
        FD.store(file.into_raw_fd(), Ordering::Relaxed);
        // A stack of its own, so a stack overflow can still be written about.
        let stack = Box::leak(vec![0u8; 64 * 1024].into_boxed_slice());
        // SAFETY: the stack is leaked, so it outlives the process; the handler only calls
        // functions that glibc documents as usable here, as far as a crash handler can.
        unsafe {
            let alt = libc::stack_t {
                ss_sp: stack.as_mut_ptr().cast::<c_void>(),
                ss_flags: 0,
                ss_size: stack.len(),
            };
            libc::sigaltstack(&raw const alt, std::ptr::null_mut());
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = handler as extern "C" fn(c_int) as usize;
            action.sa_flags = libc::SA_ONSTACK | libc::SA_RESETHAND | libc::SA_NODEFER;
            libc::sigemptyset(&raw mut action.sa_mask);
            for signal in SIGNALS {
                libc::sigaction(signal, &raw const action, std::ptr::null_mut());
            }
        }
        INSTALLED.store(true, Ordering::Relaxed);
    }

    fn uninstall(dir: &Path) {
        // SAFETY: putting the default action back.
        unsafe {
            for signal in SIGNALS {
                libc::signal(signal, libc::SIG_DFL);
            }
        }
        let fd = FD.swap(-1, Ordering::Relaxed);
        if fd >= 0 {
            // SAFETY: the descriptor was opened by install and is closed once.
            unsafe { libc::close(fd) };
        }
        let _ = std::fs::remove_file(dir.join(FILE));
    }

    fn write(fd: c_int, bytes: &[u8]) {
        // SAFETY: a plain write of a live slice.
        unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
    }

    /// `value` in hex, into `buf` from the end; the digits' start.
    fn hex(mut value: usize, buf: &mut [u8; 18]) -> usize {
        let mut at = buf.len();
        loop {
            at -= 1;
            buf[at] = b"0123456789abcdef"[value & 15];
            value >>= 4;
            if value == 0 {
                return at;
            }
        }
    }

    extern "C" fn handler(signal: c_int) {
        let fd = FD.load(Ordering::Relaxed);
        if fd < 0 {
            return;
        }
        let name: &[u8] = match signal {
            libc::SIGSEGV => b"SIGSEGV",
            libc::SIGABRT => b"SIGABRT",
            libc::SIGBUS => b"SIGBUS",
            libc::SIGFPE => b"SIGFPE",
            _ => b"SIGILL",
        };
        write(fd, b"signal "); // i18n-skip
        write(fd, name);
        write(fd, b"\n");
        let mut frames = [std::ptr::null_mut::<c_void>(); FRAMES];
        // SAFETY: the array holds FRAMES pointers; dladdr fills a plain struct.
        unsafe {
            let count = libc::backtrace(frames.as_mut_ptr(), FRAMES as c_int).max(0) as usize;
            for &frame in &frames[..count] {
                let mut info: libc::Dl_info = std::mem::zeroed();
                if libc::dladdr(frame, &raw mut info) == 0 || info.dli_fname.is_null() {
                    continue;
                }
                let path = CStr::from_ptr(info.dli_fname).to_bytes();
                let module = path.rsplit(|b| *b == b'/').next().unwrap_or(path);
                let offset = (frame as usize).wrapping_sub(info.dli_fbase as usize);
                let mut digits = [0u8; 18];
                let at = hex(offset, &mut digits);
                write(fd, module);
                write(fd, b" ");
                write(fd, &digits[at..]);
                write(fd, b"\n");
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_file_the_handler_wrote_becomes_a_signal_report() {
            let record = record_of("signal SIGSEGV\nlibmpv.so.2 1a2b3c\nkaeri-desktop ff\n", 7)
                .expect("a report");
            assert_eq!(record.kind, CrashKind::Signal);
            assert_eq!(record.reason, "SIGSEGV");
            assert_eq!(record.frames.len(), 2);
            assert_eq!(record.frames[0].module, "libmpv.so.2");
            assert_eq!(record.frames[0].offset, 0x1a2b3c);
            assert!(record_of("", 7).is_none());
        }

        #[test]
        fn hex_digits_fill_from_the_end() {
            let mut buf = [0u8; 18];
            let at = hex(0xbeef, &mut buf);
            assert_eq!(&buf[at..], b"beef");
            let at = hex(0, &mut buf);
            assert_eq!(&buf[at..], b"0");
        }
    }
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
mod imp {
    pub fn set_dir(_: std::path::PathBuf) {}
    pub fn sync() {}
}

pub use imp::{set_dir, sync};
