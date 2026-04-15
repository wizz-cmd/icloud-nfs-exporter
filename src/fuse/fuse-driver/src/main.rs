use std::env;
use std::process;

use fuse_core::IpcClient;

const DEFAULT_SOCKET: &str = "/tmp/icloud-nfs-exporter.sock";

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-v") {
        println!("icloud-nfs-exporter FUSE driver v{}", fuse_core::VERSION);
        process::exit(0);
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        process::exit(0);
    }

    let socket_path = args
        .iter()
        .position(|a| a == "--socket" || a == "-s")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or(DEFAULT_SOCKET);

    // Subcommand: ping
    if args.iter().any(|a| a == "ping") {
        let client = IpcClient::new(socket_path);
        match client.ping() {
            Ok(()) => {
                println!("pong — daemon is running at {socket_path}");
            }
            Err(e) => {
                eprintln!("ping failed: {e}");
                process::exit(1);
            }
        }
        return;
    }

    // Subcommand: query <path>
    if let Some(pos) = args.iter().position(|a| a == "query") {
        let path = args.get(pos + 1).unwrap_or_else(|| {
            eprintln!("query requires a file path");
            process::exit(1);
        });
        let client = IpcClient::new(socket_path);
        match client.query_state(path) {
            Ok(state) => println!("{path}: {state:?}"),
            Err(e) => {
                eprintln!("query failed: {e}");
                process::exit(1);
            }
        }
        return;
    }

    // Subcommand: hydrate <path>
    if let Some(pos) = args.iter().position(|a| a == "hydrate") {
        let path = args.get(pos + 1).unwrap_or_else(|| {
            eprintln!("hydrate requires a file path");
            process::exit(1);
        });
        let client = IpcClient::new(socket_path);
        match client.hydrate(path) {
            Ok(()) => println!("{path}: hydrated"),
            Err(e) => {
                eprintln!("hydrate failed: {e}");
                process::exit(1);
            }
        }
        return;
    }

    // No subcommand — print status
    println!("icloud-nfs-exporter FUSE driver v{}", fuse_core::VERSION);
    println!();
    println!("FUSE mount not yet available (requires macFUSE).");
    println!("Use subcommands to interact with the hydration daemon:");
    println!();
    print_usage();
}

fn print_usage() {
    println!(
        "\
Usage: fuse-driver [options] <command>

Commands:
  ping               Check if the hydration daemon is running
  query <path>       Query the hydration state of a file
  hydrate <path>     Request hydration of an evicted file

Options:
  -s, --socket <path>  IPC socket path (default: {DEFAULT_SOCKET})
  -v, --version        Print version and exit
  -h, --help           Print this help and exit"
    );
}
