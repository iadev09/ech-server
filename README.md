# ech-server

This repository is a small local ECH testbed for `rustls 0.24`.

Main reference:

- [rustls PR #2993](https://github.com/rustls/rustls/pull/2993)

The goal is to verify that server-side ECH works locally and that the surrounding stack still fits together:

- `rustls` with ECH enabled
- a simple HTTP/2 server path
- a QUIC + HTTP/3 server path
- a `reqwest` client using the HTTP/3 path

ECH is protocol-agnostic, so this repo uses the same TLS/ECH setup across both H2 and H3. The H3 path is the heavier integration path, while H2 is a simpler comparison path.

## Current Setup

- `server-h2`: `hyper + tokio-rustls + rustls`
- `server-h3`: `quinn + h3 + rustls`
- `client-h2`: `reqwest + h2 + rustls 0.24`
- `client-h3`: `reqwest + http3 + rustls 0.24`

The repository pins the current PR #2993 head in `Cargo.toml`, keeps a locally patched `reqwest` under `vendor/reqwest`, and uses `cargo vendor` for reproducible local builds.

## Preparation

This setup expects local test assets under `testdata/`:

- `local-ca.pem`
- `localhost-cert.pem`
- `localhost-key.pem`
- `default-ech-config.hex`
- `default-ech-config-list.hex`
- `default-ech-private-key.hex`

If `testdata/` is empty or gitignored in your checkout, generate the assets first.

Generate a local CA and `localhost` certificate:

```bash
cargo run --bin gen_test_certs
```

Generate ECH config material:

```bash
cargo run --bin gen_ech_assets
```

The current ECH generator uses:

- `config_id = 0x42`
- `public_name = "localhost"`
- `maximum_name_length = 64`

## Running

Start the HTTP/3 server:

```bash
cargo run --bin server-h3
```

Then run the HTTP/3 client in another terminal:

```bash
cargo run --bin client-h3
```

Expected output:

```text
status: 200 OK
hello over h3: GET /hello
```

You can also start the HTTP/2 server:

```bash
cargo run --bin server-h2
```

And test it with the HTTP/2 client:

```bash
cargo run --bin client-h2
```

You can also do a simple transport smoke test with curl:

```bash
curl --http2 -k https://localhost:4434/hello
```

## Interpreting Results

The reqwest clients use real ECH mode, not grease mode.

If the H3 client request succeeds and the server handles the request with ECH enabled on both sides, that is a strong indication that the ECH handshake path was accepted.

The H2 server exists as a simpler comparison path using the same TLS/ECH setup.
