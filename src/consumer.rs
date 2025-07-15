use futures_core::{Stream, stream::BoxStream};
use futures_util::{StreamExt, stream};
use tokio::io::AsyncRead;

pub struct Consumer {
    s: BoxStream<'static, std::io::Result<ConsumerClient>>,
}

fn unfold_consumer(first_server: Server) -> std::io::Result<Consumer> {
    enum Seed {
        Server(Server),
        Failed,
    }

    let consumer = stream::unfold(Seed::Server(first_server), async move |seed| match seed {
        Seed::Failed => None,
        Seed::Server(server) => match accept_connection(server).await {
            Ok(Some((client, new_server))) => Some((Ok(client), Seed::Server(new_server))),
            Ok(None) => None,
            Err(e) => Some((Err(e), Seed::Failed)),
        },
    })
    .boxed();
    Ok(Consumer { s: consumer })
}

impl Consumer {
    pub fn new(name: &std::ffi::OsStr) -> std::io::Result<Self> {
        let first_server = Server::new_first(name)?;
        unfold_consumer(first_server)
    }
}

impl Stream for Consumer {
    type Item = std::io::Result<ConsumerClient>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.s.poll_next_unpin(cx)
    }
}

pub struct ConsumerClient {
    stream: ConsumerClientImpl,
}

impl AsyncRead for ConsumerClient {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let stream = unsafe { self.map_unchecked_mut(|s| &mut s.stream) };
        stream.poll_read(cx, buf)
    }
}

#[cfg(unix)]
mod unix {
    use tokio::net::{UnixListener, UnixStream};

    use super::ConsumerClient;

    struct UnlinkOnDrop(std::path::PathBuf);

    impl Drop for UnlinkOnDrop {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    pub struct Server {
        listener: UnixListener,
        cleanup: UnlinkOnDrop,
    }

    pub type ConsumerClientImpl = UnixStream;

    pub async fn accept_connection(
        server: Server,
    ) -> std::io::Result<Option<(ConsumerClient, Server)>> {
        let Server { listener, cleanup } = server;
        listener.accept().await.map(|(stream, _)| {
            let client = ConsumerClient { stream };
            let new_server = Server {
                listener,
                cleanup,
            };
            Some((client, new_server))
        })
    }

    impl Server {
        pub fn new_first(name: &std::ffi::OsStr) -> std::io::Result<Self> {
            let listener = UnixListener::bind(name)?;
            Ok(Server { listener, cleanup: UnlinkOnDrop(std::path::PathBuf::from(name)) })
        }
    }
}

#[cfg(windows)]
mod windows {
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

    use super::ConsumerClient;

    pub type ConsumerClientImpl = NamedPipeServer;

    pub struct Server {
        pipe_name: std::ffi::OsString,
        server: NamedPipeServer,
    }

    impl Server {
        pub fn new_first(name: &std::ffi::OsStr) -> std::io::Result<Self> {
            Ok(Server {
                pipe_name: name.to_owned(),
                server: ServerOptions::new()
                    .first_pipe_instance(true)
                    .create(name)?,
            })
        }
    }

    pub async fn accept_connection(
        server: Server,
    ) -> std::io::Result<Option<(ConsumerClient, Server)>> {
        let Server { pipe_name, server } = server;
        server.connect().await?;
        let client = ConsumerClient { stream: server };
        let new_server = ServerOptions::new().create(&pipe_name)?;
        let new_server = Server {
            pipe_name,
            server: new_server,
        };
        Ok(Some((client, new_server)))
    }
}

#[cfg(unix)]
use unix::*;
#[cfg(windows)]
use windows::*;
