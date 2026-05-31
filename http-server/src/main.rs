use std::process;

use localhost::{config, server};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let config_path = match args.get(1) {
        Some(p) => p.as_str(),
        None => {
            eprintln!("Usage: {} <config_file>", args[0]);
            eprintln!("Example: {} config.conf", args[0]);
            process::exit(1);
        }
    };

    println!("localhost — loading config from '{}'", config_path);

    match config::load(config_path) {
        Ok(cfg) => {
            println!("Config loaded successfully. {} server(s) configured:", cfg.servers.len());
            for s in &cfg.servers {
                println!(
                    "  {} — {} route(s), body_limit={}",
                    s.identity(),
                    s.routes.len(),
                    if s.client_max_body_size == 0 { "unlimited".to_string() }
                    else { format!("{}B", s.client_max_body_size) }
                );
            }
            println!("\n[Phase 2 — starting epoll event loop]");
            if let Err(e) = server::run(&cfg) {
                eprintln!("Server error: {}", e);
                process::exit(1);
            }
        }
        Err(errors) => {
            eprintln!("Config errors ({}):", errors.len());
            for e in &errors {
                eprintln!("  - {}", e);
            }
            process::exit(1);
        }
    }
}
