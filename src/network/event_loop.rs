mod handlers;

use std::{
    collections::{hash_map, HashMap, HashSet},
    error::Error,
};

use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};
use libp2p::{
    core::Multiaddr,
    gossipsub, kad, mdns,
    multiaddr::Protocol,
    request_response::{self, OutboundRequestId, ResponseChannel},
    swarm::{Swarm, SwarmEvent},
    PeerId,
};

use super::{Behaviour, BehaviourEvent, FileRequest, FileResponse};

type DynResult<T> = Result<T, Box<dyn Error + Send>>;

pub(crate) struct EventLoop {
    swarm: Swarm<Behaviour>,
    command_receiver: mpsc::Receiver<Command>,
    event_sender: mpsc::Sender<Event>,
    pending_dial: HashMap<PeerId, oneshot::Sender<DynResult<()>>>,
    pending_start_providing: HashMap<kad::QueryId, oneshot::Sender<()>>,
    pending_get_providers: HashMap<kad::QueryId, oneshot::Sender<HashSet<PeerId>>>,
    pending_request_file: HashMap<OutboundRequestId, oneshot::Sender<DynResult<Vec<u8>>>>,
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
            pending_dial: HashMap::default(),
            pending_start_providing: HashMap::default(),
            pending_get_providers: HashMap::default(),
            pending_request_file: HashMap::default(),
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

            SwarmEvent::NewListenAddr { address, .. } => self.handle_new_listen_address(address),

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
                message_id: id,
                message,
            })) => self.handle_gossipsub_message(&message, &id, &peer_id),

            SwarmEvent::Dialing {
                peer_id: Some(peer_id),
                ..
            } => eprintln!("Dialing {peer_id}"),

            SwarmEvent::Behaviour(
                BehaviourEvent::Kademlia(_)
                | BehaviourEvent::RequestResponse(request_response::Event::ResponseSent { .. })
                | BehaviourEvent::Gossipsub(gossipsub::Event::Subscribed { .. }),
            )
            | SwarmEvent::IncomingConnection { .. }
            | SwarmEvent::ConnectionClosed { .. }
            | SwarmEvent::IncomingConnectionError { .. }
            | SwarmEvent::NewExternalAddrOfPeer { .. } => {}

            e => panic!("{e:?}"),
        }
    }

    fn handle_command(&mut self, command: Command) {
        match command {
            Command::StartListening { addr, sender } => {
                let _ = match self.swarm.listen_on(addr) {
                    Ok(_) => sender.send(Ok(())),
                    Err(e) => sender.send(Err(Box::new(e))),
                };
            }
            Command::RegisterName { username } => {
                let key = kad::RecordKey::new(&self.swarm.local_peer_id().to_bytes());
                let record = kad::Record {
                    key,
                    value: username.into_bytes(),
                    publisher: None,
                    expires: None,
                };
                self.swarm
                    .behaviour_mut()
                    .kademlia
                    .put_record(record, kad::Quorum::One)
                    .expect("Failed to store record locally");
            }
            Command::Dial {
                peer_id,
                peer_addr,
                sender,
            } => {
                if let hash_map::Entry::Vacant(e) = self.pending_dial.entry(peer_id) {
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, peer_addr.clone());
                    match self.swarm.dial(peer_addr.with(Protocol::P2p(peer_id))) {
                        Ok(()) => {
                            e.insert(sender);
                        }
                        Err(e) => {
                            let _ = sender.send(Err(Box::new(e)));
                        }
                    }
                } else {
                    todo!("Already dialing peer.");
                }
            }
            Command::StartProviding { file_name, sender } => {
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .start_providing(file_name.into_bytes().into())
                    .expect("No store error.");
                self.pending_start_providing.insert(query_id, sender);
            }
            Command::GetProviders { file_name, sender } => {
                let query_id = self
                    .swarm
                    .behaviour_mut()
                    .kademlia
                    .get_providers(file_name.into_bytes().into());
                self.pending_get_providers.insert(query_id, sender);
            }
            Command::RequestFile {
                file_name,
                peer,
                sender,
            } => {
                let request_id = self
                    .swarm
                    .behaviour_mut()
                    .request_response
                    .send_request(&peer, FileRequest(file_name));
                self.pending_request_file.insert(request_id, sender);
            }
            Command::RespondFile { file, channel } => {
                self.swarm
                    .behaviour_mut()
                    .request_response
                    .send_response(channel, FileResponse(file))
                    .expect("Connection to peer to be still open.");
            }
            Command::SendMessage { message } => {
                self.swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(self.gossipsub_topic.clone(), message.as_bytes())
                    .expect("Message Publish error!");
            }
        }
    }
}

#[derive(Debug)]
pub(super) enum Command {
    StartListening {
        addr: Multiaddr,
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
    },
    RegisterName {
        username: String,
    },
    Dial {
        peer_id: PeerId,
        peer_addr: Multiaddr,
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
    },
    StartProviding {
        file_name: String,
        sender: oneshot::Sender<()>,
    },
    GetProviders {
        file_name: String,
        sender: oneshot::Sender<HashSet<PeerId>>,
    },
    RequestFile {
        file_name: String,
        peer: PeerId,
        sender: oneshot::Sender<Result<Vec<u8>, Box<dyn Error + Send>>>,
    },
    RespondFile {
        file: Vec<u8>,
        channel: ResponseChannel<FileResponse>,
    },
    SendMessage {
        message: String,
    },
}
#[derive(Debug)]
pub(crate) enum Event {
    InboundRequest {
        request: String,
        channel: ResponseChannel<FileResponse>,
    },
}
