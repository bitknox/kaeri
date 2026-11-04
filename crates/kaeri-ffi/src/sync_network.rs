//! Pairing for sync by a typed code: the platform's DNS-SD under `_kaeri-sync._tcp`, which the
//! app implements as [`FfiSyncNetwork`] and hands over with
//! [`FfiEngine::set_sync_network`].
//!
//! A device showing a pairing code (`PairInvite`) is advertised while the code shows; a device
//! sent `SyncCommand::PairWithCode` browses for a few seconds and tries the code on each device
//! it finds. Kept apart from the remote's `_kaeri._tcp`, so a phone showing a code never lists as
//! a TV.

use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use crate::FfiEngine;
use kaeri_engine::syncing::network::{Advert, Advertiser, Browser, DiscoverySink, Found};

/// One key and value of a TXT record.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiSyncTxt {
    pub key: String,
    pub value: String,
}

/// What to register while this device shows a pairing code: the instance name (the device's
/// name, cut to 63 bytes), the port and the TXT record (`v` and `id`), in order.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiSyncAdvert {
    pub instance: String,
    pub port: u16,
    pub txt: Vec<FfiSyncTxt>,
}

/// The platform's DNS-SD for `_kaeri-sync._tcp`, implemented by the app.
///
/// `advertise` registers this device while it shows a pairing code and `withdraw` takes the
/// registration away. `browse` reports what it finds to `sink` until `stop_browsing`. Called on
/// the engine's threads.
#[uniffi::export(with_foreign)]
pub trait FfiSyncNetwork: Send + Sync {
    fn advertise(&self, advert: FfiSyncAdvert);
    fn withdraw(&self, instance: String);
    fn browse(&self, sink: Arc<FfiSyncSink>);
    fn stop_browsing(&self);
}

/// Where the app's browse reports the devices showing a code.
#[derive(uniffi::Object)]
pub struct FfiSyncSink {
    sink: DiscoverySink,
}

#[uniffi::export]
impl FfiSyncSink {
    /// A device was found, or found again with new details: its instance name, the IP addresses
    /// it resolved to, its port and its TXT record. Link-local IPv6 addresses are skipped.
    pub fn found(
        &self,
        instance: String,
        addresses: Vec<String>,
        port: u16,
        txt: Vec<FfiSyncTxt>,
    ) {
        let txt: Vec<(String, String)> = txt.into_iter().map(|p| (p.key, p.value)).collect();
        self.sink.found(Found::from_txt(&instance, usable(&addresses, port), &txt));
    }

    pub fn lost(&self, instance: String) {
        self.sink.lost(&instance);
    }
}

/// The addresses worth dialling, IPv4 first: link-local IPv6 needs a scope a socket address
/// here does not carry.
fn usable(addresses: &[String], port: u16) -> Vec<SocketAddr> {
    let mut ips: Vec<IpAddr> = addresses
        .iter()
        .filter_map(|text| text.split('%').next()?.parse().ok())
        .filter(|ip: &IpAddr| match ip {
            IpAddr::V4(_) => true,
            IpAddr::V6(v6) => !v6.is_unicast_link_local(),
        })
        .collect();
    ips.sort_by_key(|ip| u8::from(ip.is_ipv6()));
    ips.dedup();
    ips.into_iter().map(|ip| SocketAddr::new(ip, port)).collect()
}

#[uniffi::export]
impl FfiEngine {
    /// Hand sync the platform's DNS-SD for `_kaeri-sync._tcp`: once, before the first
    /// `PairInvite` or `PairWithCode`. Without it a code shows and pairs by address or QR code,
    /// and `PairWithCode` finds nobody.
    pub fn set_sync_network(&self, network: Arc<dyn FfiSyncNetwork>) {
        if let Some(this_device) = self.this_device() {
            let network = Arc::new(AppSyncNetwork(network));
            this_device.set_network(network.clone(), network);
        }
    }
}

/// The app's network, as the engine's advertiser and browser.
pub(crate) struct AppSyncNetwork(pub(crate) Arc<dyn FfiSyncNetwork>);

impl Advertiser for AppSyncNetwork {
    fn advertise(&self, advert: &Advert) {
        self.0.advertise(FfiSyncAdvert {
            instance: advert.instance.clone(),
            port: advert.port,
            txt: advert
                .txt
                .iter()
                .map(|(key, value)| FfiSyncTxt { key: key.clone(), value: value.clone() })
                .collect(),
        });
    }

    fn withdraw(&self, instance: &str) {
        self.0.withdraw(instance.to_owned());
    }
}

impl Browser for AppSyncNetwork {
    fn start(&self, sink: DiscoverySink) {
        self.0.browse(Arc::new(FfiSyncSink { sink }));
    }

    fn stop(&self) {
        self.0.stop_browsing();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_local_and_scoped_addresses_are_left_out_and_ipv4_comes_first() {
        let given = ["fe80::1%en0", "2001:db8::5", "192.0.2.7", "not an address", "192.0.2.7"];
        let given: Vec<String> = given.iter().map(|s| (*s).to_owned()).collect();
        let got = usable(&given, 47_812);
        let want: Vec<SocketAddr> =
            vec!["192.0.2.7:47812".parse().unwrap(), "[2001:db8::5]:47812".parse().unwrap()];
        assert_eq!(got, want);
    }
}
