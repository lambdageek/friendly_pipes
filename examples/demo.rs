#[cfg(false)]
use friendly_pipes::{consumer, async_server};

fn main() -> std::process::ExitCode {
    if std::env::args().count() < 2 {
        println!("server");
    } else if std::env::args().nth(1).is_some_and(|arg| arg == "client") {
        println!("client");
    } else {
        eprintln!("Usage: {} [client]", std::env::args().next().unwrap());
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}