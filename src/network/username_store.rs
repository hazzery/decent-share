use std::collections::HashMap;

use libp2p::PeerId;

#[derive(Default)]
pub(super) struct UsernameStore {
    username_peer_id_map: HashMap<String, PeerId>,
    peer_id_username_map: HashMap<PeerId, String>,
}

impl UsernameStore {
    pub fn get_username(&self, peer_id: &PeerId) -> Option<&String> {
        self.peer_id_username_map.get(peer_id)
    }

    pub fn get_peer_id(&self, username: &str) -> Option<&PeerId> {
        self.username_peer_id_map.get(username)
    }

    pub fn insert(&mut self, username: String, peer_id: PeerId) {
        self.username_peer_id_map.insert(username.clone(), peer_id);
        self.peer_id_username_map.insert(peer_id, username);
    }
}
