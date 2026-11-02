mod check;
mod completion;
mod context;
mod definition;
mod diagnostics;
mod index;
mod parse;
mod paths;
mod platforms;
mod server;
mod watcher;

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Stderr)
        .init();

    let arguments: Vec<String> = std::env::args().collect();
    if arguments.iter().any(|argument| argument == "--version") {
        println!("unity-asmdef-lsp {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if let Some(position) = arguments.iter().position(|argument| argument == "--check") {
        let path = arguments
            .get(position + 1)
            .map(PathBuf::from)
            .unwrap_or(std::env::current_dir()?);
        if check::run(&path)? {
            std::process::exit(1);
        }
        return Ok(());
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run_server())
}

async fn run_server() -> anyhow::Result<()> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = tower_lsp::LspService::new(server::Backend::new);
    tower_lsp::Server::new(stdin, stdout, socket)
        .serve(service)
        .await;
    Ok(())
}
