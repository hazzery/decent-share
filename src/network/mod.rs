mod client;
mod event_loop;

use std::{
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
    time::Duration,
};

use futures::{channel::mpsc, prelude::*};
use libp2p::{
    gossipsub, identity, kad, mdns, noise,
    request_response::{self, ProtocolSupport},
    swarm::NetworkBehaviour,
    tcp, yamux, StreamProtocol,
};
use serde::{Deserialize, Serialize};
use tokio::io::{Error as TokioError, ErrorKind as TokioErrorKind};

pub(crate) use client::Client;
pub(crate) use event_loop::{Event, EventLoop};

#[derive(NetworkBehaviour)]
struct Behaviour {
    request_response: request_response::cbor::Behaviour<FileRequest, FileResponse>,
    file_trading: request_response::cbor::Behaviour<TradeOffer, TradeResponse>,
    direct_messaging: request_response::cbor::Behaviour<DirectMessage, DirectMessageResponse>,
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

// Simple file exchange protocol
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FileRequest(String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FileResponse(Vec<u8>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TradeOffer(String, String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TradeResponse(Option<Vec<u8>>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DirectMessage(String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DirectMessageResponse();

/// Creates the network components, namely:
///
/// - The network client to interact with the network layer from anywhere within your application.
///
/// - The network event stream, e.g. for incoming requests.
///
/// - The network task driving the network itself.
pub(crate) fn new(
    secret_key_seed: Option<u8>,
) -> Result<(Client, impl Stream<Item = Event>, EventLoop), anyhow::Error> {
    // Create a public/private key pair, either random or based on a seed.
    let id_keys = match secret_key_seed {
        Some(seed) => {
            let mut bytes = [0u8; 32];
            bytes[0] = seed;
            identity::Keypair::ed25519_from_bytes(bytes).unwrap()
        }
        None => identity::Keypair::generate_ed25519(),
    };
    let peer_id = id_keys.public().to_peer_id();

    // Set a custom gossipsub configuration
    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
        .validation_mode(gossipsub::ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message
        // signing)
        .message_id_fn(|message: &gossipsub::Message| {
            let mut s = DefaultHasher::new();
            message.data.hash(&mut s);
            gossipsub::MessageId::from(s.finish().to_string())
        }) // content-address messages. No two messages of the same content will be propagated.
        .build()
        .map_err(|msg| TokioError::new(TokioErrorKind::Other, msg))?; // Temporary hack because `build` does not return a proper `std::error::Error`.

    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_quic()
        .with_behaviour(|keypair: &identity::Keypair| {
            Ok(Behaviour {
                kademlia: kad::Behaviour::new(
                    peer_id,
                    kad::store::MemoryStore::new(keypair.public().to_peer_id()),
                ),
                request_response: request_response::cbor::Behaviour::new(
                    [(
                        StreamProtocol::new("/file-exchange/1"),
                        ProtocolSupport::Full,
                    )],
                    request_response::Config::default(),
                ),
                file_trading: request_response::cbor::Behaviour::new(
                    [(
                        StreamProtocol::new("/file-trade/1"),
                        ProtocolSupport::Full,
                    )],
                    request_response::Config::default(),
                ),
                direct_messaging: request_response::cbor::Behaviour::new(
                    [(
                        StreamProtocol::new("/direct-message/1"),
                        ProtocolSupport::Full,
                    )],
                    request_response::Config::default(),
                ),
                gossipsub: gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(keypair.clone()),
                    gossipsub_config,
                )?,
                mdns: mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    keypair.public().to_peer_id(),
                )?,
            })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    swarm
        .behaviour_mut()
        .kademlia
        .set_mode(Some(kad::Mode::Server));

    let (command_sender, command_receiver) = mpsc::channel(0);
    let (event_sender, event_receiver) = mpsc::channel(0);

    let topic = gossipsub::IdentTopic::new("chat-room");
    swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

    // Listen on all interfaces and whatever port the OS assigns
    swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    Ok((
        Client {
            sender: command_sender,
            username_peer_id_map: Arc::default(),
        },
        event_receiver,
        EventLoop::new(swarm, command_receiver, event_sender, topic),
    ))
}
