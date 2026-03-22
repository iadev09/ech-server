use ech_server::{BoxError, DEFAULT_H3_URL, build_reqwest_h3_client, run_simple_get};

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    run_simple_get(build_reqwest_h3_client()?, DEFAULT_H3_URL).await
}
