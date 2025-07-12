#[cfg(unix)]
mod unix {
    use futures_core::Stream;
    use tokio::io::AsyncRead;
    use tokio::net::{UnixListener, UnixStream};
    use tokio_stream::wrappers::UnixListenerStream;
    pub struct Consumer {
        listener: UnixListenerStream,
    }

    pub struct ConsumerClient {
        stream: UnixStream,
    }
    impl Consumer {
        pub fn new(name: &std::ffi::OsStr) -> std::io::Result<Self> {
            let listener = UnixListener::bind(name)?;
            let stream = UnixListenerStream::new(listener);
            Ok(Consumer { listener: stream })
        }
    }

    impl Stream for Consumer {
        type Item = std::io::Result<ConsumerClient>;

        fn poll_next(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Self::Item>> {
            let listener = unsafe { self.map_unchecked_mut(|s| &mut s.listener) };
            match listener.poll_next(cx) {
                std::task::Poll::Ready(Some(stream)) => match stream {
                    Ok(s) => std::task::Poll::Ready(Some(Ok(ConsumerClient { stream: s }))),
                    Err(e) => std::task::Poll::Ready(Some(Err(e))),
                },
                std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
                std::task::Poll::Pending => std::task::Poll::Pending,
            }
        }
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
}

#[cfg(windows)]
mod windows {
    use futures::{StreamExt, stream};
    use futures_core::Stream;
    use futures_core::stream::BoxStream;
    use tokio::io::AsyncRead;
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

    pub struct Consumer {
        s: BoxStream<'static, std::io::Result<ConsumerClient>>,
    }

    pub struct ConsumerClient {
        stream: NamedPipeServer,
    }

    struct Server {
        pipe_name: std::ffi::OsString,
        server: NamedPipeServer,
    }

    impl Consumer {
        pub fn new(name: &std::ffi::OsStr) -> std::io::Result<Self> {
            enum Seed {
                Server(Server),
                Failed,
            }
            let first_server = Server {
                pipe_name: name.to_owned(),
                server: ServerOptions::new()
                    .first_pipe_instance(true)
                    .create(name)?,
            };

            let consumer =
                stream::unfold(Seed::Server(first_server), async move |seed| match seed {
                    Seed::Failed => None,
                    Seed::Server(server) => match accept_connection(server).await {
                        Ok(Some((client, new_server))) => {
                            Some((Ok(client), Seed::Server(new_server)))
                        }
                        Ok(None) => None,
                        Err(e) => Some((Err(e), Seed::Failed)),
                    },
                })
                .boxed();
            Ok(Consumer { s: consumer })
        }
    }

    async fn accept_connection(
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

    impl Stream for Consumer {
        type Item = std::io::Result<ConsumerClient>;

        fn poll_next(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Self::Item>> {
            self.s.poll_next_unpin(cx)
        }
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
}

#[cfg(unix)]
pub use unix::*;
#[cfg(windows)]
pub use windows::*;
