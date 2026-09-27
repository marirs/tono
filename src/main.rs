//! Command-line entry point; the reusable engine lives in lib.rs.
mod cli;

fn main() -> anyhow::Result<()> {
    cli::run()
}
