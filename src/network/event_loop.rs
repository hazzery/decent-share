mod behaviour_handlers;
mod command;
mod command_handlers;

use std::collections::{HashMap, HashSet};

use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};
use libp2p::{
    gossipsub, kad, mdns,
    request_response::{self, OutboundRequestId, ResponseChannel},
    swarm::{Swarm, SwarmEvent},
    PeerId,
};

use super::{
    username_store::UsernameStore, Behaviour, BehaviourEvent, DirectMessage, FileRequest,
    FileResponse,
};

pub(super) use command::Command;

type DynResult<T> = Result<T, anyhow::Error>;

pub(crate) struct EventLoop {
    swarm: Swarm<Behaviour>,
    command_receiver: mpsc::Receiver<Command>,
    event_sender: mpsc::Sender<Event>,
    username_store: UsernameStore,
    pending_dial: HashMap<PeerId, oneshot::Sender<DynResult<()>>>,
    pending_start_providing: HashMap<kad::QueryId, oneshot::Sender<()>>,
    pending_get_providers: HashMap<kad::QueryId, oneshot::Sender<HashSet<PeerId>>>,
    pending_request_file: HashMap<OutboundRequestId, oneshot::Sender<DynResult<Vec<u8>>>>,
    pending_request_message: HashMap<OutboundRequestId, oneshot::Sender<DynResult<()>>>,
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
            username_store: UsernameStore::default(),
            pending_dial: HashMap::default(),
            pending_start_providing: HashMap::default(),
            pending_get_providers: HashMap::default(),
            pending_request_file: HashMap::default(),
            pending_request_message: HashMap::default(),
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
                    result: kad::QueryResult::StartProviding(_),
                    ..
                },
            )) => self.handle_pending_start_providing(id),

            SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    id,
                    result: kad::QueryResult::GetProviders(providers),
                    ..
                },
            )) => self.handle_found_providers(id, providers),

            SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    result: kad::QueryResult::GetRecord(record),
                    ..
                },
            )) => self.handle_get_record(record),

            SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    result: kad::QueryResult::PutRecord(record),
                    ..
                },
            )) => self.handle_put_record(record),

            SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(
                request_response::Event::Message { message, .. },
            )) => self.handle_request_response_message(message).await,

            SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(
                request_response::Event::OutboundFailure {
                    request_id, error, ..
                },
            )) => self.handle_request_response_outbound_failure(request_id, error),

            SwarmEvent::Behaviour(BehaviourEvent::DirectMessaging(
                request_response::Event::Message { peer, message, .. },
            )) => self.handle_direct_messaging_message(message, peer),

            SwarmEvent::Behaviour(BehaviourEvent::DirectMessaging(
                request_response::Event::OutboundFailure {
                    request_id, error, ..
                },
            )) => self.handle_direct_messaging_outbound_failure(request_id, error),

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
            })) => self.handle_gossipsub_message(&message, &peer_id),

            SwarmEvent::Behaviour(
                BehaviourEvent::Kademlia(_)
                | BehaviourEvent::RequestResponse(request_response::Event::ResponseSent { .. })
                | BehaviourEvent::DirectMessaging(request_response::Event::ResponseSent { .. })
                | BehaviourEvent::Gossipsub(gossipsub::Event::Subscribed { .. }),
            )
            | SwarmEvent::Dialing { .. }
            | SwarmEvent::IncomingConnection { .. }
            | SwarmEvent::ConnectionClosed { .. }
            | SwarmEvent::IncomingConnectionError { .. }
            | SwarmEvent::NewExternalAddrOfPeer { .. }
            | SwarmEvent::NewListenAddr { .. } => {}

            e => panic!("{e:?}"),
        }
    }
}

#[derive(Debug)]
pub(crate) enum Event {
    InboundRequest {
        request: String,
        channel: ResponseChannel<FileResponse>,
    },
}
