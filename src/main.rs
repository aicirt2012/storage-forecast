use std::path::PathBuf;
use clap::Parser;

extern crate lib;
use lib::forcast;

#[derive(Parser)]
#[command(name = "Storage Forcast")]
#[command(about = "Storage Forecast offers a simple command line interface to predict future storage capacity requirements. First, files on the existing file system are analysed based on their creation date and grouped into appropriate time buckets. Second, the future storage capacity usage is predicted based on the analysed history.")]
#[command(version)]
struct Cli {
    #[arg(long, short, help="Path to analyze")]
    path: PathBuf,
}

fn main() {
    let cli = Cli::parse();
    forcast(cli.path.as_path(), None);
}