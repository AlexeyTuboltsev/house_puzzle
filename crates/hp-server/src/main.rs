mod routes;

use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let app = routes::build_router();

    let addr = SocketAddr::from(([0, 0, 0, 0], 5050));
    eprintln!("House Puzzle Editor listening on http://localhost:5050");

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind to {addr}: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("Server error: {e}");
        std::process::exit(1);
    }
}
