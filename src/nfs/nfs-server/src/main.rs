mod icloud_nfs;
mod staging;

use std::path::PathBuf;
use std::process;
use std::time::Duration;

use fuse_core::IpcClient;
use log::{info, warn};
use nfsserve::tcp::{NFSTcp, NFSTcpListener};

use staging::StagingLayer;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_SOCKET: &str = "/tmp/icloud-nfs-exporter.sock";
const DEFAULT_PORT: u16 = 11111;
const DEFAULT_STAGING_DIR: &str = "~/.icne-staging";
const DEFAULT_PROMOTION_DELAY: u64 = 5;

fn print_usage() {
    eprintln!("icloud-nfs-exporter NFS server v{VERSION}");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  nfs-server serve <source-dir> [--port PORT] [--socket PATH] [--staging-dir DIR] [--promotion-delay SECS]");
    eprintln!("  nfs-server ping [--socket PATH]");
    eprintln!("  nfs-server query <path> [--socket PATH]");
    eprintln!("  nfs-server hydrate <path> [--socket PATH]");
    eprintln!("  nfs-server --version | --help");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --port PORT              NFS listen port (default: {DEFAULT_PORT})");
    eprintln!("  --socket PATH            Hydration daemon socket (default: {DEFAULT_SOCKET})");
    eprintln!("  --staging-dir DIR        Write staging directory (default: {DEFAULT_STAGING_DIR})");
    eprintln!("  --promotion-delay SECS   Seconds of quiescence before promoting staged files (default: {DEFAULT_PROMOTION_DELAY})");
}

fn parse_flag(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
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

            let staging_dir = parse_flag(&args, "--staging-dir")
                .map(|s| expand_tilde(&s))
                .unwrap_or_else(|| expand_tilde(DEFAULT_STAGING_DIR));

            let promotion_delay: u64 = parse_flag(&args, "--promotion-delay")
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_PROMOTION_DELAY);

            let staging = StagingLayer::new(staging_dir.clone(), source_path.clone())
                .unwrap_or_else(|e| {
                    eprintln!("Failed to create staging directory {}: {e}", staging_dir.display());
                    process::exit(1);
                });

            // Promote any files left from a previous session
            match staging.promote_all() {
                Ok(promoted) if !promoted.is_empty() => {
                    info!("startup recovery: promoted {} files from staging", promoted.len());
                }
                Err(e) => warn!("startup promotion failed: {e}"),
                _ => {}
            }

            let fs = icloud_nfs::IcloudNfs::new(source_path.clone(), &socket_path, staging);

            // Spawn background promotion task
            let promotion_staging = fs.staging().clone();
            let promotion_quiescence = Duration::from_secs(promotion_delay);
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    match promotion_staging.promote_if_quiesced(promotion_quiescence) {
                        Ok(promoted) if !promoted.is_empty() => {
                            info!("promoted {} files to iCloud", promoted.len());
                        }
                        Err(e) => warn!("promotion error: {e}"),
                        _ => {}
                    }
                }
            });

            let bind_addr = format!("0.0.0.0:{port}");
            let listener = NFSTcpListener::bind(&bind_addr, fs).await.unwrap_or_else(|e| {
                eprintln!("Failed to bind NFS server to {bind_addr}: {e}");
                process::exit(1);
            });

            println!("Serving {} via NFSv3 on port {port} (read-write)", source_path.display());
            println!("Staging: {}", staging_dir.display());
            println!("Promotion delay: {promotion_delay}s");
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
