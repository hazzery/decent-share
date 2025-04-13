use futures::{channel, SinkExt};
use libp2p::{core, gossipsub, kad, request_response, swarm, Multiaddr, PeerId};

use crate::network::{DirectMessage, DirectMessageResponse};

use super::{Event, EventLoop, FileRequest, FileResponse};

impl EventLoop {
    pub(in crate::network::event_loop) fn handle_pending_start_providing(
        &mut self,
        id: kad::QueryId,
    ) {
        let sender: channel::oneshot::Sender<()> = self
            .pending_start_providing
            .remove(&id)
            .expect("Completed query to be previously pending.");
        let _ = sender.send(());
    }

    pub(in crate::network::event_loop) fn handle_found_providers(
        &mut self,
        id: kad::QueryId,
        providers: kad::GetProvidersResult,
    ) {
        match providers {
            Ok(kad::GetProvidersOk::FoundProviders { providers, .. }) => {
                if let Some(sender) = self.pending_get_providers.remove(&id) {
                    sender.send(providers).expect("Receiver not to be dropped");

                    // Finish the query. We are only interested in the first result.
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .query_mut(&id)
                        .unwrap()
                        .finish();
                }
            }
            Ok(_) => {}
            Err(error) => eprintln!("Failed to get providers: {error:?}"),
        }
    }

    #[allow(clippy::unused_self)]
    pub(in crate::network::event_loop) fn handle_get_record(
        &mut self,
        record: kad::GetRecordResult,
    ) {
        match record {
            Ok(kad::GetRecordOk::FoundRecord(kad::PeerRecord {
                record: kad::Record { key, value, .. },
                ..
            })) => {
                let Ok(username) = String::from_utf8(key.to_vec()) else {
                    eprintln!("Record key was not valid UTF-8");
                    return;
                };
                let peer_id = PeerId::from_bytes(&value).unwrap();
                println!("Found peer id for {username}");
                self.username_store.insert(username, peer_id);
            }
            Ok(_) => {}
            Err(error) => eprintln!("Failed to get record: {error:?}"),
        }
    }

    #[allow(clippy::unused_self)]
    pub(in crate::network::event_loop) fn handle_put_record(
        &mut self,
        record: kad::PutRecordResult,
    ) {
        match record {
            Ok(kad::PutRecordOk {..}) => println!("Successfully stored record!"),
            Err(error) => eprintln!("Failed to store record: {error:?}"),
        }
    }

    pub(in crate::network::event_loop) async fn handle_request_response_message(
        &mut self,
        message: request_response::Message<FileRequest, FileResponse>,
    ) {
        match message {
            request_response::Message::Request {
                request, channel, ..
            } => {
                self.event_sender
                    .send(Event::InboundRequest {
                        request: request.0,
                        channel,
                    })
                    .await
                    .expect("Event receiver not to be dropped.");
            }
            request_response::Message::Response {
                request_id,
                response,
            } => {
                let _ = self
                    .pending_request_file
                    .remove(&request_id)
                    .expect("Request to still be pending.")
                    .send(Ok(response.0));
            }
        }
    }

    pub(in crate::network::event_loop) fn handle_request_response_outbound_failure(
        &mut self,
        request_id: request_response::OutboundRequestId,
        error: request_response::OutboundFailure,
    ) {
        let _ = self
            .pending_request_file
            .remove(&request_id)
            .expect("Request to still be pending.")
            .send(Err(Box::new(error)));
    }

    pub(in crate::network::event_loop) fn handle_direct_messaging_message(
        &mut self,
        message: request_response::Message<DirectMessage, DirectMessageResponse>,
        peer: PeerId,
    ) {
        match message {
            request_response::Message::Request {
                request, channel, ..
            } => {
                println!("You have received a direct message:");
                match self.username_store.get_username(&peer) {
                    Some(username) => println!("From {username}: {}", request.0),
                    None => println!("From {peer}: {}", request.0),
                }

                self.swarm
                    .behaviour_mut()
                    .direct_messaging
                    .send_response(channel, DirectMessageResponse())
                    .expect("The read receipt to be sent");
            }
            request_response::Message::Response { request_id, .. } => {
                println!("A direct messaging response has arrived");
                let _ = self
                    .pending_request_message
                    .remove(&request_id)
                    .expect("Message to still be pending.")
                    .send(Ok(()));
            }
        }
    }

    pub(in crate::network::event_loop) fn handle_direct_messaging_outbound_failure(
        &mut self,
        request_id: request_response::OutboundRequestId,
        error: request_response::OutboundFailure,
    ) {
        println!("A direct messaging failure has occured: {error:?}");
        let _ = self
            .pending_request_message
            .remove(&request_id)
            .expect("Message to still be pending.")
            .send(Err(Box::new(error)));
    }

    pub(in crate::network::event_loop) fn handle_connection_established(
        &mut self,
        peer_id: &PeerId,
        endpoint: &core::ConnectedPoint,
    ) {
        if endpoint.is_dialer() {
            if let Some(sender) = self.pending_dial.remove(peer_id) {
                let _ = sender.send(Ok(()));
            }
        }
    }

    pub(in crate::network::event_loop) fn handle_outgoing_connection_error(
        &mut self,
        peer_id: Option<PeerId>,
        error: swarm::DialError,
    ) {
        if let Some(peer_id) = peer_id {
            if let Some(sender) = self.pending_dial.remove(&peer_id) {
                let _ = sender.send(Err(Box::new(error)));
            }
        }
    }

    pub(in crate::network::event_loop) fn handle_mdns_discovered(
        &mut self,
        list: Vec<(PeerId, Multiaddr)>,
    ) {
        for (peer_id, multiaddr) in list {
            self.swarm
                .behaviour_mut()
                .gossipsub
                .add_explicit_peer(&peer_id);

            self.swarm
                .behaviour_mut()
                .kademlia
                .add_address(&peer_id, multiaddr);
        }
    }

    pub(in crate::network::event_loop) fn handle_mdns_expired(
        &mut self,
        list: &Vec<(PeerId, Multiaddr)>,
    ) {
        for (peer_id, _multiaddr) in list {
            self.swarm
                .behaviour_mut()
                .gossipsub
                .remove_explicit_peer(peer_id);
        }
    }
    #[allow(clippy::unused_self)]
    pub(in crate::network::event_loop) fn handle_gossipsub_message(
        &mut self,
        message: &gossipsub::Message,
        peer_id: &PeerId,
    ) {
        let message = String::from_utf8_lossy(&message.data);

        println!("Received new global chat!");

        match self.username_store.get_username(peer_id) {
            Some(username) => println!("From {username}: '{message}'"),
            None => println!("From {peer_id}: '{message}'"),
        }
    }
}
