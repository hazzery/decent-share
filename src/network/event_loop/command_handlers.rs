use std::collections::{hash_map, HashSet};

use anyhow::anyhow;
use futures::channel::oneshot;
use libp2p::{kad, multiaddr, request_response::ResponseChannel, Multiaddr, PeerId};

use super::{DirectMessage, EventLoop, FileRequest, FileResponse};

impl EventLoop {
    pub(in crate::network::event_loop) fn handle_start_listening(
        &mut self,
        address: Multiaddr,
        sender: oneshot::Sender<Result<(), anyhow::Error>>,
    ) {
        let _ = match self.swarm.listen_on(address) {
            Ok(_) => sender.send(Ok(())),
            Err(e) => sender.send(Err(anyhow!(e))),
        };
    }

    pub(in crate::network::event_loop) fn handle_register_name(&mut self, username: String) {
        let peer_id_bytes = self.swarm.local_peer_id().to_bytes();
        let username_bytes = username.into_bytes();

        let record = kad::Record {
            key: kad::RecordKey::new(&username_bytes),
            value: peer_id_bytes.clone(),
            publisher: None,
            expires: None,
        };
        self.swarm
            .behaviour_mut()
            .kademlia
            .put_record(record, kad::Quorum::One)
            .expect("Failed to store record locally");

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

    pub(in crate::network::event_loop) fn handle_find_user(
        &mut self,
        username: String,
        sender: oneshot::Sender<Result<PeerId, anyhow::Error>>,
    ) {
        let key = kad::RecordKey::new(&username.into_bytes());
        let query_id = self.swarm.behaviour_mut().kademlia.get_record(key);
        self.pending_name_request.insert(query_id, sender);
    }

    pub(in crate::network::event_loop) fn handle_dial(
        &mut self,
        peer_id: PeerId,
        peer_addr: Multiaddr,
        sender: oneshot::Sender<Result<(), anyhow::Error>>,
    ) {
        if let hash_map::Entry::Vacant(e) = self.pending_dial.entry(peer_id) {
            self.swarm
                .behaviour_mut()
                .kademlia
                .add_address(&peer_id, peer_addr.clone());
            match self
                .swarm
                .dial(peer_addr.with(multiaddr::Protocol::P2p(peer_id)))
            {
                Ok(()) => {
                    e.insert(sender);
                }
                Err(e) => {
                    let _ = sender.send(Err(anyhow!(e)));
                }
            }
        } else {
            todo!("Already dialing peer.");
        }
    }

    pub(in crate::network::event_loop) fn handle_start_providing(
        &mut self,
        file_name: String,
        sender: oneshot::Sender<()>,
    ) {
        let query_id = self
            .swarm
            .behaviour_mut()
            .kademlia
            .start_providing(file_name.into_bytes().into())
            .expect("No store error.");
        self.pending_start_providing.insert(query_id, sender);
    }

    pub(in crate::network::event_loop) fn handle_get_providers(
        &mut self,
        file_name: String,
        sender: oneshot::Sender<HashSet<PeerId>>,
    ) {
        let query_id = self
            .swarm
            .behaviour_mut()
            .kademlia
            .get_providers(file_name.into_bytes().into());
        self.pending_get_providers.insert(query_id, sender);
    }

    pub(in crate::network::event_loop) fn handle_request_file(
        &mut self,
        file_name: String,
        peer: PeerId,
        sender: oneshot::Sender<Result<Vec<u8>, anyhow::Error>>,
    ) {
        let request_id = self
            .swarm
            .behaviour_mut()
            .request_response
            .send_request(&peer, FileRequest(file_name));
        self.pending_request_file.insert(request_id, sender);
    }

    pub(in crate::network::event_loop) fn handle_respond_file(
        &mut self,
        file: Vec<u8>,
        channel: ResponseChannel<FileResponse>,
    ) {
        self.swarm
            .behaviour_mut()
            .request_response
            .send_response(channel, FileResponse(file))
            .expect("Connection to peer to be still open.");
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
        sender: oneshot::Sender<Result<(), anyhow::Error>>,
    ) {
        let request_id = self
            .swarm
            .behaviour_mut()
            .direct_messaging
            .send_request(peer_id, DirectMessage(message));
        self.pending_request_message.insert(request_id, sender);
    }
}
