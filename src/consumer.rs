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
    use futures_core::Stream;
    use tokio::io::AsyncRead;
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
    pub struct Consumer {
        server: NamedPipeServer,
        pipe_name: std::ffi::OsString,
    }

    pub struct ConsumerClient {
        stream: NamedPipeServer,
    }

    impl Consumer {
        pub fn new(name: &std::ffi::OsStr) -> std::io::Result<Self> {
            ServerOptions::new()
                .first_pipe_instance(true)
                .create(name)
                .map(|server| Consumer {
                    server,
                    pipe_name: name.to_owned(),
                })
        }
    }

    async fn accept_connection(consumer: &mut Consumer) -> std::io::Result<ConsumerClient> {
        let server = &consumer.server;
        server.connect().await?;
        let new_server = ServerOptions::new().create(&consumer.pipe_name)?;
        let stream = std::mem::replace(&mut consumer.server, new_server);
        Ok(ConsumerClient { stream })
    }

    impl Stream for Consumer {
        type Item = std::io::Result<ConsumerClient>;

        fn poll_next(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Self::Item>> {
            use std::pin::pin;
            let conn = accept_connection(&mut self);
            let pin = pin!(conn);
            pin.poll(cx).map(Some)
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
