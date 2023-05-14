use clap::Parser;

mod cli;
mod config;
mod dyson;
mod image;
mod notifier;
mod provider;
mod summary;
mod utils;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    cli::DysonCli::parse().run().await
}
