use clap::Parser;
use server::{Backend, BackendOptions};
use tower_lsp::{LspService, Server};

mod server;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, help = "Add a path to search for include files")]
    include: Option<Vec<String>>,
}

#[tokio::main]
pub async fn main() {
    let args = Args::parse();
    // Log to stderr instead of stdout since communication with the language server client
    // happens through stdout
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    tracing_subscriber::fmt().init();

    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());

    let (service, socket) = LspService::new(|client| {
        Backend::new(
            client,
            BackendOptions {
                include_paths: args.include,
            },
        )
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
