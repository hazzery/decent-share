// Copyright 2021 Protocol Labs.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

mod action;
mod interface;
mod network;

use clap::Parser;
use futures::StreamExt;
use tokio::io::AsyncBufReadExt;
use tracing_subscriber::EnvFilter;

use interface::{handle_network_event, handle_std_in};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Subscribe to the logging output by libp2p
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::default())
        .try_init();

    let arguments = Arguments::parse();

    let (mut network_client, mut network_events, network_event_loop) =
        network::new(arguments.username, arguments.rendezvous_address)?;

    // Spawn the network task for it to run in the background
    tokio::task::spawn(network_event_loop.run());

    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();

    // listen for user actions on stdin and events from the network
    loop {
        tokio::select! {
            command = stdin.next_line() => handle_std_in(command, &mut network_client).await,
            event = network_events.next() => handle_network_event(event, &mut network_client).await,
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "decent-share: File exchange")]
struct Arguments {
    /// A username to register with for user identification.
    #[arg(long, short)]
    username: String,

    /// The IP address of the rendezvous server.
    #[arg(long, short)]
    rendezvous_address: Option<String>,
}
