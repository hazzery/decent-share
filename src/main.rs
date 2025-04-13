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
mod network;

use action::{handle_get, handle_provide, handle_send};
use clap::Parser;
use tokio::io::AsyncBufReadExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    let opt = Opt::parse();

    let (mut network_client, mut network_events, network_event_loop) =
        network::new(opt.secret_key_seed)?;

    // Spawn the network task for it to run in the background.
    tokio::task::spawn(network_event_loop.run());

    network_client.register_username(opt.username).await;

    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    loop {
        print!("Enter command:\n>");
        let Ok(Some(command)) = stdin.next_line().await else {
            continue;
        };

        let arguments = split_string(&command);
        let Some(action) = arguments.first() else {
            println!("No action specified");
            continue;
        };

        match action.as_str() {
            "Send" => {
                let Some(message) = arguments.get(1) else {
                    println!("Missing message content");
                    continue;
                };
                handle_send(message, &mut network_client).await;
            }
            "Provide" => {
                let Some(file_path) = arguments.get(1) else {
                    println!("Missing file path and name for file");
                    continue;
                };
                let Some(name) = arguments.get(2) else {
                    println!("Missing name for file");
                    continue;
                };
                handle_provide(name, file_path, &mut network_client, &mut network_events).await?;
            }
            "Get" => {
                let Some(name) = arguments.get(1) else {
                    println!("Missing name for file to recieve");
                    continue;
                };
                handle_get(name, &mut network_client).await?;
            }
            "Register" => {
                let Some(username) = arguments.get(1) else {
                    println!("Missing username");
                    continue;
                };

                network_client.register_username(username.clone()).await;
            }
            "Find" => {
                let Some(username) = arguments.get(1) else {
                    println!("Missing username");
                    continue;
                };
                network_client.find_user(username.clone()).await;
            }
            "Dm" => {
                let Some(username) = arguments.get(1) else {
                    println!("Missing username and message");
                    continue;
                };
                let Some(message) = arguments.get(2) else {
                    println!("Missing message");
                    continue;
                };
                network_client
                    .direct_message(username.clone(), message.clone())
                    .await
                    .expect("Direct message failed");
            }

            action => println!("Unknown action {action}"),
        };
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

fn split_string(input: &str) -> Vec<String> {
    let re = regex::Regex::new(r#""([^"]*)"|\S+"#).unwrap();
    re.captures_iter(input)
        .map(|cap| cap.get(0).unwrap().as_str().to_string())
        .collect()
}
