use ech_server::{BoxError, DEFAULT_H2_URL, build_reqwest_h2_client, run_simple_get};

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    run_simple_get(build_reqwest_h2_client()?, DEFAULT_H2_URL).await
}
