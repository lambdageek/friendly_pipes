use friendly_pipes::{async_server, producer};

const ENV: &str = "PIPE_PATH";

fn main() -> std::process::ExitCode {
    if std::env::args().count() < 2 {
        println!("server");
        let path = ensure_pipe_exists();
        let on_line = |line: &[u8]| {
            println!("Received line: {}", String::from_utf8_lossy(line));
        };
        tokio::runtime::Runtime::new()
            .expect("Failed to create Tokio runtime")
            .block_on(async {
                let current_exe =
                    std::env::current_exe().expect("Failed to get current executable path");
                let server = async_server::start(&path, on_line);
                let mut child = std::process::Command::new(current_exe)
                    .env(ENV, &path)
                    .arg("client")
                    .spawn()
                    .expect("Failed to start client");
                println!("Client started with PID: {}", child.id());
                child.wait().expect("Failed to wait for client process");
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                server.stop();
            });
    } else if std::env::args().nth(1).is_some_and(|arg| arg == "client") {
        println!("client");
        let path = std::env::var_os(ENV).expect("PIPE_PATH environment variable not set");
        let mut client = producer::Producer::new(&path).expect("Failed to create producer");
        let msg = format!("Hello from client {pid}!\n", pid = std::process::id());
        client
            .write(msg.as_bytes())
            .expect("Failed to write to pipe");
        println!("Message sent to server.");
    } else {
        eprintln!("Usage: {} [client]", std::env::args().next().unwrap());
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(unix)]
fn ensure_pipe_exists() -> std::ffi::OsString {
    let base_dir = std::path::Path::new("/tmp/friendly_pipe");
    std::fs::create_dir_all(base_dir).expect("Failed to create base directory");
    let path = base_dir.join(format!("{pid}.sock", pid = std::process::id()));
    path.as_os_str().to_owned()
}

#[cfg(windows)]
fn ensure_pipe_exists() -> std::ffi::OsString {
    let name = format!("\\\\.\\pipe\\friendly_pipe_{pid}", pid = std::process::id());
    name.into()
}
