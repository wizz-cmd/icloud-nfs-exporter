mod passthrough;

use std::env;
use std::path::PathBuf;
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

    // Subcommand: mount [--fskit] <source> [mountpoint]
    if let Some(pos) = args.iter().position(|a| a == "mount") {
        let use_fskit = args.iter().any(|a| a == "--fskit");

        // Collect positional args after "mount" (skip flags)
        let mount_positionals: Vec<&String> = args[pos + 1..]
            .iter()
            .filter(|a| !a.starts_with('-'))
            .collect();

        let source = mount_positionals.first().unwrap_or_else(|| {
            eprintln!("mount requires a source directory path");
            process::exit(1);
        });
        let mountpoint = mount_positionals
            .get(1)
            .map(|s| s.as_str())
            .unwrap_or("/Volumes/icloud-nfs-exporter");

        let source_path = PathBuf::from(source);
        if !source_path.is_dir() {
            eprintln!("source is not a directory: {source}");
            process::exit(1);
        }

        if let Err(e) = std::fs::create_dir_all(mountpoint) {
            eprintln!("failed to create mountpoint {mountpoint}: {e}");
            process::exit(1);
        }

        env_logger::init();

        let ipc = IpcClient::new(socket_path);
        let fs = passthrough::IcloudFs::new(source_path, ipc);

        let mut config = fuser::Config::default();
        config.mount_options = vec![
            fuser::MountOption::RO,
            fuser::MountOption::FSName("icloud-nfs-exporter".to_string()),
            fuser::MountOption::DefaultPermissions,
        ];
        if use_fskit {
            config.mount_options.push(
                fuser::MountOption::CUSTOM("backend=fskit".to_string()),
            );
        }

        println!("Mounting {source} at {mountpoint}");
        println!("Unmount with: umount {mountpoint}");
        if let Err(e) = fuser::mount2(fs, mountpoint, &config) {
            eprintln!("mount failed: {e}");
            process::exit(1);
        }
        return;
    }

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
    print_usage();
}

fn print_usage() {
    println!(
        "\
Usage: fuse-driver [options] <command>

Commands:
  mount <source> [mountpoint]  Mount iCloud folder as FUSE filesystem
                               (default mountpoint: /Volumes/icloud-nfs-exporter)
  ping                         Check if the hydration daemon is running
  query <path>                 Query the hydration state of a file
  hydrate <path>               Request hydration of an evicted file

Options:
  -s, --socket <path>  IPC socket path (default: {DEFAULT_SOCKET})
  -v, --version        Print version and exit
  -h, --help           Print this help and exit

Environment:
  RUST_LOG=debug       Enable debug logging (e.g. hydration events)"
    );
}
