use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use reqwest::Url;
use rustls::client::{EchConfig, EchMode};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, EchConfigListBytes, PrivateKeyDer};
use rustls::{ClientConfig, RootCertStore};
use rustls::server::{EchServerKey, FixedEchKeys};
use rustls_aws_lc_rs::hpke::ALL_SUPPORTED_SUITES;

pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub const DEFAULT_H3_URL: &str = "https://localhost:4433/hello";
pub const DEFAULT_H2_URL: &str = "https://localhost:4434/hello";
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
}

pub fn decode_hex(input: &str) -> Result<Vec<u8>, BoxError> {
    let input = input.trim();
    if input.len() % 2 != 0 {
        return Err("hex input has odd length".into());
    }

    let mut out = Vec::with_capacity(input.len() / 2);
    for i in (0..input.len()).step_by(2) {
        out.push(u8::from_str_radix(&input[i..i + 2], 16)?);
    }
    Ok(out)
}

pub fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn testdata_dir() -> PathBuf {
    manifest_dir().join("testdata")
}

pub fn ensure_testdata_dir() -> Result<PathBuf, BoxError> {
    let dir = testdata_dir();
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn testdata_path(name: &str) -> PathBuf {
    testdata_dir().join(name)
}

pub fn read_testdata_string(name: &str) -> Result<String, BoxError> {
    let path = testdata_path(name);
    read_required_string(
        &path,
        &format!("missing {}; generate test assets first", path.display()),
    )
}

pub fn read_required_string(path: &Path, missing_hint: &str) -> Result<String, BoxError> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(missing_hint.to_string().into()),
        Err(err) => Err(err.into()),
    }
}

pub fn build_client_tls_config() -> Result<ClientConfig, BoxError> {
    let ech_bytes = decode_hex(&read_testdata_string("default-ech-config-list.hex")?)?;
    let ech_config = EchConfig::new(EchConfigListBytes::from(ech_bytes), ALL_SUPPORTED_SUITES)
        .map_err(|_| "default ECH config is not supported by this provider")?;

    let mut roots = RootCertStore::empty();
    let ca_pem = read_testdata_string("local-ca.pem")?;
    roots.add(CertificateDer::from_pem_slice(ca_pem.as_bytes())?)?;

    Ok(
        ClientConfig::builder(rustls_aws_lc_rs::DEFAULT_TLS13_PROVIDER.into())
            .with_ech(EchMode::from(ech_config))
            .with_root_certificates(roots)
            .with_no_client_auth()?,
    )
}

pub fn build_reqwest_h3_client() -> Result<reqwest::Client, BoxError> {
    Ok(reqwest::Client::builder()
        .tls_backend_preconfigured(build_client_tls_config()?)
        .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
        .timeout(DEFAULT_REQUEST_TIMEOUT)
        .http3_prior_knowledge()
        .https_only(true)
        .build()?)
}

pub fn build_reqwest_h2_client() -> Result<reqwest::Client, BoxError> {
    Ok(reqwest::Client::builder()
        .tls_backend_preconfigured(build_client_tls_config()?)
        .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
        .timeout(DEFAULT_REQUEST_TIMEOUT)
        .http2_prior_knowledge()
        .https_only(true)
        .build()?)
}

pub async fn run_simple_get(client: reqwest::Client, url: &str) -> Result<(), BoxError> {
    let url = Url::parse(url)?;
    let response = client.get(url).send().await?;
    let status = response.status();
    let body = response.text().await?;

    println!("status: {status}");
    println!("{body}");

    Ok(())
}

pub fn default_ech_server_key() -> Result<EchServerKey, BoxError> {
    let config = decode_hex(&read_testdata_string("default-ech-config.hex")?)?;
    let private_key = decode_hex(&read_testdata_string("default-ech-private-key.hex")?)?;
    Ok(EchServerKey::from_raw(
        &config,
        private_key,
        ALL_SUPPORTED_SUITES,
    )?)
}

pub fn build_server_config(alpns: &[&[u8]]) -> Result<rustls::ServerConfig, BoxError> {
    let cert_pem = read_testdata_string("localhost-cert.pem")?;
    let key_pem = read_testdata_string("localhost-key.pem")?;
    let cert = CertificateDer::from_pem_slice(cert_pem.as_bytes())?;
    let key = PrivateKeyDer::from_pem_slice(key_pem.as_bytes())?;
    let identity = Arc::new(rustls::crypto::Identity::from_cert_chain(vec![cert])?);

    let mut tls_config = rustls::ServerConfig::builder(Arc::new(
        rustls_aws_lc_rs::DEFAULT_PROVIDER,
    ))
    .with_no_client_auth()
    .with_single_cert(identity, key.into())?;

    tls_config.ech_keys = Arc::new(FixedEchKeys::new(vec![default_ech_server_key()?]));
    tls_config.alpn_protocols = alpns
        .iter()
        .map(|alpn| (*alpn).to_vec().into())
        .collect();

    Ok(tls_config)
}

pub fn configured_alpns(tls_config: &rustls::ServerConfig) -> Vec<String> {
    tls_config
        .alpn_protocols
        .iter()
        .map(|proto| String::from_utf8_lossy(proto.as_ref()).into_owned())
        .collect()
}

pub fn hello_body(protocol: &str, method: &http::Method, path: &str) -> Bytes {
    Bytes::from(format!("hello over {protocol}: {method} {path}\n"))
}
