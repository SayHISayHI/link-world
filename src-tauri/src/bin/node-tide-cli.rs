#[tokio::main]
async fn main() {
    std::process::exit(node_tide_lib::cli::run().await);
}
