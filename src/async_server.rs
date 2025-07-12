use std::ffi::OsStr;

use crate::consumer::Consumer;

use tokio::io::AsyncReadExt;
use tokio::select;
use tokio::task;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

pub async fn start_full<F>(
    path: &OsStr,
    finish: CancellationToken,
    on_line: F,
) -> std::io::Result<()>
where
    F: Fn(&[u8]) + Send + Sync + 'static,
{
    let on_line = std::sync::Arc::new(on_line);
    let mut consumer = Consumer::new(path)?;
    loop {
        select! {
            _ = finish.cancelled() => {
                println!("Server stopped by cancellation token.");
                break;
            }
            it = consumer.next() => {
                println!("New client connected.");
                match it {
                    Some(Ok(mut client)) => {
                        let on_line = on_line.clone();
                        task::spawn(async move {
                        let mut buffer = vec![0; 1024];
                        loop {
                            match client.read(&mut buffer).await {
                                Ok(0) => {
                                    println!("Client disconnected.");
                                    break;
                                }
                                Ok(n) => {
                                    on_line(&buffer[..n]);
                                }
                                Err(e) => {
                                    eprintln!("Error reading from client: {e}");
                                    break;
                                }
                            }
                        }
                    });
                },
                Some(Err(e)) => {
                    eprintln!("Error accepting client: {e}");
                    break;
                }
                None => {
                    println!("No more clients connected.");
                    break;
                }
            }
            }
        }
    }
    Ok(())
}

pub struct AsyncServer {
    cancel: CancellationToken,
}

pub fn start<F>(path: &OsStr, on_line: F) -> AsyncServer
where
    F: Fn(&[u8]) + Send + Sync + 'static,
{
    let cancel = CancellationToken::new();

    let finish = cancel.clone();
    let path = path.to_owned();

    tokio::spawn(async move {
        start_full(&path, finish, on_line).await.unwrap();
    });

    AsyncServer { cancel }
}

impl AsyncServer {
    pub fn stop(&self) {
        self.cancel.cancel();
    }
}
