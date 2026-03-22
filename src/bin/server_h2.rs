use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use ech_server::{BoxError, build_server_config, configured_alpns, hello_body, init_tracing};
use http::{Request, Response, StatusCode};
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http2::Builder;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    init_tracing();

    let tls_config = build_server_config(&[b"h2"])?;
    let configured_alpns = configured_alpns(&tls_config);
    let tls_acceptor = TlsAcceptor::from(Arc::new(tls_config));

    let addr: SocketAddr = "[::]:4434".parse()?;
    let listener = TcpListener::bind(addr).await?;

    tracing::info!("HTTP/2 server listening on https://{}", addr);
    tracing::info!(
        configured_alpns = ?configured_alpns,
        "TLS handshake debug config"
    );
    println!("Test with:");
    println!("  cargo run --bin client-h2");

    loop {
        let (tcp_stream, remote_addr) = listener.accept().await?;
        let tls_acceptor = tls_acceptor.clone();

        tokio::spawn(async move {
            let tls_stream = match tls_acceptor.accept(tcp_stream).await {
                Ok(tls_stream) => tls_stream,
                Err(err) => {
                    tracing::error!(%remote_addr, "TLS accept failed: {err}");
                    return;
                }
            };

            let (_, server_conn) = tls_stream.get_ref();
            let negotiated_alpn = server_conn
                .alpn_protocol()
                .map(|proto| String::from_utf8_lossy(proto.as_ref()).into_owned())
                .unwrap_or_else(|| "<none>".to_string());
            let sni_hostname = server_conn
                .server_name()
                .map(|name| name.as_ref().to_string())
                .unwrap_or_else(|| "<none>".to_string());

            tracing::info!(
                %remote_addr,
                negotiated_alpn = %negotiated_alpn,
                sni_hostname = %sni_hostname,
                ech_status = ?server_conn.ech_status(),
                "TLS handshake negotiated"
            );

            let mut builder = Builder::new(TokioExecutor::new());
            builder.max_concurrent_streams(Some(100));

            if let Err(err) = builder
                .serve_connection(TokioIo::new(tls_stream), service_fn(handle_request))
                .await
            {
                tracing::error!(%remote_addr, "HTTP/2 connection failed: {err}");
            }
        });
    }
}

async fn handle_request(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    tracing::info!(
        method = %req.method(),
        path = req.uri().path(),
        "Serving HTTP/2 request"
    );

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Full::new(hello_body("h2", req.method(), req.uri().path())))
        .expect("static response is valid");

    Ok(response)
}
