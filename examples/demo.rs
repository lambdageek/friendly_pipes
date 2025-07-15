use friendly_pipes::{async_server, producer};
use serde::{Deserialize, Serialize};

const ENV: &str = "PIPE_PATH";

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    #[serde(rename = "type")]
    msg_type: String,
    content: String,
}

impl Message {
    fn new_info(content: String) -> Self {
        Message {
            msg_type: "info".to_string(),
            content,
        }
    }
}

fn main() -> std::process::ExitCode {
    if std::env::args().count() < 2 {
        println!("server");
        let path = ensure_pipe_exists();
        let on_recv_line = |line: String| {
            let msg = serde_json::from_str::<Message>(&line).expect("Failed to parse JSON message");
            println!(
                "Received message [type = {}]: '{}'",
                msg.msg_type, msg.content
            );
        };
        tokio::runtime::Runtime::new()
            .expect("Failed to create Tokio runtime")
            .block_on(async {
                let current_exe =
                    std::env::current_exe().expect("Failed to get current executable path");
                let server = async_server::start_lines(&path, on_recv_line);
                let children = run_children(&current_exe, &path);
                wait_children(children);
                server.stop();
                server.wait().await
            })
            .map(|()| std::process::ExitCode::SUCCESS)
            .or_else(|e| {
                match e.try_into_panic() {
                    Ok(panic) => std::panic::resume_unwind(panic),
                    Err(e) => eprintln!("Server task failed: {:?}", e),
                }
                Ok::<std::process::ExitCode, std::convert::Infallible>(
                    std::process::ExitCode::FAILURE,
                )
            })
            .unwrap();
    } else if std::env::args().nth(1).is_some_and(|arg| arg == "client") {
        run_client();
    } else {
        eprintln!("Usage: {} [client]", std::env::args().next().unwrap());
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}

fn run_client() {
    println!("client {pid}", pid = std::process::id());
    let path = std::env::var_os(ENV).expect("PIPE_PATH environment variable not set");
    let mut client = producer::Producer::new(&path).expect("Failed to create producer");
    let msg = Message::new_info(format!(
        "Hello from client {pid}!",
        pid = std::process::id()
    ));
    serde_json::to_writer(&mut client, &msg).expect("Failed to serialize message to JSON");
    println!("Message sent to server.");
}

fn run_children(exe: &std::path::Path, pipe_path: &std::ffi::OsStr) -> Vec<std::process::Child> {
    let mut children = Vec::new();
    for _ in 0..4 {
        children.push(run_child(exe, pipe_path))
    }
    children
}

fn run_child(exe: &std::path::Path, pipe_path: &std::ffi::OsStr) -> std::process::Child {
    let child = std::process::Command::new(exe)
        .env(ENV, pipe_path)
        .arg("client")
        .spawn()
        .expect("Failed to start client");
    println!("Client started with PID: {}", child.id());
    child
}

fn wait_children(children: Vec<std::process::Child>) {
    for mut child in children {
        let status = child.wait().expect("Failed to wait for child process");
        let id = child.id();
        if status.success() {
            println!("Child process {id} exited successfully.");
        } else {
            eprintln!("Child process {id} exited with an error: {status}");
        }
    }
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
