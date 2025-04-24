use anyhow::anyhow;
use futures::SinkExt;
use libp2p::{
    gossipsub, identify,
    kad::{self, QueryId},
    multiaddr, rendezvous, request_response, Multiaddr, PeerId,
};

use super::{Event, EventLoop};
use crate::network::{
    event_loop::RENDEZVOUS_NAMESPACE, DirectMessage, NoResponse, TradeOffer, TradeResponse,
    TradeResponseResponse,
};

impl EventLoop {
    #[allow(clippy::unused_self)]
    pub(in crate::network::event_loop) fn handle_get_record(
        &mut self,
        record: kad::GetRecordResult,
        query_id: QueryId,
    ) {
        match record {
            Ok(kad::GetRecordOk::FoundRecord(kad::PeerRecord {
                record: kad::Record { value, .. },
                ..
            })) => {
                if let Some(peer_id_sender) = self.pending_peer_id_request.remove(&query_id) {
                    let peer_id = PeerId::from_bytes(&value).ok();
                    peer_id_sender
                        .send(peer_id)
                        .expect("Peer ID receiver was dropped");
                } else if let Some(username_sender) =
                    self.pending_username_request.remove(&query_id)
                {
                    let username = String::from_utf8(value).map_err(|error| anyhow!(error));
                    username_sender
                        .send(username)
                        .expect("Username receiver was dropped");
                }
            }
            Ok(_) => {}
            Err(error) => {
                if let Some(peer_id_sender) = self.pending_peer_id_request.remove(&query_id) {
                    peer_id_sender
                        .send(None)
                        .expect("Peer ID receiver was dropped");
                } else if let Some(username_sender) =
                    self.pending_username_request.remove(&query_id)
                {
                    username_sender
                        .send(Err(anyhow!(error)))
                        .expect("Username receiver was dropped");
                }
            }
        }
    }

    #[allow(clippy::unused_self)]
    pub(in crate::network::event_loop) fn handle_put_record(
        &mut self,
        record: kad::PutRecordResult,
        query_id: QueryId,
    ) {
        if let Some(status_sender) = self.pending_register_username.remove(&query_id) {
            let status = record.map(|_| ());
            status_sender
                .send(status)
                .expect("Status receiver was dropped");
        }
    }

    pub(in crate::network::event_loop) async fn handle_direct_messaging_message(
        &mut self,
        message: request_response::Message<DirectMessage, NoResponse>,
        peer_id: PeerId,
    ) {
        match message {
            request_response::Message::Request {
                request, channel, ..
            } => {
                self.event_sender
                    .send(Event::InboundDirectMessage {
                        peer_id,
                        message: request.0,
                    })
                    .await
                    .expect("Event receiver was dropped");

                self.swarm
                    .behaviour_mut()
                    .direct_messaging
                    .send_response(channel, NoResponse())
                    .expect("Connection to peer was dropped");
            }
            request_response::Message::Response { request_id, .. } => {
                let _ = self
                    .pending_request_message
                    .remove(&request_id)
                    .expect("Message was not pending")
                    .send(Ok(()));
            }
        }
    }

    pub(in crate::network::event_loop) fn handle_direct_messaging_outbound_failure(
        &mut self,
        request_id: request_response::OutboundRequestId,
        error: request_response::OutboundFailure,
    ) {
        self.pending_request_message
            .remove(&request_id)
            .expect("Message was not pending")
            .send(Err(anyhow!(error)))
            .expect("Direct messaging receiver was dropped");
    }

    pub(in crate::network::event_loop) async fn handle_trade_offering_message(
        &mut self,
        message: request_response::Message<TradeOffer, NoResponse>,
        peer_id: PeerId,
    ) {
        match message {
            request_response::Message::Request {
                request, channel, ..
            } => {
                self.swarm
                    .behaviour_mut()
                    .trade_offering
                    .send_response(channel, NoResponse())
                    .expect("Connection to peer was dropped");

                self.inbound_trade_offers.insert((peer_id, request.clone()));

                self.event_sender
                    .send(Event::InboundTradeOffer {
                        offered_file_name: request.offered_file_name,
                        peer_id,
                        requested_file_name: request.requested_file_name,
                    })
                    .await
                    .expect("Event receiver was dropped");
            }
            request_response::Message::Response { request_id, .. } => {
                if let Some(status_sender) = self.pending_trade_offer_request.remove(&request_id) {
                    status_sender
                        .send(Ok(()))
                        .expect("Status sender was dropped");
                }
            }
        }
    }

    #[allow(clippy::unused_self)]
    pub(in crate::network::event_loop) fn handle_trade_offering_outbound_failure(
        &mut self,
        error: request_response::OutboundFailure,
        request_id: request_response::OutboundRequestId,
    ) {
        if let Some(status_sender) = self.pending_trade_offer_request.remove(&request_id) {
            status_sender
                .send(Err(anyhow!(error)))
                .expect("Status receiver was dropped");
        }
    }

    pub(in crate::network::event_loop) async fn handle_trade_response_message(
        &mut self,
        message: request_response::Message<TradeResponse, TradeResponseResponse>,
        peer_id: PeerId,
    ) {
        match message {
            request_response::Message::Request {
                request, channel, ..
            } => {
                let offer = TradeOffer {
                    requested_file_name: request.requested_file_name.clone(),
                    offered_file_name: request.offered_file_name.clone(),
                };
                let entry = self.outgoing_trade_offers.remove(&(peer_id, offer));
                let Some((offered_file_bytes, requested_file_path)) = entry else {
                    return;
                };

                self.event_sender
                    .send(Event::InboundTradeResponse {
                        peer_id,
                        offered_file_name: request.offered_file_name.clone(),
                        requested_file_name: request.requested_file_name.clone(),
                        was_accepted: request.requested_file_bytes.is_some(),
                    })
                    .await
                    .expect("Event receiver was dropped");

                let mut response: Option<Vec<u8>> = None;

                if let Some(requested_file_bytes) = request.requested_file_bytes {
                    if let Some(parent_directory) = requested_file_path.parent() {
                        tokio::fs::create_dir_all(parent_directory)
                            .await
                            .expect("Failed to create parent directories");
                    }
                    tokio::fs::write(requested_file_path, requested_file_bytes)
                        .await
                        .expect("Failed to write to file system");
                    response = Some(offered_file_bytes);
                }

                self.swarm
                    .behaviour_mut()
                    .trade_response
                    .send_response(
                        channel,
                        TradeResponseResponse {
                            offered_file_name: request.offered_file_name,
                            requested_file_name: request.requested_file_name,
                            offered_file_bytes: response,
                        },
                    )
                    .expect("Connection to peer was dropped");
            }
            request_response::Message::Response {
                response,
                request_id,
            } => {
                if let Some(offered_bytes_sender) =
                    self.pending_trade_response_response.remove(&request_id)
                {
                    offered_bytes_sender
                        .send(Ok(response.offered_file_bytes))
                        .expect("Offered bytes receiver was dropped");
                }
            }
        }
    }

    pub(in crate::network::event_loop) fn handle_trade_response_outbound_failure(
        &mut self,
        request_id: request_response::OutboundRequestId,
        error: request_response::OutboundFailure,
    ) {
        if let Some(offered_bytes_sender) = self.pending_trade_response_response.remove(&request_id)
        {
            offered_bytes_sender
                .send(Err(anyhow::Error::from(error)))
                .expect("Offered bytes receiver was dropped");
        }
    }

    #[allow(clippy::unused_self)]
    pub(in crate::network::event_loop) async fn handle_gossipsub_message(
        &mut self,
        message: &gossipsub::Message,
        peer_id: PeerId,
    ) {
        let message = String::from_utf8_lossy(&message.data).into_owned();
        self.event_sender
            .send(Event::InboundChat { peer_id, message })
            .await
            .expect("Event receiver was dropped");
    }

    pub(in crate::network::event_loop) fn handle_rendezvous_discovered(
        &mut self,
        registrations: Vec<rendezvous::Registration>,
        cookie: rendezvous::Cookie,
    ) {
        self.cookie.replace(cookie);

        for registration in registrations {
            for address in registration.record.addresses() {
                let peer_id = registration.record.peer_id();
                tracing::info!(%peer_id, %address, "Discovered peer");

                let p2p_suffix = multiaddr::Protocol::P2p(peer_id);
                let address_with_p2p =
                    if address.ends_with(&Multiaddr::empty().with(p2p_suffix.clone())) {
                        address.clone()
                    } else {
                        address.clone().with(p2p_suffix)
                    };

                self.swarm.dial(address_with_p2p).unwrap();

                self.swarm
                    .behaviour_mut()
                    .gossipsub
                    .add_explicit_peer(&peer_id);

                self.swarm
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, address.to_owned());
            }
        }
    }

    pub(in crate::network::event_loop) fn handle_connected_to_rendezvous_server(&mut self) {
        self.swarm.behaviour_mut().rendezvous.discover(
            Some(rendezvous::Namespace::from_static(RENDEZVOUS_NAMESPACE)),
            None,
            None,
            self.rendezvous_peer_id,
        );
    }

    pub(in crate::network::event_loop) fn handle_identify_received(
        &mut self,
        info: identify::Info,
    ) {
        // once `/identify` did its job, we know our external address and can
        // register. This needs to be done explicitly for this case, as it's a
        // local address.
        self.swarm.add_external_address(info.observed_addr);
        if let Err(error) = self.swarm.behaviour_mut().rendezvous.register(
            rendezvous::Namespace::from_static("rendezvous"),
            self.rendezvous_peer_id,
            None,
        ) {
            tracing::error!("Failed to register: {error}");
        }
        tracing::info!("Connection established with rendezvous point");
    }
}
