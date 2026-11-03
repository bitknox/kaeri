//! The proxy torrent traffic can be routed through, as a native app's torrent sheet sets it:
//! written to the file as one change and the proxy's password to the host's secret store; and
//! that password put back into the proxy's URL on the copy of the config a torrent session is
//! brought up on, and nowhere else.

use std::sync::Arc;

use kaeri_core::config::Config;
use kaeri_model::{
    Cause, Msg,
    event::Event,
    notice::Notice,
    proxy::{self, PROXY_SECRET, Refusal, TorrentProxy},
    settings::{SettingKey, SettingsView},
    sources::ProxyStatus,
    stores::Secret,
};

use crate::{
    host::SecretStore,
    outbox::{Internal, Outbox},
    services::Services,
    state::EngineState,
};

/// Route torrent traffic through `proxy`, or with `None` connect directly, and say how that
/// went: the settings as the file then leaves them, and a notice. Refused in words, with
/// nothing written, for a proxy with no address.
///
/// Torrent playback is brought back up on what was written when it is on.
pub fn set_torrent_proxy(
    proxy: Option<TorrentProxy>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let (edits, removals) = match proxy::edits(proxy.as_ref()) {
        Ok(changes) => changes,
        Err(refusal) => {
            let refused = Some(SettingKey::TorrentProxy);
            let _ = tx.send(Event::Settings(SettingsView { refused, ..engine.config.view() }));
            let cause = refusal_cause(refusal);
            let _ = tx.send(Event::Toast(Notice::alert(Msg::NoticeNotSavedBecause { cause })));
            return;
        }
    };
    let written = engine.config.write_proxy(&services.paths, edits, removals);
    let refused = written.is_err().then_some(SettingKey::TorrentProxy);
    let _ = tx.send(Event::Settings(SettingsView { refused, ..engine.config.view() }));
    match written {
        Ok(()) => {
            let proxied = proxy.is_some();
            // Before the source is brought back up, which reads it.
            keep_password(proxied, proxy.and_then(|proxy| proxy.password), services, tx);
            let saved = if proxied { Msg::NoticeProxySaved } else { Msg::NoticeProxyRemoved };
            let _ = tx.send(Event::Toast(Notice::info(saved)));
            let _ = tx.internal(Internal::SettingsSaved);
            if engine.config.effective().providers.torrent.enabled {
                let _ = tx.internal(Internal::SourcesChanged);
            }
        }
        Err(e) => {
            let reason = match e {
                kaeri_core::Error::Config(reason) => reason,
                other => other.to_string(),
            };
            let _ = tx.send(Event::Toast(Notice::alert(Msg::NoticeNotSaved { reason })));
        }
    }
}

/// Check a proxy not yet saved, as a sheet asks before it saves: the check torrents make, made
/// once, the verdict on [`Event::ProxyChecked`]. Nothing is written and nothing running
/// changes.
///
/// A proxy given with no password is checked with the one kept, as saving it would keep it;
/// one given empty, with none. The password is only ever in the check's copy of the URL.
pub fn check_torrent_proxy(
    proxy: TorrentProxy,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let config = match trial(engine.config.effective(), proxy, || kept_password(services)) {
        Ok(config) => config,
        Err(cause) => {
            not_checked(cause, tx);
            return;
        }
    };
    let route = match crate::sources::route_for(&config, services.fixture_route) {
        Ok(route) => route,
        // Its reason can name the proxy's URL, password and all: said in fixed words instead.
        Err(_) => {
            not_checked(Cause::ProxyAddressUnusable, tx);
            return;
        }
    };
    let tx = tx.clone();
    tokio::spawn(async move {
        let state = route.verify().await;
        let _ = tx.send(Event::ProxyChecked(crate::sources::proxy_status(&state)));
    });
}

/// `config` with `proxy` in place of its own, for a check and nothing else. The proxy's
/// password is the one given, or `kept` when none is and the proxy names a user; given empty,
/// there is none.
fn trial(
    config: &Config,
    proxy: TorrentProxy,
    kept: impl FnOnce() -> Option<String>,
) -> Result<Config, Cause> {
    if proxy.host.trim().is_empty() || proxy.port == 0 {
        return Err(Cause::ProxyNeedsAddress);
    }
    let user = proxy.user.as_deref().map(str::trim).filter(|user| !user.is_empty());
    let password = match proxy.password {
        Some(Secret(password)) => Some(password).filter(|password| !password.is_empty()),
        None if user.is_some() => kept(),
        None => None,
    };
    let mut config = config.clone();
    let url = proxy::socks_url(proxy.host.trim(), proxy.port, user, password.as_deref());
    config.providers.torrent.proxy = Some(url);
    Ok(config)
}

/// A proxy that cannot be checked at all: `Failed`, and why.
fn not_checked(cause: Cause, tx: &Outbox) {
    let _ = tx.send(Event::ProxyChecked(ProxyStatus::Failed));
    let _ = tx.send(Event::Toast(Notice::alert(Msg::NoticeProxyNotChecked { cause })));
}

/// Why a proxy was refused, as the notice says it.
fn refusal_cause(refusal: Refusal) -> Cause {
    match refusal {
        Refusal::NoProxy => Cause::ProxyNeedsAddress,
    }
}

/// The proxy's password the secret store keeps, if any.
fn kept_password(services: &Services) -> Option<String> {
    match secrets(services).get(PROXY_SECRET) {
        Ok(password) => password,
        Err(error) => {
            tracing::warn!(%error, "could not read the proxy's password");
            None
        }
    }
}

/// Keep the proxy's password in the host's secret store: the one given, or the one kept when
/// none is; forgotten when it is given empty, or when no proxy is used any more.
fn keep_password(proxied: bool, password: Option<Secret>, services: &Services, tx: &Outbox) {
    let secrets = secrets(services);
    let kept = match (proxied, password) {
        (true, None) => return,
        (true, Some(Secret(password))) if !password.is_empty() => {
            secrets.set(PROXY_SECRET, &password).map(|_| ())
        }
        _ => secrets.delete(PROXY_SECRET),
    };
    if let Err(error) = kept {
        tracing::warn!(%error, "could not keep the proxy's password");
        let reason = error.to_string();
        let _ =
            tx.send(Event::Toast(Notice::alert(Msg::NoticeProxyPasswordNotKept { reason })));
    }
}

/// Where the host keeps secrets, or files under the data directory for a host with none.
fn secrets(services: &Services) -> Arc<dyn SecretStore> {
    services.host.secrets().unwrap_or_else(|| crate::host::file_secrets(&services.paths))
}

/// The config a torrent session is brought up on: `config`, with the proxy's password put in
/// its URL when the file names a user and no password and the secret store keeps one. Only
/// this copy holds it: the file never does, nor the config in effect.
pub(crate) fn with_proxy_password(config: &Config, services: &Services) -> Config {
    let wanted = config
        .providers
        .torrent
        .proxy
        .as_deref()
        .and_then(proxy::parse)
        .is_some_and(|address| address.user.is_some() && address.password.is_none());
    // The store is asked only when there is a password to put in: on a desktop each read of
    // the keychain can prompt.
    if !wanted {
        return config.clone();
    }
    put_password(config, secrets(services).as_ref())
}

/// [`with_proxy_password`], from `secrets`.
fn put_password(config: &Config, secrets: &dyn SecretStore) -> Config {
    let mut config = config.clone();
    let route = &mut config.providers.torrent.proxy;
    let Some(url) = route.as_deref() else { return config };
    match secrets.get(PROXY_SECRET) {
        Ok(Some(password)) => {
            if let Some(full) = proxy::with_password(url, &password) {
                *route = Some(full);
            }
        }
        Ok(None) => {}
        Err(error) => tracing::warn!(%error, "could not read the proxy's password"),
    }
    config
}

#[cfg(test)]
mod tests {
    use kaeri_track::secret::{Backend, LocalSecrets};

    use super::*;

    fn proxy(password: Option<&str>) -> TorrentProxy {
        TorrentProxy {
            host: " 192.0.2.10 ".into(),
            port: 1080,
            user: Some("mika".into()),
            password: password.map(|password| Secret(password.into())),
        }
    }

    fn url(config: &Config) -> Option<&str> {
        config.providers.torrent.proxy.as_deref()
    }

    #[test]
    fn a_trial_checks_the_password_given_or_else_the_one_kept() {
        let config = Config::default();
        let kept = || Some("kept".to_owned());
        let given = trial(&config, proxy(Some("lantern")), kept).unwrap();
        assert_eq!(url(&given), Some("socks5://mika:lantern@192.0.2.10:1080"));
        let left = trial(&config, proxy(None), kept).unwrap();
        assert_eq!(
            url(&left),
            Some("socks5://mika:kept@192.0.2.10:1080"),
            "left out, the kept one"
        );
        let emptied = trial(&config, proxy(Some("")), kept).unwrap();
        assert_eq!(url(&emptied), Some("socks5://mika@192.0.2.10:1080"), "given empty, none");
        assert_eq!(url(&config), None, "the config it was given is left as it is");
    }

    #[test]
    fn a_trial_without_a_proxy_address_has_nothing_to_check() {
        let nowhere = TorrentProxy { host: " ".into(), port: 1080, user: None, password: None };
        assert_eq!(trial(&Config::default(), nowhere, || None), Err(Cause::ProxyNeedsAddress));
        let portless = TorrentProxy { port: 0, ..proxy(None) };
        assert_eq!(trial(&Config::default(), portless, || None), Err(Cause::ProxyNeedsAddress));
    }

    #[test]
    fn a_session_is_brought_up_with_the_password_the_file_never_holds() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = LocalSecrets::new(dir.path(), Backend::File);
        let mut config = Config::default();
        config.providers.torrent.proxy = Some("socks5://mika@192.0.2.10:1080".into());

        // Nothing kept: the URL as the file has it.
        assert_eq!(put_password(&config, &secrets), config);

        secrets.set(PROXY_SECRET, "lantern").unwrap();
        let up = put_password(&config, &secrets);
        assert_eq!(url(&up), Some("socks5://mika:lantern@192.0.2.10:1080"));
        assert_eq!(
            url(&config),
            Some("socks5://mika@192.0.2.10:1080"),
            "the config it was given is left as it is"
        );

        // A password written into the file by hand stands.
        config.providers.torrent.proxy = Some("socks5://mika:own@192.0.2.10:1080".into());
        assert_eq!(put_password(&config, &secrets), config);
    }
}
