use anyhow::anyhow;
use futures::SinkExt;
use libp2p::{
    core, gossipsub,
    kad::{self, QueryId},
    request_response, swarm, Multiaddr, PeerId,
};
use std::borrow::ToOwned;

use super::{Event, EventLoop};
use crate::network::{DirectMessage, NoResponse, TradeOffer, TradeResponse, TradeResponseResponse};

impl EventLoop {
    #[allow(clippy::unused_self)]
    pub(in crate::network::event_loop) fn handle_get_record(
        &mut self,
        record: kad::GetRecordResult,
        query_id: QueryId,
    ) {
        match record {
            Ok(kad::GetRecordOk::FoundRecord(kad::PeerRecord {
                record: kad::Record { key, value, .. },
                ..
            })) => {
                if let Some(sender) = self.pending_name_request.remove(&query_id) {
                    let peer_id = PeerId::from_bytes(&value).ok();

                    if let Some(peer_id) = peer_id {
                        if let Ok(username) = String::from_utf8(key.to_vec()) {
                            self.peer_id_username_map.insert(peer_id, username);
                        }
                    }

                    sender.send(peer_id).expect("Receiver not to be dropped");
                } else if let Some(sender) = self.pending_username_request.remove(&query_id) {
                    let username = String::from_utf8(value).map_err(|error| anyhow!(error));

                    if let Ok(ref username) = username {
                        if let Ok(peer_id) = PeerId::from_bytes(&key.to_vec()) {
                            self.peer_id_username_map
                                .insert(peer_id, username.to_owned());
                        }
                    }
                    sender.send(username).expect("Receiver not to be dropped");
                }
            }
            Ok(_) => {}
            Err(_) => {
                if let Some(sender) = self.pending_name_request.remove(&query_id) {
                    let _ = sender.send(None);
                }
            }
        }
    }

    #[allow(clippy::unused_self)]
    pub(in crate::network::event_loop) fn handle_put_record(
        &mut self,
        record: kad::PutRecordResult,
    ) {
        match record {
            Ok(kad::PutRecordOk { .. }) => println!("Successfully stored record!"),
            Err(error) => eprintln!("Failed to store record: {error:?}"),
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
                    .expect("Event sender not to be dropped");

                self.swarm
                    .behaviour_mut()
                    .direct_messaging
                    .send_response(channel, NoResponse())
                    .expect("The response to be sent");
            }
            request_response::Message::Response { request_id, .. } => {
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
            .send(Err(anyhow!(error)));
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
                    .expect("Connection to peer to still be open");

                self.inbound_trade_offers.insert((peer_id, request.clone()));

                self.event_sender
                    .send(Event::InboundTradeOffer {
                        offered_file_name: request.offered_file_name,
                        peer_id,
                        requested_file_name: request.requested_file_name,
                    })
                    .await
                    .expect("Event receiver not to be dropped");
            }
            request_response::Message::Response { .. } => {}
        }
    }

    #[allow(clippy::unused_self)]
    pub(in crate::network::event_loop) fn handle_trade_offering_outbound_failure(
        &mut self,
        error: &request_response::OutboundFailure,
    ) {
        println!("A trade offer failure has occured: {error:?}");
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
                    println!("Received invalid trade response!");
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
                    .expect("Event receiver not to be dropped");

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
                    .expect("Connection to peer to still be open");
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
        println!("A trade response failure has occured: {error:?}");
        if let Some(offered_bytes_sender) = self.pending_trade_response_response.remove(&request_id)
        {
            let _ = offered_bytes_sender.send(Err(anyhow::Error::from(error)));
        }
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
                let _ = sender.send(Err(anyhow!(error)));
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
    pub(in crate::network::event_loop) async fn handle_gossipsub_message(
        &mut self,
        message: &gossipsub::Message,
        peer_id: PeerId,
    ) {
        let message = String::from_utf8_lossy(&message.data).into_owned();
        self.event_sender
            .send(Event::InboundChat { peer_id, message })
            .await
            .expect("Event sender not to be dropped");
    }
}
