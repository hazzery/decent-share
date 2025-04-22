mod behaviour_handlers;
mod command;
mod command_handlers;

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};
use libp2p::{
    gossipsub, kad, mdns, request_response,
    swarm::{Swarm, SwarmEvent},
    PeerId,
};

use super::{Behaviour, BehaviourEvent, DirectMessage, TradeOffer, TradeResponse};

pub(super) use command::Command;

type DynResult<T> = Result<T, anyhow::Error>;

pub(crate) struct EventLoop {
    swarm: Swarm<Behaviour>,
    command_receiver: mpsc::Receiver<Command>,
    event_sender: mpsc::Sender<Event>,
    peer_id_username_map: HashMap<PeerId, String>,
    pending_dial: HashMap<PeerId, oneshot::Sender<DynResult<()>>>,
    pending_request_message:
        HashMap<request_response::OutboundRequestId, oneshot::Sender<DynResult<()>>>,
    pending_name_request: HashMap<kad::QueryId, oneshot::Sender<Option<PeerId>>>,
    pending_username_request: HashMap<kad::QueryId, oneshot::Sender<DynResult<String>>>,
    pending_trade_response_response:
        HashMap<request_response::OutboundRequestId, oneshot::Sender<DynResult<Option<Vec<u8>>>>>,
    outgoing_trade_offers: HashMap<(PeerId, TradeOffer), (Vec<u8>, PathBuf)>,
    inbound_trade_offers: HashSet<(PeerId, TradeOffer)>,
    gossipsub_topic: gossipsub::IdentTopic,
}

impl EventLoop {
    pub(super) fn new(
        swarm: Swarm<Behaviour>,
        command_receiver: mpsc::Receiver<Command>,
        event_sender: mpsc::Sender<Event>,
        gossipsub_topic: gossipsub::IdentTopic,
    ) -> Self {
        Self {
            swarm,
            command_receiver,
            event_sender,
            peer_id_username_map: HashMap::default(),
            pending_dial: HashMap::default(),
            pending_request_message: HashMap::default(),
            pending_name_request: HashMap::default(),
            pending_username_request: HashMap::default(),
            pending_trade_response_response: HashMap::default(),
            outgoing_trade_offers: HashMap::default(),
            inbound_trade_offers: HashSet::default(),
            gossipsub_topic,
        }
    }

    pub(crate) async fn run(mut self) {
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => self.handle_event(event).await,
                command = self.command_receiver.next() => match command {
                    Some(c) => self.handle_command(c),
                    // Command channel closed, thus shutting down the network event loop.
                    None => return,
                },
            }
        }
    }

    async fn handle_event(&mut self, event: SwarmEvent<BehaviourEvent>) {
        match event {
            SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    id,
                    result: kad::QueryResult::GetRecord(record),
                    ..
                },
            )) => self.handle_get_record(record, id),

            SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    result: kad::QueryResult::PutRecord(record),
                    ..
                },
            )) => self.handle_put_record(record),

            SwarmEvent::Behaviour(BehaviourEvent::DirectMessaging(
                request_response::Event::Message { peer, message, .. },
            )) => self.handle_direct_messaging_message(message, peer).await,

            SwarmEvent::Behaviour(BehaviourEvent::DirectMessaging(
                request_response::Event::OutboundFailure {
                    request_id, error, ..
                },
            )) => self.handle_direct_messaging_outbound_failure(request_id, error),

            SwarmEvent::Behaviour(BehaviourEvent::TradeOffering(
                request_response::Event::Message { peer, message, .. },
            )) => self.handle_trade_offering_message(message, peer).await,

            SwarmEvent::Behaviour(BehaviourEvent::TradeOffering(
                request_response::Event::OutboundFailure { error, .. },
            )) => self.handle_trade_offering_outbound_failure(&error),

            SwarmEvent::Behaviour(BehaviourEvent::TradeResponse(
                request_response::Event::Message { peer, message, .. },
            )) => self.handle_trade_response_message(message, peer).await,

            SwarmEvent::Behaviour(BehaviourEvent::TradeResponse(
                request_response::Event::OutboundFailure {
                    request_id, error, ..
                },
            )) => self.handle_trade_response_outbound_failure(request_id, error),

            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => self.handle_connection_established(&peer_id, &endpoint),

            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                self.handle_outgoing_connection_error(peer_id, error);
            }

            SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                self.handle_mdns_discovered(list);
            }

            SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                self.handle_mdns_expired(&list);
            }

            SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(gossipsub::Event::Message {
                propagation_source: peer_id,
                message,
                ..
            })) => self.handle_gossipsub_message(&message, peer_id).await,

            SwarmEvent::Behaviour(
                BehaviourEvent::Kademlia(_)
                | BehaviourEvent::DirectMessaging(request_response::Event::ResponseSent { .. })
                | BehaviourEvent::TradeOffering(request_response::Event::ResponseSent { .. })
                | BehaviourEvent::TradeResponse(request_response::Event::ResponseSent { .. })
                | BehaviourEvent::Gossipsub(gossipsub::Event::Subscribed { .. }),
            )
            | SwarmEvent::Dialing { .. }
            | SwarmEvent::IncomingConnection { .. }
            | SwarmEvent::ConnectionClosed { .. }
            | SwarmEvent::IncomingConnectionError { .. }
            | SwarmEvent::NewExternalAddrOfPeer { .. }
            | SwarmEvent::NewListenAddr { .. } => {}

            event => println!("{event:?}"),
        }
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub(crate) enum Event {
    InboundTradeOffer {
        offered_file_name: String,
        peer_id: PeerId,
        requested_file_name: String,
    },
    InboundTradeResponse {
        peer_id: PeerId,
        offered_file_name: String,
        requested_file_name: String,
        was_accepted: bool,
    },
    InboundDirectMessage {
        peer_id: PeerId,
        message: String,
    },
    InboundChat {
        peer_id: PeerId,
        message: String,
    },
}
