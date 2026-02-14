use std::io::Write;

use clap::Parser;
use tokio::net::TcpStream;

use env_logger::Env;
use log;
use rpassword::read_password;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use rcon_tokio::RconClient;
use rcon_tokio::errors::RconError;

#[derive(Parser)]
struct Args {
    /// Server Address (eg: 127.0.0.1:27015, or localhost)
    #[arg(short, long)]
    address: Option<String>,

    /// Server password
    #[arg(short, long)]
    password: Option<String>,

    /// The command to execute. This can be any string
    #[arg(short, long)]
    command: Option<String>,

    show_responses: Option<bool>,
}

async fn run_cli(mut client: RconClient<TcpStream>, show_responses: bool) -> rustyline::Result<()> {
    log::info!("Connected!");

    let mut rl = DefaultEditor::new()?;

    if rl.load_history("history.txt").is_err() {
        log::info!("No previous history.");
    }

    loop {
        let readline = rl.readline("> ");
        match readline {
            Ok(line) => {
                let _ = rl.add_history_entry(line.as_str());
                let resp = client.exec(&line).await.unwrap_or_else(|e| format!("Error: {}", e));
                
                if show_responses {
                    log::info!("Response: {:?}", resp);
                }
            },
            Err(ReadlineError::Interrupted) => {
                log::info!("CTRL-C");
                break;
            },
            Err(ReadlineError::Eof) => {
                log::info!("CTRL-D");
                break;
            },
            Err(err) => {
                log::error!("Error: {:?}", err);
                break;
            }
        }
    }

    rl.save_history("history.txt").unwrap_or_else(|e| log::error!("Failed to save history: {}", e));
    Ok(())
}

fn get_address(provided_addr: &Option<String>) -> String {
    if provided_addr.is_some() {
        return provided_addr.clone().unwrap().to_string();
    }
    print!("Enter address: ");
    std::io::stdout().flush().unwrap();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

fn get_password(provided_pw: &Option<String>) -> String {
    if provided_pw.is_some() {
        return provided_pw.clone().unwrap().to_string();
    }
    print!("Enter password: ");
    std::io::stdout().flush().unwrap();
    read_password().unwrap()
}

async fn get_authenticated_client(args: &Args) -> Result<RconClient<TcpStream>, RconError> {
    let address = get_address(&args.address);
    let password = get_password(&args.password);

    let mut client = RconClient::connect(format!("{}", &address)).await?;
    client.auth(&password).await?;
    Ok(client)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    env_logger::Builder::from_env(
        Env::default().filter_or("RUST_LOG", "info")
    ).init();

    let mut client = get_authenticated_client(&args).await?;

    if args.command.is_some() {
        let cmd = args.command.unwrap();
        let response = client.exec(&cmd).await?;
        println!("{}", response);
        return Ok(())
    } else {
        run_cli(client, args.show_responses.is_some_and(|x| x)).await?;
    }
    Ok(())
}
