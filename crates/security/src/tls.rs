//! rustls configuration for mutual TLS 1.3 with self-signed device certificates.
//!
//! The verifiers here accept *any* well-formed self-signed certificate but
//! still verify the handshake signature, so the peer must prove possession of
//! the matching private key. The resulting [`DeviceId`] is then compared
//! against the expected/trusted identity by the caller. This is the same trust
//! model as SSH host keys: identity = key, authorisation = explicit pairing.

use crate::identity::Identity;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{ring, verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{
    ClientConfig, DigitallySignedStruct, DistinguishedName, Error, ServerConfig, SignatureScheme,
};
use std::sync::Arc;

#[derive(Debug)]
struct AnyDeviceCert {
    provider: Arc<CryptoProvider>,
}

impl AnyDeviceCert {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            provider: Arc::new(ring::default_provider()),
        })
    }
}

impl ServerCertVerifier for AnyDeviceCert {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

impl ClientCertVerifier for AnyDeviceCert {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, Error> {
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// TLS 1.3-only server config requiring a client certificate.
pub fn server_config(identity: &Identity, alpn: &[u8]) -> Result<ServerConfig, Error> {
    let provider = Arc::new(ring::default_provider());
    let mut cfg = ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_client_cert_verifier(AnyDeviceCert::new())
        .with_single_cert(identity.cert_chain(), identity.private_key())?;
    cfg.alpn_protocols = vec![alpn.to_vec()];
    Ok(cfg)
}

/// TLS 1.3-only client config presenting our certificate.
pub fn client_config(identity: &Identity, alpn: &[u8]) -> Result<ClientConfig, Error> {
    let provider = Arc::new(ring::default_provider());
    let mut cfg = ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(AnyDeviceCert::new())
        .with_client_auth_cert(identity.cert_chain(), identity.private_key())?;
    cfg.alpn_protocols = vec![alpn.to_vec()];
    Ok(cfg)
}
