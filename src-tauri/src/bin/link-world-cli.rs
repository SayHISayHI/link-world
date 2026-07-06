#[tokio::main]
async fn main() {
    std::process::exit(link_world_lib::cli::run().await);
}
