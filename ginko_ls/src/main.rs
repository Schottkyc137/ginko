use tower_lsp::{LspService, Server};
use clap::Parser;

mod server;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {}

#[tokio::main]
pub async fn main() {
    Args::parse();
    tracing_subscriber::fmt().init();

    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());

    let (service, socket) = LspService::new(server::Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
