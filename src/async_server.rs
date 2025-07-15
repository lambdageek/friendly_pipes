use std::ffi::OsStr;

use crate::consumer::Consumer;

use futures_util::StreamExt;
use tokio::io::AsyncReadExt;
use tokio::select;
use tokio::task;
use tokio_util::sync::CancellationToken;

/// Called by the server every time a new client connects
pub trait ReceiverSource {
    type Receiver: Receiver + Send + 'static;
    fn new_receiver(&mut self) -> Self::Receiver;
}

pub trait Receiver {
    type Item : ?Sized;
    /// Called when new data is received from the client
    fn receive(&self, data: &Self::Item) -> std::io::Result<()>;
}

/// A receiver source that just clones a prototypical receiver
pub struct PrototypeReceiverSource<R>
{
    prototype: std::sync::Arc<R>,
}

/// A receiver that just calls a closure repeatedly
pub struct ClosureReceiver<T, F>
where
    T: ?Sized,
    F: Fn(&T) + Send + Sync + 'static,
{
    receive: F,
    phantom: std::marker::PhantomData<fn(&T)>
}

impl<R> PrototypeReceiverSource<R>
{
    pub fn new(r: R) -> Self {
        PrototypeReceiverSource { prototype: std::sync::Arc::new(r) }
    }
}

impl<R> ReceiverSource for PrototypeReceiverSource<R>
where
    R: Receiver + Clone + Send + Sync + 'static,
{
    type Receiver = std::sync::Arc<R>;

    fn new_receiver(&mut self) -> Self::Receiver {
        self.prototype.clone()
    }
}

impl<R> Receiver for std::sync::Arc<R>
where
    R: Receiver + Send + Sync + 'static,
{
    type Item = R::Item;
    fn receive(&self, data: &Self::Item) -> std::io::Result<()> {
        self.as_ref().receive(data)
    }
}

impl<T, F> ClosureReceiver<T, F>
where
    T: ?Sized,
    F: Fn(&T) + Send + Sync + 'static,
{
    pub fn new(receive: F) -> Self {
        ClosureReceiver {
            receive,
            phantom: std::marker::PhantomData,
        }
    }
}

impl<T, F> Receiver for ClosureReceiver<T, F>
where
    T: ?Sized,
    F: Fn(&T) + Send + Sync + 'static,
{
    type Item = T;
    fn receive(&self, data: &Self::Item) -> std::io::Result<()> {
        (self.receive)(data);
        Ok(())
    }
}

pub async fn start_full<R>(
    path: &OsStr,
    finish: CancellationToken,
    mut receiver_source: R,
) -> std::io::Result<()>
where
    R: ReceiverSource,
    R::Receiver: Receiver<Item = [u8]> + Send + 'static,
{
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
                        let receiver = receiver_source.new_receiver();
                        task::spawn(async move {
                        let mut buffer = vec![0; 1024];
                        loop {
                            match client.read(&mut buffer).await {
                                Ok(0) => {
                                    println!("Client disconnected.");
                                    break;
                                }
                                Ok(n) => {
                                    receiver.receive(&buffer[..n]).unwrap();
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

pub fn start<F>(path: &OsStr, on_data: F) -> AsyncServer
where
    F: for<'a> Fn(&'a [u8]) + Send + Sync + 'static,
{
    let cancel = CancellationToken::new();

    let finish = cancel.clone();
    let path = path.to_owned();

    let prototype = std::sync::Arc::new(ClosureReceiver::new(on_data));
    let receiver_source = PrototypeReceiverSource::new(prototype);

    tokio::spawn(async move {
        start_full(&path, finish, receiver_source).await.unwrap();
    });

    AsyncServer { cancel }
}

impl AsyncServer {
    pub fn stop(&self) {
        self.cancel.cancel();
    }
}
