mod client;
mod event_loop;
mod username_store;

use std::{hash::Hash, sync::Arc, time::Duration};

use futures::{channel::mpsc, Stream};
use libp2p::{
    gossipsub, identify, identity, kad, noise, rendezvous,
    request_response::{self, ProtocolSupport},
    swarm::NetworkBehaviour,
    tcp, yamux, Multiaddr, PeerId, StreamProtocol,
};
use serde::{Deserialize, Serialize};
use tokio::io::{Error as TokioError, ErrorKind as TokioErrorKind};

pub(crate) use client::Client;
pub(crate) use event_loop::{Event, EventLoop};

const RENDEZVOUS_POINT_PORT_NUMBER: u16 = 62649;
pub const RENDEZVOUS_POINT_PEER_ID: &str = "12D3KooWDpJ7As7BWAwRMfu1VU2WCqNjvq387JEYKDBj4kx6nXTN";

#[derive(NetworkBehaviour)]
struct Behaviour {
    trade_offering: request_response::cbor::Behaviour<TradeOffer, NoResponse>,
    trade_response: request_response::cbor::Behaviour<TradeResponse, TradeResponseResponse>,
    direct_messaging: request_response::cbor::Behaviour<DirectMessage, NoResponse>,
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    gossipsub: gossipsub::Behaviour,
    rendezvous: rendezvous::client::Behaviour,
    identify: identify::Behaviour,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub(crate) struct TradeOffer {
    offered_file_name: String,
    requested_file_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TradeResponse {
    requested_file_name: String,
    offered_file_name: String,
    requested_file_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TradeResponseResponse {
    offered_file_name: String,
    requested_file_name: String,
    offered_file_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DirectMessage(String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct NoResponse();

/// Creates the network components, namely:
///
/// - The network client to interact with the network layer from anywhere within your application.
///
/// - The network event stream, e.g. for incoming requests.
///
/// - The network task driving the network itself.
pub(crate) fn new(
    username: String,
    rendezvous_ip_address: &str,
) -> Result<(Client, impl Stream<Item = Event>, EventLoop), anyhow::Error> {
    // Create a public/private key pair, either random or based on a seed.
    let id_keys = identity::Keypair::generate_ed25519();
    let peer_id = id_keys.public().to_peer_id();

    // Set a custom gossipsub configuration
    let gossipsub_config = gossipsub::ConfigBuilder::default()
        // This is set to aid debugging by not cluttering the log space
        .heartbeat_interval(Duration::from_secs(10))
        // This sets the kind of message validation. The default is Strict (enforce message signing)
        .validation_mode(gossipsub::ValidationMode::Strict)
        .build()
        // Temporary hack because `build` does not return a proper `std::error::Error`.
        .map_err(|msg| TokioError::new(TokioErrorKind::Other, msg))?;

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
                trade_offering: request_response::cbor::Behaviour::new(
                    [(StreamProtocol::new("/trade-offer/1"), ProtocolSupport::Full)],
                    request_response::Config::default(),
                ),
                trade_response: request_response::cbor::Behaviour::new(
                    [(
                        StreamProtocol::new("/trade-response/1"),
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
                rendezvous: rendezvous::client::Behaviour::new(keypair.clone()),
                identify: identify::Behaviour::new(identify::Config::new(
                    "rendezvous-identify/1.0.0".to_string(),
                    keypair.public(),
                )),
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

    let rendezvous_peer_id: PeerId = RENDEZVOUS_POINT_PEER_ID.parse()?;

    let rendezvous_multi_address: Multiaddr =
        format!("/ip4/{rendezvous_ip_address}/tcp/{RENDEZVOUS_POINT_PORT_NUMBER}").parse()?;
    swarm.dial(rendezvous_multi_address)?;

    Ok((
        Client {
            command_sender,
            username_store: Arc::default(),
        },
        event_receiver,
        EventLoop::new(
            swarm,
            command_receiver,
            event_sender,
            topic,
            username,
            rendezvous_peer_id,
        ),
    ))
}
