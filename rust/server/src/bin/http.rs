#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    server::http_runtime::run_http_server().await
}
