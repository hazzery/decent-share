use std::collections::HashMap;

use libp2p::PeerId;

#[derive(Default)]
pub(super) struct UsernameStore {
    username_peer_id_map: HashMap<String, PeerId>,
    peer_id_username_map: HashMap<PeerId, String>,
}

impl UsernameStore {
    pub fn contains_username(&self, username: &str) -> bool {
        self.username_peer_id_map.contains_key(username)
    }

    pub fn contains_peer_id(&self, peer_id: &PeerId) -> bool {
        self.peer_id_username_map.contains_key(peer_id)
    }

    pub fn get_username(&self, peer_id: &PeerId) -> Option<&String> {
        let cached_value = self.peer_id_username_map.get(peer_id);
        if cached_value.is_some() {
            return cached_value;
        }
        // network_client.find_user()
        todo!("Find username for given Peer ID");
    }

    pub fn get_peer_id(&self, username: &str) -> Option<&PeerId> {
        let cached_value = self.username_peer_id_map.get(username);
        if cached_value.is_some() {
            return cached_value;
        }
        // network_client.find_user(username)
        todo!("Find Peeer ID of given username");
    }

    pub fn insert(&mut self, username: String, peer_id: PeerId) {
        self.username_peer_id_map.insert(username.clone(), peer_id);
        self.peer_id_username_map.insert(peer_id, username);
    }
}
