mod completion;
mod context;
mod diagnostics;
mod index;
mod parse;
mod paths;
mod platforms;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Stderr)
        .init();

    if std::env::args().any(|argument| argument == "--version") {
        println!("unity-asmdef-lsp {}", env!("CARGO_PKG_VERSION"));
    }
}
