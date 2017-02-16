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
use std::io::{Read, Write, ErrorKind};
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

fn get_connection(
    handle: &Handle,
    connToServer: ConnectedClients
) -> Box<Future<Item = (), Error = io::Error>> {
    let remote_addr = "127.0.0.1:9876".parse().unwrap();
    let tcp = TcpStream::connect(&remote_addr, handle);

    let handle_clone = handle.clone();
    let mut i: u32 = 0; // i is always zero and in my case it's ok because I want to overwrite the last one

    let connToServer_clone = connToServer.clone();

    let client = tcp.and_then(move |stream| {
        let (tx, rx) = mpsc::channel(8);
        connToServer_clone.insert(i, Client::new(tx));

        let (sender, receiver) = stream.framed(LineCodec).split();
        let reader = receiver.for_each(|message| {
            println!("{}", message);
            Ok(())
        })
        .map(|_| -> Result<(), ()> {
            Ok(())
        })
        .map_err(|_| -> Result<(), ()> {
            Ok(())
        });

        // let writer = rx.forward(sender)
        //     .map(|_| ())
        //     .map_err(|_| io::Error::new(io::ErrorKind::Other, "fail to forward"));

    let connToServer_clone = connToServer.clone();
        let writer = rx
            .map_err(|()| unreachable!("rx can't fail"))
            .fold(sender, |sender, msg| {
                sender.send(msg).map_err(|_| ())
            })
            .map(|_| -> Result<(), ()> {
                Ok(())
            })
            .map_err(move |_| -> Result<(), ()> {
                Ok(())
            });
        ;

        reader.select(writer)
            .map(|_| {
                println!("CLIENT DISCONNECTED");
                ()
            })
            .map_err(|_| {
                io::Error::new(io::ErrorKind::Other, "client disconnected")
            })
    });

    let client = client
        .or_else(|_| {
            println!("connection refuse");
            Ok(())
            // Err(io::Error::new(io::ErrorKind::Other, "connection refuse"))
        });

    Box::new(client)
}

#[derive(Clone)]
struct Buffer {
    remote_server_is_ready: bool,
    remote_tx: Option<mpsc::Sender<String>>,
}

impl Buffer {
    fn ready(&mut self) {
        self.remote_server_is_ready = true;
    }
}

use futures::{AsyncSink, StartSend, Poll};
use futures::sync::mpsc::{SendError};

impl Sink for Buffer {
    type SinkItem = String;
    type SinkError = SendError<()>;

    fn start_send(&mut self, msg: String) -> StartSend<String, SendError<()>> {
        if !self.remote_server_is_ready {
            println!("not ready");
            return Ok(AsyncSink::NotReady(msg));
        }

        println!("{}", msg);

        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), SendError<()>> {
        println!("complete");
        Ok(Async::Ready(()))
    }
}

use tokio_core::reactor::Timeout;
use std::time::Duration;
use futures::IntoFuture;

fn main() {
    env_logger::init().unwrap();

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let mut buffer = Buffer {
        remote_server_is_ready: false,
        remote_tx: None,
    };
    let buffer_cloned = buffer.clone();

    let t = Timeout::new(Duration::new(5, 0), &handle).into_future().flatten();
    let f = t.and_then(move |_| {
        println!("TIMEOUT");
        buffer.ready();
        Ok(())
    });

    let f = f.map_err(|_| panic!());
    handle.spawn(f);

    let s = buffer_cloned.send("prova".to_string());

    core.run(s).unwrap();

    return;


    let clients = ConnectedClients::new();
    let (tx, rx) = mpsc::unbounded();
    // let rx = rx.map_err(|_| panic!()); // errors not possible on rx

    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let handle_clone = handle.clone();
    let clients_clone = clients.clone();
    let client = future::loop_fn((), move |_| {
        // Run the get_connection function and loop again regardless of its result
        get_connection(&handle_clone, clients_clone.clone())
            .map(|_| -> Loop<(), ()> {
                Loop::Continue(())
            })
            // .map_err(|_| -> Loop<(), ()> { //unuseful :(
            //     Loop::Continue(())
            // })
    });

    let client = client.map_err(|_| panic!());
    handle.spawn(client);
    // core.run(client).unwrap();


    let address = "0.0.0.0:12345".parse().unwrap();
    let listener = TcpListener::bind(&address, &core.handle()).unwrap();
    let connections = listener.incoming();

    let clients_clone = clients.clone();
    let sync = rx.for_each(move |msg| {
        clients_clone.broadcast(msg)
    });
    handle.spawn(sync);

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
