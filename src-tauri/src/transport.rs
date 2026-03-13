use crate::models::ClipboardPayloadKind;
use crate::runtime::RelayRuntime;
use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use prost::Message;
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, DigitallySignedStruct, Error as RustlsError, ServerConfig, SignatureScheme,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};

const MAX_FRAME_SIZE: usize = 12 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum PayloadKind {
    Unknown = 0,
    Text = 1,
    Image = 2,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ClipboardEnvelope {
    #[prost(string, tag = "1")]
    pub origin_device_id: String,
    #[prost(string, tag = "2")]
    pub target_device_id: String,
    #[prost(string, tag = "3")]
    pub content_hash: String,
    #[prost(enumeration = "PayloadKind", tag = "4")]
    pub payload_kind: i32,
    #[prost(bytes = "vec", tag = "5")]
    pub payload_bytes: Vec<u8>,
    #[prost(int64, tag = "6")]
    pub created_at: i64,
    #[prost(uint64, tag = "7")]
    pub sequence: u64,
    #[prost(string, tag = "8")]
    pub mime: String,
}

pub async fn start_server(
    runtime: RelayRuntime,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
) -> Result<u16> {
    let acceptor = TlsAcceptor::from(Arc::new(build_server_config(cert_der, key_der)?));
    let listener = TcpListener::bind(("0.0.0.0", 0))
        .await
        .context("failed to bind the relay tcp listener")?;
    let port = listener.local_addr()?.port();
    let relay = runtime.clone();

    tauri::async_runtime::spawn(async move {
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(result) => result,
                Err(error) => {
                    relay.emit_clipboard_error(format!("Transport listener failed: {error}"));
                    break;
                }
            };

            let acceptor = acceptor.clone();
            let runtime = relay.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = accept_connection(runtime.clone(), acceptor, stream).await {
                    runtime.emit_clipboard_error(format!("Inbound sync failed: {error}"));
                }
            });
        }
    });

    Ok(port)
}

pub async fn send_envelope(
    address: std::net::SocketAddr,
    expected_fingerprint: &str,
    envelope: ClipboardEnvelope,
) -> Result<()> {
    let connector = TlsConnector::from(Arc::new(build_client_config()));
    let stream = TcpStream::connect(address)
        .await
        .with_context(|| format!("failed to connect to {address}"))?;
    let server_name =
        ServerName::try_from("relayclip.local").map_err(|_| anyhow!("invalid tls server name"))?;
    let mut tls_stream = connector
        .connect(server_name, stream)
        .await
        .context("failed to complete the tls handshake")?;

    let fingerprint = peer_fingerprint(&tls_stream)?;
    if fingerprint != expected_fingerprint {
        bail!("peer fingerprint mismatch");
    }

    write_envelope(&mut tls_stream, &envelope).await?;
    tls_stream.shutdown().await.ok();
    Ok(())
}

pub fn fingerprint_from_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub fn generate_self_signed_identity(
    device_name: &str,
    device_id: &str,
) -> Result<(Vec<u8>, Vec<u8>, String)> {
    let mut distinguished_name = DistinguishedName::new();
    distinguished_name.push(DnType::CommonName, device_name);

    let mut params = CertificateParams::new(vec![format!("relayclip-{device_id}.local")])?;
    params.distinguished_name = distinguished_name;
    let key_pair = KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;
    let cert_der = cert.der().to_vec();
    let key_der = key_pair.serialize_der();
    let fingerprint = fingerprint_from_bytes(&cert_der);
    Ok((cert_der, key_der, fingerprint))
}

pub fn kind_from_i32(value: i32) -> Result<ClipboardPayloadKind> {
    match PayloadKind::try_from(value).unwrap_or(PayloadKind::Unknown) {
        PayloadKind::Text => Ok(ClipboardPayloadKind::Text),
        PayloadKind::Image => Ok(ClipboardPayloadKind::Image),
        PayloadKind::Unknown => bail!("unsupported payload kind"),
    }
}

pub fn envelope_from_packet(
    origin_device_id: String,
    target_device_id: String,
    packet: &crate::clipboard::ClipboardPacket,
    sequence: u64,
) -> ClipboardEnvelope {
    ClipboardEnvelope {
        origin_device_id,
        target_device_id,
        content_hash: packet.meta.hash.clone(),
        payload_kind: packet.meta.kind.as_transport_kind(),
        payload_bytes: packet.bytes.clone(),
        created_at: Utc::now().timestamp_millis(),
        sequence,
        mime: packet.meta.mime.clone(),
    }
}

async fn accept_connection(
    runtime: RelayRuntime,
    acceptor: TlsAcceptor,
    stream: TcpStream,
) -> Result<()> {
    let mut tls_stream = acceptor.accept(stream).await.context("tls accept failed")?;

    loop {
        match read_envelope(&mut tls_stream).await? {
            Some(envelope) => runtime.handle_incoming_envelope(envelope).await?,
            None => break,
        }
    }

    Ok(())
}

fn build_server_config(cert_der: Vec<u8>, key_der: Vec<u8>) -> Result<ServerConfig> {
    let cert_chain = vec![CertificateDer::from(cert_der)];
    let key = PrivatePkcs8KeyDer::from(key_der);
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key.into())
        .context("failed to build the rustls server config")?;
    Ok(config)
}

fn build_client_config() -> ClientConfig {
    ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyServerCertVerifier))
        .with_no_client_auth()
}

async fn read_envelope<S>(stream: &mut S) -> Result<Option<ClipboardEnvelope>>
where
    S: AsyncRead + Unpin,
{
    let mut len_buf = [0_u8; 4];
    match stream.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error.into()),
    }

    let frame_len = u32::from_be_bytes(len_buf) as usize;
    if frame_len > MAX_FRAME_SIZE {
        bail!("incoming frame exceeded the maximum allowed size");
    }

    let mut frame = vec![0_u8; frame_len];
    stream.read_exact(&mut frame).await?;
    Ok(Some(ClipboardEnvelope::decode(frame.as_slice())?))
}

async fn write_envelope<S>(stream: &mut S, envelope: &ClipboardEnvelope) -> Result<()>
where
    S: AsyncWrite + Unpin,
{
    let mut frame = Vec::with_capacity(envelope.encoded_len());
    envelope.encode(&mut frame)?;
    if frame.len() > MAX_FRAME_SIZE {
        bail!("outgoing frame exceeded the maximum allowed size");
    }

    stream
        .write_all(&(frame.len() as u32).to_be_bytes())
        .await?;
    stream.write_all(&frame).await?;
    stream.flush().await?;
    Ok(())
}

fn peer_fingerprint(stream: &tokio_rustls::client::TlsStream<TcpStream>) -> Result<String> {
    let (_, connection) = stream.get_ref();
    let certificates = connection
        .peer_certificates()
        .ok_or_else(|| anyhow!("missing peer certificate"))?;
    let certificate = certificates
        .first()
        .ok_or_else(|| anyhow!("empty peer certificate chain"))?;
    Ok(fingerprint_from_bytes(certificate.as_ref()))
}

#[derive(Debug)]
struct AcceptAnyServerCertVerifier;

impl ServerCertVerifier for AcceptAnyServerCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, RustlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ED25519,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
        ]
    }
}
