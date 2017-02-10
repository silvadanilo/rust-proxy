extern crate futures;
extern crate tokio_core;
extern crate tokio_line;
#[macro_use]
extern crate log;
extern crate env_logger;

use log::LogLevel;

use futures::sync::mpsc;
use futures::{finished, Future, Stream, Sink, stream};
use std::{io, str};
use tokio_core::io::{Io, Codec, EasyBuf};
use tokio_core::net::{TcpStream, TcpListener};
use tokio_core::reactor::{Core, Handle};
use tokio_line::LineCodec;
use std::env;
use std::io::{Read, Write, ErrorKind};
use std::cell::RefCell;
use std::rc::Rc;
use futures::Async::{NotReady, Ready};
use futures::Async;
use std::borrow::Borrow;
use futures::sync::mpsc::UnboundedReceiver;

// struct Client {
//     rx: Stream<Item=String, Error=io::Error>,
// }

// impl Client {
//     // pub fn get_connection<S>(&self, handle: &Handle) -> Box<Future<Item = (), Error = io::Error>>
//     pub fn get_connection<S>(&self, handle: &Handle) -> Box<Future<Item = (), Error = io::Error>>
//     {
//         let remote_addr = "127.0.0.1:9876".parse().unwrap();
//         let tcp = TcpStream::connect(&remote_addr, handle);
//         let handle_clone = handle.clone();
//         // let rx_rc_clone = rx_rc.clone();

//         // let client = tcp.and_then(|stream| -> Result<(), io::Error> {
//         // let client = tcp.and_then(|stream| -> Result<futures::Async<u8>, io::Error> {
//         let client = tcp.and_then(|stream| {
//             let (sink, from_server) = stream.framed(LineCodec).split();
//             // let reader = from_server.for_each(|message| {
//             //     println!("{}", message);
//             //     Ok(())
//             // });

//             self.rx.forward(sink)
//                 .map_err(|_| io::Error::new(io::ErrorKind::Other, "fail to forward"))
//                 .map(|_| ())

//                 // let writer = rx.forward(sink)
//                 //     .map_err(|_| io::Error::new(io::ErrorKind::Other, "fail to forward"))
//                 //     .map(|_| ());
//                 // writer

//                 // reader.select(writer).and_then(move |_| {
//                 //     println!("CLIENT DISCONNECTED");
//                 //     // Attempt to reconnect in the future
//                 //     // get_connection(&handle_clone, rx)
//                 //     Err(io::Error::new(io::ErrorKind::Other, "clinet disconnected"))
//                 // })
//         });

//         // let client = client.and_then(|stream| {
//         // });

//         let handle_clone = handle.clone();
//         let client = client.or_else(move |err| {
//             // Note: this code will infinitely retry, but you could pattern match on the error
//             // to retry only on certain kinds of error
//             println!("Error connecting to server: {}", err);
//             // get_connection(&handle_clone, rx_rc)
//             Ok(())
//         });
//         Box::new(client)
//     }
// }

// // Try to avoid to return a Box because it have a runtime cost
// // https://github.com/alexcrichton/futures-rs/blob/6a1950e6cd91cb7fd4f27b19d0090f81dc957a05/TUTORIAL.md#returning-futures
// fn get_connection<S>(handle: &Handle, rx: S) -> Box<Future<Item = (), Error = io::Error>>
//         where S: 'static + Stream<Item=String, Error=std::io::Error>
fn get_connection<S>(handle: &Handle, rx: S) -> Box<Future<Item = (), Error = io::Error>>
        where S: 'static + Stream<Item=String, Error=std::io::Error>
{
    let remote_addr = "127.0.0.1:9876".parse().unwrap();
    let tcp = TcpStream::connect(&remote_addr, handle);
    let handle_clone = handle.clone();

    // let client = tcp.and_then(|stream| -> Result<(), io::Error> {
    // let client = tcp.and_then(|stream| -> Result<futures::Async<u8>, io::Error> {
    let client = tcp.and_then(|stream| {
        let (sink, from_server) = stream.framed(LineCodec).split();
        let reader = from_server.for_each(|message| {
            println!("{}", message);
            Ok(())
        });

        let reader = reader.and_then(|_| {
            Err(io::Error::new(io::ErrorKind::Other, "clinet disconnected"))
        });

        reader

        // Ok(())
        // rx.forward(sink)
        //     .map(|_| ())
        // let writer = rx.forward(sink)
        //     .map_err(|_| io::Error::new(io::ErrorKind::Other, "fail to forward"))
        //     .map(|_| ());
        // writer

        // reader.select(writer).and_then(move |_| {
        //     println!("CLIENT DISCONNECTED");
        //     // Attempt to reconnect in the future
        //     // get_connection(&handle_clone, rx)
        //     Err(io::Error::new(io::ErrorKind::Other, "clinet disconnected"))
        // })
    });

    // let client = client.and_then(|stream| {
    // });

    let handle_clone = handle.clone();
    let client = client.or_else(move |err| {
        // Note: this code will infinitely retry, but you could pattern match on the error
        // to retry only on certain kinds of error
        println!("Error connecting to server: {}", err);
        get_connection(&handle_clone, rx)
    });
    Box::new(client)
}

fn main() {
    env_logger::init().unwrap();

    let mut core = Core::new().unwrap();
    let address = "0.0.0.0:12345".parse().unwrap();
    let listener = TcpListener::bind(&address, &core.handle()).unwrap();
    let connections = listener.incoming();
    let handle = core.handle();

    let (tx, rx) = mpsc::unbounded();
    let rx = rx.map_err(|_| panic!()); // errors not possible on rx
    // let rx_rc = Rc::new(rx);
    // let client = get_connection(&handle, rx_rc);
    let client = get_connection(&handle, rx);

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



// extern crate futures;
// extern crate tokio_core;

// use std::io;

// use futures::stream::Stream;
// use futures::{finished, Future};
// use tokio_core::channel::channel;
// use tokio_core::reactor::Core;

// fn main() {
//     let mut main_loop = Core::new().unwrap();
//     let handle = main_loop.handle();
//     let (mut last_trx, last_rx) = channel::<u32>(&handle).unwrap();
//     last_trx.send(1).unwrap();
//     for _ in  1..5 {
//         let (tx2, rx2) = channel::<u32>(&handle).unwrap();
//         handle.spawn(rx2.for_each(move |s| {
//             last_trx.send(s + 1)
//         }).map_err(|e| panic!("{}", e)));
//         last_trx = tx2;
//     }
//     let future = last_rx.take(2).fold(0, |_, num| {
//         let num = num + 1;
//         last_trx.send(num).unwrap();
//         finished::<u32, io::Error>(num)
//     });
//     let res = main_loop.run(future).unwrap();
//     println!("res {}", res);
// }
