use std::net::SocketAddr;
use std::sync::Arc;

use bytes::{Buf as _, Bytes};
use ech_server::{BoxError, build_server_config, configured_alpns, hello_body, init_tracing};
use h3_quinn::quinn;
use rustls::pki_types::CertificateDer;

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    init_tracing();

    let mut tls_config = build_server_config(&[b"h3"])?;
    let configured_alpns = configured_alpns(&tls_config);

    // HTTP/3 allows early data on the QUIC/TLS path, which is useful to keep visible
    // while testing handshake behavior.
    tls_config.max_early_data_size = u32::MAX;
    let max_early_data_size = tls_config.max_early_data_size;

    let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(tls_config)?,
    ));

    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config
        .max_concurrent_bidi_streams(100_u32.into())
        .max_concurrent_uni_streams(100_u32.into())
        .max_idle_timeout(Some(std::time::Duration::from_secs(60).try_into()?));

    let addr: SocketAddr = "[::]:4433".parse()?;
    let endpoint = quinn::Endpoint::server(server_config, addr)?;

    tracing::info!("HTTP/3 server listening on https://{}", addr);
    tracing::info!(
        configured_alpns = ?configured_alpns,
        max_early_data_size,
        "TLS handshake debug config"
    );
    println!("Test with:");
    println!("  cargo run --bin client-h3");

    while let Some(incoming) = endpoint.accept().await {
        tokio::spawn(async move {
            if let Err(e) = handle_connection(incoming).await {
                tracing::error!("Connection error: {}", e);
            }
        });
    }

    Ok(())
}

async fn handle_connection(incoming: quinn::Incoming) -> Result<(), BoxError> {
    let conn = incoming.await?;
    let remote_addr = conn.remote_address();

    tracing::info!(
        remote_addr = %remote_addr,
        stable_id = conn.stable_id(),
        "New QUIC connection"
    );
    log_tls_handshake(&conn);

    let h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(conn)).await?;
    tokio::pin!(h3_conn);

    loop {
        match h3_conn.accept().await {
            Ok(Some(resolver)) => {
                tokio::spawn(async move {
                    if let Err(e) = handle_request(resolver).await {
                        tracing::error!("Request error: {}", e);
                    }
                });
            }
            Ok(None) => {
                tracing::info!("Connection closed by peer: {}", remote_addr);
                break;
            }
            Err(e) => {
                if e.is_h3_no_error() {
                    tracing::info!("Connection closed gracefully: {}", remote_addr);
                } else {
                    tracing::error!("H3 connection error: {:?}", e);
                }
                break;
            }
        }
    }

    Ok(())
}

fn log_tls_handshake(conn: &quinn::Connection) {
    match conn.handshake_data() {
        Some(any) => match any.downcast::<quinn::crypto::rustls::HandshakeData>() {
            Ok(handshake) => {
                let handshake = *handshake;
                let negotiated_alpn = handshake
                    .protocol
                    .as_ref()
                    .map(|proto| String::from_utf8_lossy(proto).into_owned())
                    .unwrap_or_else(|| "<none>".to_string());
                tracing::info!(
                    negotiated_alpn = %negotiated_alpn,
                    sni_hostname = handshake.server_name.as_deref().unwrap_or("<none>"),
                    "TLS handshake negotiated"
                );
            }
            Err(_) => {
                tracing::warn!("Handshake data type was not rustls::HandshakeData");
            }
        },
        None => {
            tracing::warn!("Handshake data not available on established connection");
        }
    }

    match conn.peer_identity() {
        Some(any) => match any.downcast::<Vec<CertificateDer<'static>>>() {
            Ok(certs) => {
                tracing::info!(peer_cert_chain_len = certs.len(), "TLS peer identity");
            }
            Err(_) => {
                tracing::warn!("Peer identity type was not Vec<CertificateDer<'static>>");
            }
        },
        None => {
            tracing::warn!("Peer identity not available");
        }
    }
}

async fn handle_request<C>(resolver: h3::server::RequestResolver<C, Bytes>) -> Result<(), BoxError>
where
    C: h3::quic::Connection<Bytes>,
    C::BidiStream: h3::quic::Is0rtt,
{
    let (req, mut stream) = resolver.resolve_request().await?;
    let is_0rtt = stream.is_0rtt();

    tracing::info!(
        method = %req.method(),
        path = req.uri().path(),
        is_0rtt,
        "Serving HTTP/3 request"
    );

    let response = http::Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(())?;

    stream.send_response(response).await?;
    stream
        .send_data(hello_body("h3", req.method(), req.uri().path()))
        .await?;
    stream.finish().await?;
    while let Some(mut chunk) = stream.recv_data().await? {
        chunk.advance(chunk.remaining());
    }
    let _ = stream.recv_trailers().await?;

    Ok(())
}
