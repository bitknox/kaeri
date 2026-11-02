//! The proxy torrent traffic can be routed through, as the native apps' torrent sheet sets it:
//! an optional privacy setting, off by default, when torrents connect directly. The rules turning
//! what the sheet asks for into the file's key, and the proxy's address into the URL the file
//! keeps and out of it again.
//!
//! The proxy's password is never in the file. The host's secret store keeps it, under
//! [`PROXY_SECRET`], and the engine puts it into the URL only on the copy of the config a
//! torrent session is brought up on.

use kaeri_core::{config::TorrentConfig, settings::SettingValue};

use crate::stores::Secret;

/// Where the host's secret store keeps the proxy's password.
pub const PROXY_SECRET: &str = "torrent-proxy";

/// The proxy torrent traffic goes through.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TorrentProxy {
    pub host: String,
    pub port: u16,
    /// The user name, when the proxy asks for one.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub user: Option<String>,
    /// The password: kept in the host's secret store, never in `config.toml`. `None` keeps the
    /// one already kept, so a sheet that leaves the field alone need not ask for it again; an
    /// empty one forgets it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub password: Option<Secret>,
}

/// Torrent playback's route, as the file says: what the torrent sheet and Sources show.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TorrentView {
    /// The proxy torrent traffic goes through, when one is set: the sheet's fields, filled.
    /// Without its password, which stays where it is kept. `None`: torrents connect directly.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub proxy: Option<ProxyView>,
}

/// The proxy as the file names it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ProxyView {
    pub host: String,
    pub port: u16,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub user: Option<String>,
}

/// How the file routes torrent traffic.
pub fn view(torrent: &TorrentConfig) -> TorrentView {
    TorrentView {
        proxy: torrent.proxy.as_deref().and_then(parse).map(|address| ProxyView {
            host: address.host,
            port: address.port,
            user: address.user,
        }),
    }
}

/// Why a proxy asked for is not written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// A proxy with no host or no port.
    NoProxy,
}

impl Refusal {
    /// The refusal in the words the notice says it in.
    pub const fn reason(self) -> &'static str {
        match self {
            Self::NoProxy => "a proxy needs its host and port",
        }
    }
}

/// One key of the file: its table, as `kaeri_core::settings::write_keys` takes it, its
/// name, and its value.
pub type Edit = (&'static [&'static str], &'static str, SettingValue);

/// One key of the file to take out: its table and its name.
pub type Removal = (&'static [&'static str], &'static str);

/// Where the proxy key lives.
pub const TORRENT: &[&str] = &["providers", "torrent"];

/// What the file is given for a proxy asked for, as one change: the proxy's URL without its
/// password, or with `None` the key taken out, so torrents connect directly. Or why not.
pub fn edits(proxy: Option<&TorrentProxy>) -> Result<(Vec<Edit>, Vec<Removal>), Refusal> {
    let Some(proxy) = proxy else {
        return Ok((Vec::new(), vec![(TORRENT, "proxy")]));
    };
    if proxy.host.trim().is_empty() || proxy.port == 0 {
        return Err(Refusal::NoProxy);
    }
    let user = proxy.user.as_deref().map(str::trim).filter(|user| !user.is_empty());
    let url = socks_url(proxy.host.trim(), proxy.port, user, None);
    Ok((vec![(TORRENT, "proxy", SettingValue::Str(url))], Vec::new()))
}

/// A proxy's address, as a SOCKS5 URL names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyAddress {
    pub host: String,
    pub port: u16,
    pub user: Option<String>,
    pub password: Option<String>,
}

/// The URL the file keeps for a proxy: `socks5://user:password@host:port`, the user and the
/// password escaped, and an IPv6 host in brackets.
pub fn socks_url(host: &str, port: u16, user: Option<&str>, password: Option<&str>) -> String {
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    let credentials = match (user, password) {
        (Some(user), Some(password)) => format!("{}:{}@", escape(user), escape(password)),
        (Some(user), None) => format!("{}@", escape(user)),
        (None, _) => String::new(),
    };
    format!("socks5://{credentials}{host}:{port}")
}

/// A SOCKS5 URL's parts, or `None` for one that is not a SOCKS5 URL with a host and a port.
pub fn parse(url: &str) -> Option<ProxyAddress> {
    let rest = url.trim();
    let rest = rest.strip_prefix("socks5h://").or_else(|| rest.strip_prefix("socks5://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let (credentials, address) = match authority.rsplit_once('@') {
        Some((credentials, address)) => (Some(credentials), address),
        None => (None, authority),
    };
    let (host, port) = address.rsplit_once(':')?;
    let host = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host);
    let port = port.parse().ok().filter(|port| *port != 0)?;
    if host.is_empty() {
        return None;
    }
    let (user, password) = match credentials.map(|c| c.split_once(':').unwrap_or((c, ""))) {
        Some((user, password)) => (
            Some(unescape(user)).filter(|user| !user.is_empty()),
            Some(unescape(password)).filter(|password| !password.is_empty()),
        ),
        None => (None, None),
    };
    Some(ProxyAddress { host: host.to_owned(), port, user, password })
}

/// `url` with `password` put in it: what a torrent session is brought up on, when the file
/// names a user and no password and the secret store keeps one. `None` when there is nothing
/// to put it beside or one is there already.
pub fn with_password(url: &str, password: &str) -> Option<String> {
    let address = parse(url)?;
    if address.user.is_none() || address.password.is_some() || password.is_empty() {
        return None;
    }
    Some(socks_url(&address.host, address.port, address.user.as_deref(), Some(password)))
}

/// Percent-escaped, keeping what a URL's user information may hold as it is.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// [`escape`] undone. A `%` that starts no escape is kept as it is.
fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let escaped = (bytes[at] == b'%')
            .then(|| text.get(at + 1..at + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match escaped {
            Some(byte) => {
                out.push(byte);
                at += 3;
            }
            None => {
                out.push(bytes[at]);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proxy(user: Option<&str>) -> TorrentProxy {
        TorrentProxy {
            host: "192.0.2.10".into(),
            port: 1080,
            user: user.map(Into::into),
            password: Some(Secret("lantern".into())),
        }
    }

    #[test]
    fn a_proxy_is_written_without_its_password() {
        let (set, removed) = edits(Some(&proxy(Some("mika")))).unwrap();
        assert_eq!(
            set,
            vec![(TORRENT, "proxy", SettingValue::Str("socks5://mika@192.0.2.10:1080".into()))]
        );
        assert!(removed.is_empty());
        assert!(!format!("{set:?}").contains("lantern"), "the password stays out of the file");
    }

    #[test]
    fn no_proxy_takes_the_key_out() {
        assert_eq!(edits(None), Ok((Vec::new(), vec![(TORRENT, "proxy")])));
    }

    #[test]
    fn a_proxy_needs_its_host_and_port() {
        assert_eq!(
            edits(Some(&TorrentProxy { port: 0, ..proxy(None) })),
            Err(Refusal::NoProxy)
        );
        let blank = TorrentProxy { host: " ".into(), ..proxy(None) };
        assert_eq!(edits(Some(&blank)), Err(Refusal::NoProxy));
    }

    #[test]
    fn a_proxy_url_reads_back_as_written_and_takes_its_password_on_bring_up() {
        let url = socks_url("fe80::1", 1080, Some("mika@home"), None);
        assert_eq!(url, "socks5://mika%40home@[fe80::1]:1080");
        let address = parse(&url).unwrap();
        assert_eq!(
            (address.host.as_str(), address.port, address.user.as_deref()),
            ("fe80::1", 1080, Some("mika@home"))
        );
        assert_eq!(address.password, None);

        let full = with_password(&url, "p:ss").unwrap();
        assert_eq!(full, "socks5://mika%40home:p%3Ass@[fe80::1]:1080");
        assert_eq!(parse(&full).unwrap().password.as_deref(), Some("p:ss"));
        assert_eq!(with_password(&full, "other"), None, "one is there already");
        assert_eq!(with_password("socks5://192.0.2.10:1080", "p"), None, "no user to go with");

        // What the terminal's users write by hand reads too.
        let typed = parse("socks5h://192.0.2.10:1080").unwrap();
        assert_eq!((typed.host.as_str(), typed.user), ("192.0.2.10", None));
        assert_eq!(parse("http://192.0.2.10:1080"), None);
        assert_eq!(parse("socks5://192.0.2.10"), None, "no port");
    }

    #[test]
    fn the_view_reads_the_file() {
        let torrent = TorrentConfig {
            proxy: Some("socks5://mika:secret@192.0.2.10:1080".into()),
            ..TorrentConfig::default()
        };
        assert_eq!(
            view(&torrent).proxy,
            Some(ProxyView {
                host: "192.0.2.10".into(),
                port: 1080,
                user: Some("mika".into())
            })
        );
        assert_eq!(view(&TorrentConfig::default()), TorrentView::default());
    }
}
