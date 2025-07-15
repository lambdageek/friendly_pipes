use std::ffi::OsStr;

use crate::consumer::Consumer;
use crate::consumer::ConsumerClient;

use futures_util::StreamExt;
use tokio::io::AsyncReadExt;
use tokio::select;
use tokio::task;
use tokio_util::sync::CancellationToken;

async fn server<C>(
    path: &OsStr,
    finish: CancellationToken,
    mut client_action: C,
) -> std::io::Result<()>
where
    C: FnMut(ConsumerClient),
{
    let mut consumer = Consumer::new(path)?;
    loop {
        select! {
            _ = finish.cancelled() => {
                println!("Server stopped by cancellation token.");
                break;
            }
            it = consumer.next() => {
                match it {
                    Some(Ok(client)) => {
                        println!("New client connected.");
                        client_action(client);
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

pub async fn start_full<F>(
    path: &OsStr,
    finish: CancellationToken,
    callback_prototype: std::sync::Arc<F>,
) -> std::io::Result<()>
where
    F: Fn(&[u8]) + Send + Sync + 'static,
{
    server(path, finish, |mut client| {
        let callback = callback_prototype.clone();
        task::spawn(async move {
            let mut buffer = vec![0; 1024];
            loop {
                match client.read(&mut buffer).await {
                    Ok(0) => {
                        println!("Client disconnected.");
                        break;
                    }
                    Ok(n) => {
                        callback(&buffer[..n]);
                    }
                    Err(e) => {
                        eprintln!("Error reading from client: {e}");
                        break;
                    }
                }
            }
        });
    })
    .await
}

pub struct ByteSliceServer {
    cancel: CancellationToken,
    join_handle: task::JoinHandle<()>,
}

pub fn start<F>(path: &OsStr, on_data: F) -> ByteSliceServer
where
    F: for<'a> Fn(&'a [u8]) + Send + Sync + 'static,
{
    let cancel = CancellationToken::new();

    let finish = cancel.clone();
    let path = path.to_owned();

    let callback = std::sync::Arc::new(on_data);

    let join_handle = tokio::spawn(async move {
        start_full(&path, finish, callback).await.unwrap();
    });

    ByteSliceServer {
        cancel,
        join_handle,
    }
}

impl ByteSliceServer {
    pub fn stop(&self) {
        self.cancel.cancel();
    }

    pub async fn wait(self) -> Result<(), task::JoinError> {
        self.join_handle.await
    }
}

#[cfg(feature = "lines_server")]
mod lines_server {
    use super::*;
    use tokio_util::codec::{FramedRead, LinesCodec};
    // use tokio_util::io::ReaderStream;

    pub struct StringLinesServer {
        cancel: CancellationToken,
        join_handle: task::JoinHandle<()>,
    }

    impl StringLinesServer {
        pub fn stop(&self) {
            self.cancel.cancel();
        }

        pub async fn wait(self) -> Result<(), task::JoinError> {
            self.join_handle.await
        }
    }

    pub fn start_lines<F>(path: &OsStr, on_line: F) -> StringLinesServer
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        let cancel = CancellationToken::new();
        let finish = cancel.clone();
        let on_line = std::sync::Arc::new(on_line);
        let path = path.to_owned();
        let join_handle = tokio::spawn(async move {
            server(&path, finish, |client| {
                let on_line = on_line.clone();
                tokio::spawn(async move {
                    let mut stream = FramedRead::new(client, LinesCodec::new());
                    while let Some(line) = stream.next().await {
                        match line {
                            Ok(line) => on_line(line),
                            Err(e) => eprintln!("Error reading line: {e}"),
                        }
                    }
                });
            })
            .await
            .unwrap();
        });
        StringLinesServer {
            cancel,
            join_handle,
        }
    }
}

pub use lines_server::start_lines;
