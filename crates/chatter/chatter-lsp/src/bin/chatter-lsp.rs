//! Standalone chatter-lsp binary.
//!
//! Serves the TalkBank Language Server Protocol over stdio. Intended to be
//! spawned by editor extensions (VS Code, etc.), not invoked directly by users.

fn main() {
    if let Err(err) = chatter_lsp::run_stdio_server() {
        eprintln!("Error: failed to start language server: {err}");
        std::process::exit(1);
    }
}
