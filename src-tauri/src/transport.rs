use crate::models::{ClipboardPayloadKind, TransferEntry};
use crate::runtime::RelayRuntime;
use crate::transfers::{PreparedTransfer, PreparedTransferEntry, TRANSFER_CHUNK_SIZE};
use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use prost::Message;
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, DigitallySignedStruct, Error as RustlsError, ServerConfig, SignatureScheme,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};

const MAX_FRAME_SIZE: usize = 12 * 1024 * 1024;
const CHANNEL_CLIPBOARD: u8 = 1;
const CHANNEL_TRANSFER: u8 = 2;

const TRANSFER_FRAME_OFFER: u8 = 1;
const TRANSFER_FRAME_ACCEPT: u8 = 2;
const TRANSFER_FRAME_REJECT: u8 = 3;
const TRANSFER_FRAME_FILE_START: u8 = 4;
const TRANSFER_FRAME_FILE_CHUNK: u8 = 5;
const TRANSFER_FRAME_COMPLETE: u8 = 6;
const TRANSFER_FRAME_CANCEL: u8 = 7;
const TRANSFER_FRAME_ACK: u8 = 8;

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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingTransferOffer {
    pub transfer_id: String,
    pub origin_device_id: String,
    pub target_device_id: String,
    pub display_name: String,
    pub total_bytes: u64,
    pub total_entries: u32,
    pub entries: Vec<TransferEntry>,
    pub top_level_names: Vec<String>,
    pub warning_message: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransferDecision {
    accepted: bool,
    reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileStartFrame {
    relative_path: String,
    size: u64,
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

    write_channel(&mut tls_stream, CHANNEL_CLIPBOARD).await?;
    write_envelope(&mut tls_stream, &envelope).await?;
    tls_stream.shutdown().await.ok();
    Ok(())
}

pub async fn send_transfer(
    runtime: RelayRuntime,
    address: std::net::SocketAddr,
    expected_fingerprint: &str,
    offer: IncomingTransferOffer,
    prepared: PreparedTransfer,
    cancel_flag: Arc<AtomicBool>,
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

    write_channel(&mut tls_stream, CHANNEL_TRANSFER).await?;
    write_transfer_json(&mut tls_stream, TRANSFER_FRAME_OFFER, &offer).await?;
    let (frame_type, frame_payload) = read_transfer_frame(&mut tls_stream).await?;
    match frame_type {
        TRANSFER_FRAME_ACCEPT => {
            let decision: TransferDecision = serde_json::from_slice(&frame_payload)?;
            if !decision.accepted {
                bail!(
                    "{}",
                    decision.reason.unwrap_or_else(|| "transfer was rejected".to_string())
                );
            }
        }
        TRANSFER_FRAME_REJECT => {
            let decision: TransferDecision = serde_json::from_slice(&frame_payload)?;
            bail!(
                "{}",
                decision.reason.unwrap_or_else(|| "transfer was rejected".to_string())
            );
        }
        _ => bail!("unexpected transfer response frame"),
    }

    for entry in prepared.file_entries() {
        if cancel_flag.load(Ordering::SeqCst) {
            write_transfer_json(
                &mut tls_stream,
                TRANSFER_FRAME_CANCEL,
                &TransferDecision {
                    accepted: false,
                    reason: Some("canceled".to_string()),
                },
            )
            .await
            .ok();
            bail!("transfer canceled");
        }
        send_file_entry(&runtime, &offer.transfer_id, &mut tls_stream, entry, cancel_flag.clone())
            .await?;
    }

    write_transfer_json(
        &mut tls_stream,
        TRANSFER_FRAME_COMPLETE,
        &TransferDecision {
            accepted: true,
            reason: None,
        },
    )
    .await?;
    let (frame_type, payload) = read_transfer_frame(&mut tls_stream).await?;
    match frame_type {
        TRANSFER_FRAME_ACK => Ok(()),
        TRANSFER_FRAME_REJECT | TRANSFER_FRAME_CANCEL => {
            let decision: TransferDecision = serde_json::from_slice(&payload)?;
            bail!(
                "{}",
                decision
                    .reason
                    .unwrap_or_else(|| "receiver rejected the transfer".to_string())
            )
        }
        _ => bail!("unexpected transfer completion frame"),
    }
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
    match read_channel(&mut tls_stream).await? {
        CHANNEL_CLIPBOARD => loop {
            match read_envelope(&mut tls_stream).await? {
                Some(envelope) => runtime.handle_incoming_envelope(envelope).await?,
                None => break,
            }
        },
        CHANNEL_TRANSFER => accept_transfer(runtime, &mut tls_stream).await?,
        _ => bail!("unsupported relay channel"),
    }
    Ok(())
}

async fn accept_transfer(
    runtime: RelayRuntime,
    stream: &mut tokio_rustls::server::TlsStream<TcpStream>,
) -> Result<()> {
    let (frame_type, payload) = read_transfer_frame(stream).await?;
    if frame_type != TRANSFER_FRAME_OFFER {
        bail!("transfer stream did not start with an offer");
    }

    let offer: IncomingTransferOffer = serde_json::from_slice(&payload)?;
    let prep = match runtime.prepare_incoming_transfer(&offer).await {
        Ok(prep) => prep,
        Err(error) => {
            write_transfer_json(
                stream,
                TRANSFER_FRAME_REJECT,
                &TransferDecision {
                    accepted: false,
                    reason: Some(error.to_string()),
                },
            )
            .await
            .ok();
            return Err(error);
        }
    };

    write_transfer_json(
        stream,
        TRANSFER_FRAME_ACCEPT,
        &TransferDecision {
            accepted: true,
            reason: None,
        },
    )
    .await?;

    let result = receive_transfer_payload(runtime.clone(), &offer, &prep.staging_root, stream).await;
    drop(prep.lease);
    result
}

async fn receive_transfer_payload(
    runtime: RelayRuntime,
    offer: &IncomingTransferOffer,
    staging_root: &Path,
    stream: &mut tokio_rustls::server::TlsStream<TcpStream>,
) -> Result<()> {
    let payload_root = crate::transfers::transfer_payload_dir(staging_root);
    let mut current_file: Option<(String, u64, u64, File)> = None;

    loop {
        let (frame_type, payload) = read_transfer_frame(stream).await?;
        match frame_type {
            TRANSFER_FRAME_FILE_START => {
                if let Some((_, _, _, file)) = current_file.take() {
                    drop(file);
                }
                let frame: FileStartFrame = serde_json::from_slice(&payload)?;
                let file_path = payload_root.join(native_relative_path(&frame.relative_path));
                if let Some(parent) = file_path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                let file = File::create(&file_path)
                    .await
                    .with_context(|| format!("failed to create {}", file_path.display()))?;
                current_file = Some((frame.relative_path, frame.size, 0, file));
            }
            TRANSFER_FRAME_FILE_CHUNK => {
                let Some((_, size, written, file)) = current_file.as_mut() else {
                    bail!("received a file chunk without a file start frame");
                };
                file.write_all(&payload).await?;
                *written = written.saturating_add(payload.len() as u64);
                runtime.increment_transfer_progress(&offer.transfer_id, payload.len() as u64, 0)?;
                if *written == *size {
                    file.flush().await?;
                    runtime.increment_transfer_progress(&offer.transfer_id, 0, 1)?;
                    current_file = None;
                }
            }
            TRANSFER_FRAME_COMPLETE => {
                runtime.complete_incoming_transfer(&offer.transfer_id)?;
                write_transfer_json(
                    stream,
                    TRANSFER_FRAME_ACK,
                    &TransferDecision {
                        accepted: true,
                        reason: None,
                    },
                )
                .await?;
                return Ok(());
            }
            TRANSFER_FRAME_CANCEL => {
                runtime.cancel_transfer(&offer.transfer_id)?;
                write_transfer_json(
                    stream,
                    TRANSFER_FRAME_CANCEL,
                    &TransferDecision {
                        accepted: false,
                        reason: Some("transfer canceled".to_string()),
                    },
                )
                .await
                .ok();
                bail!("transfer canceled");
            }
            _ => bail!("unexpected transfer frame"),
        }
    }
}

async fn send_file_entry<S>(
    runtime: &RelayRuntime,
    transfer_id: &str,
    stream: &mut S,
    entry: &PreparedTransferEntry,
    cancel_flag: Arc<AtomicBool>,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let relative_path = entry.entry.relative_path.clone();
    let source_path = entry
        .source_path
        .as_ref()
        .ok_or_else(|| anyhow!("file entry is missing a source path"))?;
    write_transfer_json(
        stream,
        TRANSFER_FRAME_FILE_START,
        &FileStartFrame {
            relative_path,
            size: entry.entry.size,
        },
    )
    .await?;

    let mut file = File::open(source_path)
        .await
        .with_context(|| format!("failed to open {}", source_path.display()))?;
    let mut buffer = vec![0_u8; TRANSFER_CHUNK_SIZE];
    loop {
        if cancel_flag.load(Ordering::SeqCst) {
            bail!("transfer canceled");
        }

        let bytes_read = file.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }

        write_transfer_bytes(stream, TRANSFER_FRAME_FILE_CHUNK, &buffer[..bytes_read]).await?;
        runtime.increment_transfer_progress(transfer_id, bytes_read as u64, 0)?;
    }

    runtime.increment_transfer_progress(transfer_id, 0, 1)?;
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

async fn write_channel<S>(stream: &mut S, channel: u8) -> Result<()>
where
    S: AsyncWrite + Unpin,
{
    stream.write_u8(channel).await?;
    stream.flush().await?;
    Ok(())
}

async fn read_channel<S>(stream: &mut S) -> Result<u8>
where
    S: AsyncRead + Unpin,
{
    Ok(stream.read_u8().await?)
}

async fn write_transfer_json<S, T>(stream: &mut S, frame_type: u8, value: &T) -> Result<()>
where
    S: AsyncWrite + Unpin,
    T: Serialize,
{
    let payload = serde_json::to_vec(value)?;
    write_transfer_bytes(stream, frame_type, &payload).await
}

async fn write_transfer_bytes<S>(stream: &mut S, frame_type: u8, payload: &[u8]) -> Result<()>
where
    S: AsyncWrite + Unpin,
{
    if payload.len() > u32::MAX as usize {
        bail!("transfer frame exceeded the maximum supported size");
    }

    stream.write_u8(frame_type).await?;
    stream
        .write_all(&(payload.len() as u32).to_be_bytes())
        .await?;
    stream.write_all(payload).await?;
    stream.flush().await?;
    Ok(())
}

async fn read_transfer_frame<S>(stream: &mut S) -> Result<(u8, Vec<u8>)>
where
    S: AsyncRead + Unpin,
{
    let frame_type = stream.read_u8().await?;
    let payload_len = stream.read_u32().await? as usize;
    let mut payload = vec![0_u8; payload_len];
    stream.read_exact(&mut payload).await?;
    Ok((frame_type, payload))
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

fn native_relative_path(relative_path: &str) -> PathBuf {
    relative_path
        .split('/')
        .fold(PathBuf::new(), |path, segment| path.join(segment))
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
