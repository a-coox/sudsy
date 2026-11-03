//! The LSP server (plan.md "LSP server"): pull diagnostics, quick fixes, fix-all actions and
//! document formatting, over the same incremental database the CLI uses. One server process keeps
//! one database and, through the C# provider, one long-lived worker.
//!
//! * [`workspace`]: the state (config, open documents) and its use by the handlers.
//! * `diagnostics`, `actions`, `formatting`: the handlers, as methods of [`Workspace`].
//! * `server`: initialize, the message loop and the translation of LSP messages.

mod actions;
mod diagnostics;
mod formatting;
mod position;
mod server;
pub mod workspace;

pub use actions::FIX_ALL_KIND;
pub use server::serve;
pub use workspace::Workspace;

use anyhow::Result;
use lsp_server::Connection;

/// Runs the server on stdin and stdout until the client shuts it down.
pub fn run_stdio() -> Result<()> {
    let (connection, io_threads) = Connection::stdio();
    serve(connection, languages::providers())?;
    io_threads.join()?;
    Ok(())
}
