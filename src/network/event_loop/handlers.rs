use std::{collections::HashSet, ops::Mul};

use futures::{channel, SinkExt};
use libp2p::{core, gossipsub, kad, multiaddr, request_response, swarm, Multiaddr, PeerId};

use super::{Event, EventLoop, FileRequest, FileResponse};

impl EventLoop {
    pub(in crate::network::event_loop) fn handle_pending_start_providing(
        &mut self,
        id: &kad::QueryId,
    ) {
        let sender: channel::oneshot::Sender<()> = self
            .pending_start_providing
            .remove(id)
            .expect("Completed query to be previously pending.");
        let _ = sender.send(());
    }

    pub(in crate::network::event_loop) fn handle_found_providers(
        &mut self,
        id: &kad::QueryId,
        providers: HashSet<PeerId>,
    ) {
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

    pub(in crate::network::event_loop) fn handle_new_listen_address(&mut self, address: Multiaddr) {
        let local_peer_id = *self.swarm.local_peer_id();
        eprintln!(
            "Local node is listening on {:?}",
            address.with(multiaddr::Protocol::P2p(local_peer_id))
        );
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
        list: &Vec<(PeerId, Multiaddr)>,
    ) {
        for (peer_id, _multiaddr) in list {
            println!("mDNS discovered a new peer: {peer_id}");
            self.swarm
                .behaviour_mut()
                .gossipsub
                .add_explicit_peer(&peer_id);
        }
    }

    pub(in crate::network::event_loop) fn handle_mdns_expired(
        &mut self,
        list: &Vec<(PeerId, Multiaddr)>,
    ) {
        for (peer_id, _multiaddr) in list {
            println!("mDNS discover peer has expired: {peer_id}");
            self.swarm
                .behaviour_mut()
                .gossipsub
                .remove_explicit_peer(&peer_id);
        }
    }

    pub(in crate::network::event_loop) fn handle_gossipsub_message(
        &mut self,
        message: &gossipsub::Message,
        id: &gossipsub::MessageId,
        peer_id: &PeerId,
    ) {
        println!(
            "Got message: '{}' with id: {id} from peer: {peer_id}",
            String::from_utf8_lossy(&message.data),
        )
    }
}
