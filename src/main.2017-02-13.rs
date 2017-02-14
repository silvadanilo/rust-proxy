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
use tokio_core::io::{Io, Codec, EasyBuf, Framed};
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
use std::net::SocketAddr;
use std::collections::HashMap;

#[derive(Clone)]
struct Client {
    tx: mpsc::Sender<String>,
}

impl Client {
    fn new(tx: mpsc::Sender<String>) -> Client {
        Client {
            tx: tx,
        }
    }
}

#[derive(Clone)]
struct ConnectedClients(Rc<RefCell<HashMap<u32, Client>>>);

impl ConnectedClients {
    pub fn new() -> ConnectedClients {
        ConnectedClients(Rc::new(RefCell::new(HashMap::new())))
    }

    pub fn insert(&self, i: u32, client: Client) {
        self.0.borrow_mut().insert(i, client);
    }

    pub fn remove(&self, i: &u32) -> Option<Client> {
        self.0.borrow_mut().remove(i)
    }

    pub fn broadcast<E: 'static>(&self, message: String) -> Box<Future<Item = (), Error = E>> {
        let client_map: std::cell::RefMut<HashMap<u32, Client>> = self.0.borrow_mut();

        // For each client, clone its `mpsc::Sender` (because sending consumes the sender) and
        // start sending a clone of `message`. This produces an iterator of Futures.
        let clients = client_map.values();
        let all_sends = clients.map(|client| client.tx.clone().send(message.clone()));

        // Collect the futures into a stream. We don't care about:
        //
        //    1. what order they finish (hence `futures_unordered`)
        //    2. the result of any individual send (hence the `.then(|_| Ok(()))`. If the send
        //       succeeds we don't need the `Sender` back since we still have it in our hashmap.
        //       If the send fails its because the receiver is gone, so we don't need to broadcast
        //       to them anyway.
        let send_stream = stream::futures_unordered(all_sends).then(|_| Ok(()));

        // Convert the stream to a future that runs all the sends and box it up.
        Box::new(send_stream.for_each(|()| Ok(())))
    }
}

// // Try to avoid to return a Box because it have a runtime cost
// // https://github.com/alexcrichton/futures-rs/blob/6a1950e6cd91cb7fd4f27b19d0090f81dc957a05/TUTORIAL.md#returning-futures
// fn get_connection<S>(handle: &Handle, rx: S) -> Box<Future<Item = (), Error = io::Error>>
//         where S: 'static + Stream<Item=String, Error=std::io::Error>
// fn get_connection<S>(handle: &Handle, rx: S) -> Box<Future<Item = (), Error = io::Error>>
//         where S: 'static + Stream<Item=String, Error=std::io::Error>
// fn get_connection(handle: &Handle, c: ConnectedClients) -> Box<Future<Item = (), Error = io::Error>>
fn get_connection(handle: &Handle, connToServer: ConnectedClients) -> Box<Future<Item = (), Error = io::Error>>
{
    let remote_addr = "127.0.0.1:9876".parse().unwrap();
    let tcp = TcpStream::connect(&remote_addr, handle);
    let handle_clone = handle.clone();
    let i: u32 = 0;

    // let client = tcp.and_then(|stream| -> Result<(), io::Error> {
    // let client = tcp.and_then(|stream| -> Result<futures::Async<u8>, io::Error> {
    let connToServer_clone = connToServer.clone();
    let client = tcp.and_then(move |stream| {
        let (tx, rx) = mpsc::channel(8);
        connToServer_clone.insert(i, Client::new(tx));

        let (sender, receiver) = stream.framed(LineCodec).split();

        let reader = receiver.for_each(|message| {
            println!("{}", message);
            Ok(())
        });

        let writer = rx.forward(sender)
            // .map(|_| ())
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "fail to forward"));

        // let writer = rx
        //     .map_err(|()| unreachable!("rx can't fail"))
        //     .fold(sender, |sender, msg| {
        //         sender.send(msg)
        //         //Err(io::Error::new(io::ErrorKind::Other, "aa"))
        //     })
        //     .map(|_| ());
        //     // .map_err(|()| Err(io::Error::new(io::ErrorKind::Other, "aaargh")))
        //     // .map(|_| ());

        reader.and_then(move |_| {
            println!("Client Disconnected");
            Err(io::Error::new(io::ErrorKind::Other, "client disconnected"))
        })
        // reader.select(writer).map(move |_| {
        //     println!("Client Disconnected");
        //     Err(io::Error::new(io::ErrorKind::Other, "client disconnected"))
        // }).map_err(|(err, _)| {
        //     ()
        //     // println!("Errore nel reader: {}", err);
        //     // err
        // })

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
        //     // get_connection(&handle_clone, connToServer_clone)
        //     Err(io::Error::new(io::ErrorKind::Other, "clinet disconnected"))
        // })
    });

    // let client = client.and_then(|stream| {
    // });

    let handle_clone = handle.clone();
    let connToServer_clone = connToServer.clone();
    let client = client.or_else(move |err| {
        // Note: this code will infinitely retry, but you could pattern match on the error
        // to retry only on certain kinds of error
        println!("Error connecting to server: {}", err);
        get_connection(&handle_clone, connToServer_clone)
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

    let clients = ConnectedClients::new();

    let (tx, rx) = mpsc::unbounded();
    // let rx = rx.map_err(|_| panic!()); // errors not possible on rx

    let clients_clone = clients.clone();
    let sync = rx.for_each(move |msg| {
        clients_clone.broadcast(msg)
    });
    handle.spawn(sync);


    let client = get_connection(&handle, clients);

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
