#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    web::http_runtime::run_http_server().await
}
