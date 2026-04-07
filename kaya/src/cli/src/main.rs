//! KAYA CLI: interactive command-line client.

use clap::Parser;
use kaya_sdk::{ClientConfig, KayaClient};

#[derive(Parser, Debug)]
#[command(name = "kaya-cli", version, about = "KAYA command-line client")]
struct Args {
    /// Server host.
    #[arg(short = 'H', long, default_value = "127.0.0.1")]
    host: String,

    /// Server port.
    #[arg(short, long, default_value_t = 6380)]
    port: u16,

    /// Password for authentication.
    #[arg(short = 'a', long)]
    password: Option<String>,

    /// Command to execute (non-interactive mode).
    #[arg(trailing_var_arg = true)]
    command: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let config = ClientConfig {
        host: args.host.clone(),
        port: args.port,
        password: args.password,
        ..ClientConfig::default()
    };

    let mut client = KayaClient::connect(&config).await.map_err(|e| {
        anyhow::anyhow!("failed to connect to {}:{}: {}", args.host, args.port, e)
    })?;

    if !args.command.is_empty() {
        // Non-interactive: execute single command.
        let cmd_args: Vec<&str> = args.command.iter().map(|s| s.as_str()).collect();
        let resp = client.execute_raw(&cmd_args).await?;
        println!("{}", format_frame(&resp));
    } else {
        // Interactive mode.
        println!("Connected to KAYA at {}:{}", args.host, args.port);
        println!("Type 'quit' to exit.\n");

        let mut reader = tokio::io::BufReader::new(tokio::io::stdin());
        let mut line = String::new();

        loop {
            eprint!("kaya> ");
            line.clear();
            let n = tokio::io::AsyncBufReadExt::read_line(
                &mut reader,
                &mut line,
            )
            .await?;

            if n == 0 {
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.eq_ignore_ascii_case("quit") || trimmed.eq_ignore_ascii_case("exit") {
                break;
            }

            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            match client.execute_raw(&parts).await {
                Ok(resp) => println!("{}", format_frame(&resp)),
                Err(e) => eprintln!("(error) {e}"),
            }
        }
    }

    Ok(())
}

fn format_frame(frame: &kaya_protocol::Frame) -> String {
    match frame {
        kaya_protocol::Frame::SimpleString(s) => s.clone(),
        kaya_protocol::Frame::Error(e) => format!("(error) {e}"),
        kaya_protocol::Frame::Integer(n) => format!("(integer) {n}"),
        kaya_protocol::Frame::BulkString(b) => {
            String::from_utf8(b.to_vec()).unwrap_or_else(|_| format!("{b:?}"))
        }
        kaya_protocol::Frame::Null => "(nil)".into(),
        kaya_protocol::Frame::Boolean(b) => format!("(boolean) {b}"),
        kaya_protocol::Frame::Double(d) => format!("(double) {d}"),
        kaya_protocol::Frame::Array(items) => {
            if items.is_empty() {
                return "(empty array)".into();
            }
            let mut out = String::new();
            for (i, item) in items.iter().enumerate() {
                out.push_str(&format!("{}) {}\n", i + 1, format_frame(item)));
            }
            out.trim_end().to_string()
        }
        other => format!("{other:?}"),
    }
}
