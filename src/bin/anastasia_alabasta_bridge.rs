#[path = "../alabasta_bridge.rs"]
mod alabasta_bridge;

/// Run the dedicated stdio transport without initializing the Anastasia GUI.
fn main() {
    if let Err(error) = alabasta_bridge::serve_stdio() {
        eprintln!("Anastasia Alabasta Bridge: {error:#}");
        std::process::exit(1);
    }
}
