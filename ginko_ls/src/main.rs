use tower_lsp::{LspService, Server};

mod server;
#[tokio::main]
pub async fn main() {
    tracing_subscriber::fmt().init();

    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());

    let (service, socket) = LspService::new(server::Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
