use futures::prelude::*;
use gio_futures::SocketListener;

async fn run() {
    let listener = SocketListener::new();
    listener.add_inet_port(12345).unwrap();

    let mut incoming = listener.incoming();
    while let Some(conn) = incoming.next().await {
        let mut conn = conn.unwrap();

        println!("new connection");
        conn.write_all(b"test").await.unwrap();
    }
}

fn main() {
    let ctx = glib::MainContext::default();
    ctx.push_thread_default();
    ctx.block_on(run());
    ctx.pop_thread_default();
}
