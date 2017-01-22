extern crate futures;
extern crate tokio_core;
extern crate tokio_line;
#[macro_use]
extern crate log;
extern crate env_logger;

use log::LogLevel;

use futures::sync::mpsc;
use futures::{Future, Stream, Sink, stream};
use std::{io, str};
use tokio_core::io::{Io, Codec, EasyBuf};
use tokio_core::net::{TcpStream, TcpListener};
use tokio_core::reactor::Core;
use tokio_line::LineCodec;
use std::env;
use std::io::{Read, Write, ErrorKind};

fn main() {
    env_logger::init().unwrap();

    let mut core = Core::new().unwrap();
    let address = "0.0.0.0:12345".parse().unwrap();
    let listener = TcpListener::bind(&address, &core.handle()).unwrap();

    let (tx, rx) = mpsc::unbounded();

    let connections = listener.incoming();
    let handle = core.handle();

    let rx = rx.map_err(|_| panic!()); // errors not possible on rx
    let remote_addr = "127.0.0.1:9876".parse().unwrap();
    let tcp = TcpStream::connect(&remote_addr, &handle);
    let client = tcp.and_then(|stream| {
        let (sink, _) = stream.framed(LineCodec).split();
        rx.forward(sink)
            .map(|_| ())
    });

    let client = client.map_err(|_| panic!()); // errors not possible on rx
    handle.spawn(client);

    let server = connections.for_each(|(socket, _)| {
        // Use the `Io::framed` helper to get a transport from a socket. The
        // `LineCodec` handles encoding / decoding frames.
        let transport = socket.framed(LineCodec);

        // The transport is a `Stream<Item = String>`. So we can now operate at
        // at the frame level. For each received line, write the string to
        // STDOUT.
        //
        // The return value of `for_each` is a future that completes once
        // `transport` is done yielding new lines. This happens when the
        // underlying socket closes.
        // let process_connection = transport.for_each(move |line| {
        let nonhocapitoperchedevoclonarlo = tx.clone();
        let process_connection = transport.for_each(move |line| {
            nonhocapitoperchedevoclonarlo.clone().send(line)
                .map_err(|err| io::Error::new(ErrorKind::Other, err))
                .map(|_| ())
        });

        // Spawn a new task dedicated to processing the connection
        handle.spawn(process_connection.map_err(|_| ()));

        Ok(())
    });

    core.run(server).unwrap();
}
