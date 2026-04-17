mod icloud_nfs;

use std::path::PathBuf;
use std::process;

use fuse_core::IpcClient;
use nfsserve::tcp::{NFSTcp, NFSTcpListener};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_SOCKET: &str = "/tmp/icloud-nfs-exporter.sock";
const DEFAULT_PORT: u16 = 11111;

fn print_usage() {
    eprintln!("icloud-nfs-exporter NFS server v{VERSION}");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  nfs-server serve <source-dir> [--port PORT] [--socket PATH]");
    eprintln!("  nfs-server ping [--socket PATH]");
    eprintln!("  nfs-server query <path> [--socket PATH]");
    eprintln!("  nfs-server hydrate <path> [--socket PATH]");
    eprintln!("  nfs-server --version | --help");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --port PORT      NFS listen port (default: {DEFAULT_PORT})");
    eprintln!("  --socket PATH    Hydration daemon socket (default: {DEFAULT_SOCKET})");
}

fn parse_flag(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 || args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        process::exit(if args.len() < 2 { 1 } else { 0 });
    }

    if args.iter().any(|a| a == "--version" || a == "-v") {
        println!("nfs-server {VERSION}");
        return;
    }

    let socket_path = parse_flag(&args, "--socket")
        .or_else(|| parse_flag(&args, "-s"))
        .unwrap_or_else(|| DEFAULT_SOCKET.to_string());

    match args[1].as_str() {
        "serve" => {
            let positionals: Vec<&String> = args[2..]
                .iter()
                .filter(|a| !a.starts_with('-'))
                .collect();

            let source = positionals.first().unwrap_or_else(|| {
                eprintln!("Error: source directory required");
                eprintln!("Usage: nfs-server serve <source-dir>");
                process::exit(1);
            });

            let source_path = PathBuf::from(source);
            if !source_path.is_dir() {
                eprintln!("Error: {} is not a directory", source_path.display());
                process::exit(1);
            }

            let port: u16 = parse_flag(&args, "--port")
                .and_then(|p| p.parse().ok())
                .unwrap_or(DEFAULT_PORT);

            let bind_addr = format!("0.0.0.0:{port}");
            let fs = icloud_nfs::IcloudNfs::new(source_path.clone(), &socket_path);

            let listener = NFSTcpListener::bind(&bind_addr, fs).await.unwrap_or_else(|e| {
                eprintln!("Failed to bind NFS server to {bind_addr}: {e}");
                process::exit(1);
            });

            println!("Serving {} via NFSv3 on port {port}", source_path.display());
            println!("Mount with:");
            println!(
                "  Linux:  sudo mount.nfs -o vers=3,tcp,port={port},mountport={port},nolock HOST:/ /mnt"
            );
            println!(
                "  macOS:  mount_nfs -o vers=3,tcp,port={port},mountport={port},nolocks HOST:/ /mnt"
            );

            listener.handle_forever().await.unwrap_or_else(|e| {
                eprintln!("NFS server error: {e}");
                process::exit(1);
            });
        }

        "ping" => {
            let ipc = IpcClient::new(&socket_path);
            match ipc.ping() {
                Ok(()) => println!("pong"),
                Err(e) => {
                    eprintln!("ping failed: {e}");
                    process::exit(1);
                }
            }
        }

        "query" => {
            let path = args.get(2).unwrap_or_else(|| {
                eprintln!("Usage: nfs-server query <path>");
                process::exit(1);
            });
            let ipc = IpcClient::new(&socket_path);
            match ipc.query_state(path) {
                Ok(state) => println!("{state:?}"),
                Err(e) => {
                    eprintln!("query failed: {e}");
                    process::exit(1);
                }
            }
        }

        "hydrate" => {
            let path = args.get(2).unwrap_or_else(|| {
                eprintln!("Usage: nfs-server hydrate <path>");
                process::exit(1);
            });
            let ipc = IpcClient::new(&socket_path);
            match ipc.hydrate(path) {
                Ok(()) => println!("hydrated: {path}"),
                Err(e) => {
                    eprintln!("hydration failed: {e}");
                    process::exit(1);
                }
            }
        }

        other => {
            eprintln!("Unknown command: {other}");
            print_usage();
            process::exit(1);
        }
    }
}
