mod routes;
mod session;

use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let sessions = session::new_session_store();
    let app = routes::build_router(sessions);

    // Try ports 5050-5059, use first available
    let mut listener = None;
    let mut bound_port = 0u16;
    for port in 5050..=5059 {
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => {
                bound_port = port;
                listener = Some(l);
                break;
            }
            Err(e) => {
                eprintln!("Port {port} unavailable: {e}");
            }
        }
    }

    let listener = match listener {
        Some(l) => l,
        None => {
            eprintln!("ERROR: Could not bind to any port in range 5050-5059");
            std::process::exit(1);
        }
    };

    let version = option_env!("HP_VERSION").unwrap_or("dev");
    let url = format!("http://localhost:{bound_port}");
    eprintln!("House Puzzle Editor v{version} listening on {url}");

    // Open browser
    if let Err(e) = open::that(&url) {
        eprintln!("Could not open browser: {e}");
    }

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("Server error: {e}");
        std::process::exit(1);
    }
}
