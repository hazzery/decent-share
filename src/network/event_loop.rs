mod behaviour_handlers;
mod command;
mod command_handlers;

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::Duration,
};

use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};
use libp2p::{
    gossipsub, identify, kad, rendezvous, request_response,
    swarm::{Swarm, SwarmEvent},
    PeerId,
};

use super::{Behaviour, BehaviourEvent, DirectMessage, TradeOffer, TradeResponse};

pub(super) use command::Command;

type DynResult<T> = Result<T, anyhow::Error>;

const RENDEZVOUS_NAMESPACE: &str = "rendezvous";

pub(crate) struct EventLoop {
    swarm: Swarm<Behaviour>,
    rendezvous_peer_id: PeerId,
    command_receiver: mpsc::Receiver<Command>,
    event_sender: mpsc::Sender<Event>,
    pending_register_username:
        HashMap<kad::QueryId, oneshot::Sender<Result<(), kad::PutRecordError>>>,
    pending_request_message:
        HashMap<request_response::OutboundRequestId, oneshot::Sender<DynResult<()>>>,
    pending_peer_id_request: HashMap<kad::QueryId, oneshot::Sender<Option<PeerId>>>,
    pending_username_request: HashMap<kad::QueryId, oneshot::Sender<DynResult<String>>>,
    pending_trade_offer_request:
        HashMap<request_response::OutboundRequestId, oneshot::Sender<DynResult<()>>>,
    pending_trade_response_response:
        HashMap<request_response::OutboundRequestId, oneshot::Sender<DynResult<Option<Vec<u8>>>>>,
    outgoing_trade_offers: HashMap<(PeerId, TradeOffer), (Vec<u8>, PathBuf)>,
    inbound_trade_offers: HashSet<(PeerId, TradeOffer)>,
    gossipsub_topic: gossipsub::IdentTopic,
    discover_tick: tokio::time::Interval,
    cookie: Option<rendezvous::Cookie>,
    rendezvous_namespace: rendezvous::Namespace,
}

impl EventLoop {
    pub(super) fn new(
        swarm: Swarm<Behaviour>,
        command_receiver: mpsc::Receiver<Command>,
        event_sender: mpsc::Sender<Event>,
        gossipsub_topic: gossipsub::IdentTopic,
        rendezvous_peer_id: PeerId,
    ) -> Self {
        Self {
            swarm,
            rendezvous_peer_id,
            command_receiver,
            event_sender,
            pending_register_username: HashMap::default(),
            pending_request_message: HashMap::default(),
            pending_peer_id_request: HashMap::default(),
            pending_username_request: HashMap::default(),
            pending_trade_offer_request: HashMap::default(),
            pending_trade_response_response: HashMap::default(),
            outgoing_trade_offers: HashMap::default(),
            inbound_trade_offers: HashSet::default(),
            gossipsub_topic,
            discover_tick: tokio::time::interval(Duration::from_secs(30)),
            cookie: None,
            rendezvous_namespace: rendezvous::Namespace::from_static(RENDEZVOUS_NAMESPACE),
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
                _ = self.discover_tick.tick(), if self.cookie.is_some() => {
                    self.swarm
                        .behaviour_mut()
                        .rendezvous
                        .discover(
                            Some(self.rendezvous_namespace.clone()),
                            self.cookie.clone(),
                            None,
                            self.rendezvous_peer_id
                        );
                }
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
                    id: query_id,
                    ..
                },
            )) => self.handle_put_record(record, query_id),

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
                request_response::Event::OutboundFailure {
                    error, request_id, ..
                },
            )) => self.handle_trade_offering_outbound_failure(error, request_id),

            SwarmEvent::Behaviour(BehaviourEvent::TradeResponse(
                request_response::Event::Message { peer, message, .. },
            )) => self.handle_trade_response_message(message, peer).await,

            SwarmEvent::Behaviour(BehaviourEvent::TradeResponse(
                request_response::Event::OutboundFailure {
                    request_id, error, ..
                },
            )) => self.handle_trade_response_outbound_failure(request_id, error),

            SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(gossipsub::Event::Message {
                propagation_source: peer_id,
                message,
                ..
            })) => self.handle_gossipsub_message(&message, peer_id).await,

            SwarmEvent::ConnectionEstablished { peer_id, .. }
                if peer_id == self.rendezvous_peer_id =>
            {
                self.handle_connected_to_rendezvous_server();
            }

            SwarmEvent::Behaviour(BehaviourEvent::Rendezvous(
                rendezvous::client::Event::Discovered {
                    registrations,
                    cookie,
                    ..
                },
            )) => self.handle_rendezvous_discovered(registrations, cookie),

            SwarmEvent::Behaviour(BehaviourEvent::Identify(identify::Event::Received {
                info,
                ..
            })) => self.handle_identify_received(info),

            SwarmEvent::Behaviour(
                BehaviourEvent::Kademlia(_)
                | BehaviourEvent::Identify(
                    identify::Event::Sent { .. } | identify::Event::Pushed { .. },
                )
                | BehaviourEvent::Gossipsub(
                    gossipsub::Event::GossipsubNotSupported { .. }
                    | gossipsub::Event::Subscribed { .. },
                )
                | BehaviourEvent::Rendezvous(rendezvous::client::Event::Registered { .. })
                | BehaviourEvent::DirectMessaging(request_response::Event::ResponseSent { .. })
                | BehaviourEvent::TradeOffering(request_response::Event::ResponseSent { .. })
                | BehaviourEvent::TradeResponse(request_response::Event::ResponseSent { .. }),
            )
            | SwarmEvent::Dialing { .. }
            | SwarmEvent::IncomingConnection { .. }
            | SwarmEvent::ConnectionClosed { .. }
            | SwarmEvent::IncomingConnectionError { .. }
            | SwarmEvent::ConnectionEstablished { .. }
            | SwarmEvent::NewExternalAddrOfPeer { .. }
            | SwarmEvent::OutgoingConnectionError { .. }
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
