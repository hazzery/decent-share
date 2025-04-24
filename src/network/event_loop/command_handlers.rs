use std::path::PathBuf;

use anyhow::anyhow;
use futures::channel::oneshot;
use libp2p::{kad, PeerId};

use super::{DirectMessage, EventLoop, TradeResponse};
use crate::network::TradeOffer;

impl EventLoop {
    pub(in crate::network::event_loop) fn handle_register_name(
        &mut self,
        username: &str,
        status_sender: oneshot::Sender<Result<(), kad::PutRecordError>>,
    ) {
        let peer_id_bytes = self.swarm.local_peer_id().to_bytes();
        let username_bytes = username.to_lowercase().into_bytes();

        let record = kad::Record {
            key: kad::RecordKey::new(&username_bytes),
            value: peer_id_bytes.clone(),
            publisher: None,
            expires: None,
        };
        let query_id = self
            .swarm
            .behaviour_mut()
            .kademlia
            .put_record(record, kad::Quorum::One)
            .expect("Failed to store record locally");

        self.pending_register_username
            .insert(query_id, status_sender);

        let record = kad::Record {
            key: kad::RecordKey::new(&peer_id_bytes),
            value: username_bytes.clone(),
            publisher: None,
            expires: None,
        };
        self.swarm
            .behaviour_mut()
            .kademlia
            .put_record(record, kad::Quorum::One)
            .expect("Failed to store record locally");
    }

    pub(in crate::network::event_loop) fn handle_find_peer_id(
        &mut self,
        username: &str,
        peer_id_sender: oneshot::Sender<Option<PeerId>>,
    ) {
        let key = kad::RecordKey::new(&username.to_lowercase().into_bytes());
        let query_id = self.swarm.behaviour_mut().kademlia.get_record(key);
        self.pending_peer_id_request
            .insert(query_id, peer_id_sender);
    }

    pub(in crate::network::event_loop) fn handle_find_peer_username(
        &mut self,
        peer_id: PeerId,
        username_sender: oneshot::Sender<Result<String, anyhow::Error>>,
    ) {
        let key = kad::RecordKey::new(&peer_id.to_bytes());
        let query_id = self.swarm.behaviour_mut().kademlia.get_record(key);
        self.pending_username_request
            .insert(query_id, username_sender);
    }

    pub(in crate::network::event_loop) fn handle_make_offer(
        &mut self,
        offered_file_name: String,
        offered_file_bytes: Vec<u8>,
        peer_id: PeerId,
        requested_file_name: String,
        requested_file_path: PathBuf,
        error_sender: oneshot::Sender<Result<(), anyhow::Error>>,
    ) {
        if &peer_id == self.swarm.local_peer_id() {
            error_sender
                .send(Err(anyhow!("May not send trade to yourself")))
                .expect("Error receiver was dropped");
            return;
        }
        let offer = TradeOffer {
            offered_file_name,
            requested_file_name,
        };
        self.swarm
            .behaviour_mut()
            .trade_offering
            .send_request(&peer_id, offer.clone());

        self.outgoing_trade_offers
            .insert((peer_id, offer), (offered_file_bytes, requested_file_path));

        error_sender
            .send(Ok(()))
            .expect("Error receiver was dropped");
    }

    pub(in crate::network::event_loop) fn handle_respond_trade(
        &mut self,
        peer_id: PeerId,
        requested_file_name: String,
        offered_file_name: String,
        requested_file_bytes: Option<Vec<u8>>,
        offered_bytes_sender: Option<oneshot::Sender<Result<Option<Vec<u8>>, anyhow::Error>>>,
    ) {
        let offer = TradeOffer {
            requested_file_name: requested_file_name.clone(),
            offered_file_name: offered_file_name.clone(),
        };
        if !self.inbound_trade_offers.remove(&(peer_id, offer)) {
            if let Some(offered_bytes_sender) = offered_bytes_sender {
                let _ = offered_bytes_sender.send(Err(anyhow!(format!(
                    "No valid trade with this user for {offered_file_name} and {requested_file_name}"
                ))));
            }
            return;
        }
        let request_id = self.swarm.behaviour_mut().trade_response.send_request(
            &peer_id,
            TradeResponse {
                requested_file_name,
                offered_file_name,
                requested_file_bytes,
            },
        );
        if let Some(offered_bytes_sender) = offered_bytes_sender {
            self.pending_trade_response_response
                .insert(request_id, offered_bytes_sender);
        }
    }

    pub(in crate::network::event_loop) fn handle_send_message(&mut self, message: &str) {
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(self.gossipsub_topic.clone(), message.as_bytes())
            .expect("Message Publish error!");
    }

    pub(in crate::network::event_loop) fn handle_direct_message(
        &mut self,
        peer_id: &PeerId,
        message: String,
        error_sender: oneshot::Sender<Result<(), anyhow::Error>>,
    ) {
        let request_id = self
            .swarm
            .behaviour_mut()
            .direct_messaging
            .send_request(peer_id, DirectMessage(message));
        self.pending_request_message
            .insert(request_id, error_sender);
    }
}
