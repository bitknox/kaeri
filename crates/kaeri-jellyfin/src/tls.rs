//! Certificates a server at home uses that this device does not trust.
//!
//! A server with a self-signed certificate is normal at home. Check this server shows its
//! SHA-256 fingerprint, and Allow this certificate pins that fingerprint to the server: from then
//! on a client for it accepts exactly that certificate, as well as any the public roots vouch
//! for. The handshake's signatures are always checked, so a pinned certificate still has to be
//! held by the server that shows it.

use std::sync::{Arc, Mutex};

use rustls::{
    ClientConfig, DigitallySignedStruct, SignatureScheme,
    client::{
        WebPkiServerVerifier,
        danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    },
    crypto::CryptoProvider,
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use sha2::{Digest, Sha256};

use crate::{address::Address, error::JellyfinError};

/// How long a request through a pinned client may take.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// A certificate's SHA-256 fingerprint: upper-case hex, no separators.
pub fn fingerprint(der: &[u8]) -> String {
    Sha256::digest(der).iter().map(|b| format!("{b:02X}")).collect()
}

/// A fingerprint as the step shows it, in groups of four.
pub fn groups(fingerprint: &str) -> Vec<String> {
    let clean = normal(fingerprint);
    clean.as_bytes().chunks(4).map(|c| String::from_utf8_lossy(c).into_owned()).collect()
}

/// A fingerprint however it was written (lower case, with colons or spaces) as [`fingerprint`]
/// writes it.
pub fn normal(fingerprint: &str) -> String {
    fingerprint
        .chars()
        .filter(char::is_ascii_hexdigit)
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn config(verifier: Arc<dyn ServerCertVerifier>) -> Result<ClientConfig, JellyfinError> {
    let config = ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(|e| JellyfinError::Decode(e.to_string()))?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    Ok(config)
}

fn client(config: ClientConfig) -> Result<reqwest::Client, JellyfinError> {
    reqwest::Client::builder()
        .tls_backend_preconfigured(config)
        .timeout(TIMEOUT)
        .user_agent(concat!("kaeri/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| JellyfinError::Decode(e.to_string()))
}

/// A client that trusts the certificate with `fingerprint`, and anything the public roots trust.
pub fn pinned_client(fingerprint: &str) -> Result<reqwest::Client, JellyfinError> {
    client(config(Arc::new(Pinned::new(fingerprint)?))?)
}

/// The fingerprint of the certificate the server at `address` shows. Only the public endpoint is
/// asked, with no sign-in, so nothing private is sent before the certificate is allowed.
pub async fn peer_fingerprint(address: &Address) -> Result<String, JellyfinError> {
    let seen = Arc::new(Recorder { provider: provider(), seen: Mutex::default() });
    let probe = client(config(seen.clone())?)?;
    let _ = probe.get(address.url("/System/Info/Public")).send().await;
    let found = seen.seen.lock().unwrap_or_else(|e| e.into_inner()).clone();
    found.ok_or_else(|| JellyfinError::Unreachable { address: address.display().to_owned() })
}

fn signatures(
    provider: &CryptoProvider,
    tls13: bool,
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
) -> Result<HandshakeSignatureValid, rustls::Error> {
    let algorithms = &provider.signature_verification_algorithms;
    if tls13 {
        rustls::crypto::verify_tls13_signature(message, cert, dss, algorithms)
    } else {
        rustls::crypto::verify_tls12_signature(message, cert, dss, algorithms)
    }
}

/// Accepts the pinned certificate, and otherwise what the public roots accept.
#[derive(Debug)]
struct Pinned {
    fingerprint: String,
    provider: Arc<CryptoProvider>,
    roots: Arc<WebPkiServerVerifier>,
}

impl Pinned {
    fn new(fingerprint: &str) -> Result<Self, JellyfinError> {
        let provider = provider();
        let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
        let roots =
            WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider.clone())
                .build()
                .map_err(|e| JellyfinError::Decode(e.to_string()))?;
        Ok(Self { fingerprint: normal(fingerprint), provider, roots })
    }
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if fingerprint(end_entity) == self.fingerprint {
            return Ok(ServerCertVerified::assertion());
        }
        self.roots.verify_server_cert(end_entity, intermediates, server_name, ocsp, now)
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        signatures(&self.provider, false, message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        signatures(&self.provider, true, message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// Notes the certificate shown, for [`peer_fingerprint`]. It accepts any certificate, so it is
/// only used for the one public request that reads the fingerprint.
#[derive(Debug)]
struct Recorder {
    provider: Arc<CryptoProvider>,
    seen: Mutex<Option<String>>,
}

impl ServerCertVerifier for Recorder {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        *self.seen.lock().unwrap_or_else(|e| e.into_inner()) = Some(fingerprint(end_entity));
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        signatures(&self.provider, false, message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        signatures(&self.provider, true, message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fingerprint_is_shown_in_sixteen_groups_of_four() {
        let print = fingerprint(b"a certificate");
        assert_eq!(print.len(), 64);
        let shown = groups(&print);
        assert_eq!(shown.len(), 16);
        assert!(shown.iter().all(|g| g.len() == 4));
        assert_eq!(shown.concat(), print);
        assert_eq!(groups("ab:cd:ef:01"), ["ABCD", "EF01"], "however it was written");
    }

    #[test]
    fn only_the_pinned_certificate_is_accepted_outside_the_public_roots() {
        let pinned = CertificateDer::from(b"the server's own".to_vec());
        let other = CertificateDer::from(b"someone else's".to_vec());
        let verifier = Pinned::new(&fingerprint(&pinned).to_lowercase()).unwrap();
        let name = ServerName::try_from("jellyfin.mika.example").unwrap();
        let now = UnixTime::now();
        assert!(verifier.verify_server_cert(&pinned, &[], &name, &[], now).is_ok());
        assert!(verifier.verify_server_cert(&other, &[], &name, &[], now).is_err());
    }

    #[test]
    fn a_pinned_client_builds() {
        assert!(pinned_client(&fingerprint(b"x")).is_ok());
    }
}
