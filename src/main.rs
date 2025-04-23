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
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(tracing_subscriber::filter::LevelFilter::WARN.into())
                .from_env_lossy(),
        )
        .try_init();

    let opt = Opt::parse();

    let (mut network_client, mut network_events, network_event_loop) =
        network::new(opt.username, opt.secret_key_seed)?;

    // Spawn the network task for it to run in the background.
    tokio::task::spawn(network_event_loop.run());

    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    loop {
        tokio::select! {
            command = stdin.next_line() => handle_std_in(command, &mut network_client).await,
            event = network_events.next() => handle_network_event(event, &mut network_client).await,
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "libp2p file sharing example")]
struct Opt {
    /// Fixed value to generate deterministic peer ID.
    #[arg(long)]
    secret_key_seed: Option<u8>,

    #[arg(long, short)]
    username: String,
}
