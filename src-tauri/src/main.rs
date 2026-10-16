#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().nth(1).as_deref() == Some("mcp") {
        if let Err(e) = kala_lib::mcp::run_blocking() {
            eprintln!("kala mcp: {e}");
            std::process::exit(1);
        }
        return;
    }
    kala_lib::run()
}
