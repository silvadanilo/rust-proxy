extern crate futures;
extern crate tokio_core;
extern crate tokio_line;
#[macro_use]
extern crate log;
extern crate env_logger;

use log::LogLevel;

use futures::Async::{NotReady, Ready};
use futures::Async;
use futures::future;
use futures::future::{ok, loop_fn, Future, FutureResult, Loop};
use futures::sync::mpsc::UnboundedReceiver;
use futures::sync::mpsc;
use futures::{finished, Stream, Sink, stream};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::io::{Read, Write, ErrorKind, Error};
use std::net::SocketAddr;
use std::rc::Rc;
use std::{io, str};
use tokio_core::io::{Io, Codec, EasyBuf, Framed};
use tokio_core::net::{TcpStream, TcpListener};
use tokio_core::reactor::{Core, Handle};
use tokio_line::LineCodec;

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

fn get_connection(handle: &Handle) -> Box<Future<Item = (), Error = io::Error>> {
    let remote_addr = "127.0.0.1:9876".parse().unwrap();
    let tcp = TcpStream::connect(&remote_addr, handle);

    let client = tcp.and_then(|stream| {
        let (sink, from_server) = stream.framed(LineCodec).split();
        let reader = from_server.for_each(|message| {
            println!("{}", message);
            Ok(())
        });

        reader.map(|_| {
            println!("CLIENT DISCONNECTED");
            ()
        })
    });

    let client = client.or_else(|_| {
        println!("connection refuse");
        Ok(())
    });

    Box::new(client)
}

fn main() {
    env_logger::init().unwrap();

    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let handle_clone = handle.clone();
    let client = future::loop_fn((), move |_| {
        // Run the get_connection function and loop again regardless of its result
        get_connection(&handle_clone)
            .map(|_| -> Loop<(), ()> {
                Loop::Continue(())
            })
    });

    core.run(client).unwrap();
}
