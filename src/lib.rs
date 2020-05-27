use glib::prelude::*;

use gio::prelude::*;

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::prelude::*;
use futures::{AsyncRead, AsyncWrite};

use pin_project::pin_project;

pub struct SocketClient(gio::SocketClient);

impl SocketClient {
    pub fn new() -> Self {
        SocketClient(gio::SocketClient::new())
    }

    pub async fn connect<P: IsA<gio::SocketConnectable> + Clone + 'static>(
        &self,
        connectable: &P,
    ) -> Result<SocketConnection, glib::Error> {
        let connection = self.0.connect_async_future(connectable).await?;

        // Get the input/output streams and convert them to the AsyncRead and AsyncWrite adapters
        // FIXME: Code duplication
        let ostream = connection
            .get_output_stream()
            .unwrap()
            .dynamic_cast::<gio::PollableOutputStream>()
            .unwrap();
        let write = ostream.into_async_write().unwrap();

        let istream = connection
            .get_input_stream()
            .unwrap()
            .dynamic_cast::<gio::PollableInputStream>()
            .unwrap();
        let read = istream.into_async_read().unwrap();

        Ok(SocketConnection {
            connection,
            read,
            write,
        })
    }
}

pub struct SocketConnection {
    #[allow(unused)]
    connection: gio::SocketConnection,
    read: gio::InputStreamAsyncRead<gio::PollableInputStream>,
    write: gio::OutputStreamAsyncWrite<gio::PollableOutputStream>,
}

impl SocketConnection {
    pub fn get_local_address(self: &Self) -> Result<gio::SocketAddress, glib::Error> {
        self.connection.get_local_address()
    }

    pub fn get_remote_address(self: &Self) -> Result<gio::SocketAddress, glib::Error> {
        self.connection.get_remote_address()
    }
}

// Proxy to the internal AsyncRead
impl AsyncRead for SocketConnection {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut Pin::get_mut(self).read).poll_read(cx, buf)
    }
}

// Proxy to the internal AsyncWrite
impl AsyncWrite for SocketConnection {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut Pin::get_mut(self).write).poll_write(cx, buf)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut Pin::get_mut(self).write).poll_close(cx)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut Pin::get_mut(self).write).poll_flush(cx)
    }
}

pub struct SocketListener(gio::SocketListener);

impl SocketListener {
    pub fn new() -> Self {
        SocketListener(gio::SocketListener::new())
    }

    pub fn add_inet_port(&self, port: u16) -> Result<(), glib::Error> {
        self.0.add_inet_port(port, None::<&glib::Object>)
    }

    pub fn add_address<P: IsA<gio::SocketAddress>, Q: IsA<glib::Object>>(
        &self,
        address: &P,
        type_: gio::SocketType,
        protocol: gio::SocketProtocol,
        source_object: Option<&Q>
    ) -> Result<gio::SocketAddress, glib::Error> {
        self.0.add_address(address, type_, protocol, source_object)
    }

    pub async fn accept(&self) -> Result<SocketConnection, glib::Error> {
        let connection = self.0.accept_async_future().await?.0;

        // Get the input/output streams and convert them to the AsyncRead and AsyncWrite adapters
        // FIXME: Code duplication
        let ostream = connection
            .get_output_stream()
            .unwrap()
            .dynamic_cast::<gio::PollableOutputStream>()
            .unwrap();
        let write = ostream.into_async_write().unwrap();

        let istream = connection
            .get_input_stream()
            .unwrap()
            .dynamic_cast::<gio::PollableInputStream>()
            .unwrap();
        let read = istream.into_async_read().unwrap();

        Ok(SocketConnection {
            connection,
            read,
            write,
        })
    }

    pub fn incoming(&self) -> Incoming {
        Incoming {
            listener: self,
            pending: None,
        }
    }
}

// FIXME: Not sure if this is correct
#[pin_project]
pub struct Incoming<'a> {
    listener: &'a SocketListener,
    pending: Option<
        Pin<
            Box<
                dyn Future<
                        Output = Result<(gio::SocketConnection, Option<glib::Object>), glib::Error>,
                    > + 'static,
            >,
        >,
    >,
}

impl<'a> Stream for Incoming<'a> {
    type Item = Result<SocketConnection, glib::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        if this.pending.is_none() {
            let fut = this.listener.0.accept_async_future();

            *this.pending = Some(fut);
        }

        let fut = this.pending.as_mut().unwrap();
        match fut.poll_unpin(cx) {
            Poll::Ready(Ok((connection, _))) => {
                *this.pending = None;

                // Get the input/output streams and convert them to the AsyncRead and AsyncWrite adapters
                // FIXME: Code duplication
                let ostream = connection
                    .get_output_stream()
                    .unwrap()
                    .dynamic_cast::<gio::PollableOutputStream>()
                    .unwrap();
                let write = ostream.into_async_write().unwrap();

                let istream = connection
                    .get_input_stream()
                    .unwrap()
                    .dynamic_cast::<gio::PollableInputStream>()
                    .unwrap();
                let read = istream.into_async_read().unwrap();

                Poll::Ready(Some(Ok(SocketConnection {
                    connection,
                    read,
                    write,
                })))
            }
            Poll::Ready(Err(err)) => {
                *this.pending = None;

                Poll::Ready(Some(Err(err)))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
