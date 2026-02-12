use std::io::Write;

use clap::Parser;
use tokio::io;
use tokio::io::{BufReader, AsyncBufReadExt};
use tokio::net::TcpStream;

use env_logger::Env;
use log;

use rcon_rs::RconClient;
use rcon_rs::errors::RconError;

#[derive(Parser)]
struct Args {
    /// Server Address (eg: 127.0.0.1:27015, or localhost)
    #[arg(short, long)]
    address: String,

    /// Server password
    #[arg(short, long)]
    password: String,

    /// The command to execute. This can be any string
    #[arg(short, long)]
    command: Option<String>
}

async fn run_cli(mut client: RconClient<TcpStream>) -> Result<(), RconError> {
    log::info!("Connected!");

    let stdin = BufReader::new(io::stdin());
    let mut lines = stdin.lines();

    print!("> ");
    std::io::stdout().flush().unwrap();

    while let Some(line) = lines.next_line().await? {
        let resp = client.exec(&line).await?;

        if !resp.is_empty() {
            log::info!("Response: {:?}", resp);
        }

        print!("> ");
        std::io::stdout().flush().unwrap();
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), RconError> {
    let args = Args::parse();

    env_logger::Builder::from_env(
        Env::default()
            .filter_or("RUST_LOG", "info")
    ).init();

    let mut client = RconClient::connect(
        format!("{}", &args.address)).await?;
    client.auth(&args.password).await?;

    if args.command.is_some() {
        let cmd = args.command.unwrap();
        let response = client.exec(&cmd).await?;
        println!("{}", response);
        return Ok(())
    } else {
        run_cli(client).await?;
    }
    Ok(())
}
