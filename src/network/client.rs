use std::{
    borrow::ToOwned,
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::anyhow;
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use libp2p::PeerId;

use super::event_loop::Command;

#[derive(Clone)]
pub(crate) struct Client {
    pub(super) sender: mpsc::Sender<Command>,
    pub(super) username_peer_id_map: Arc<Mutex<HashMap<String, PeerId>>>,
}

impl Client {
    async fn get_peer_id(&mut self, username: String) -> Result<PeerId, anyhow::Error> {
        let peer_id = self
            .username_peer_id_map
            .lock()
            .unwrap()
            .get(&username.to_lowercase())
            .map(ToOwned::to_owned);

        match peer_id {
            Some(peer_id) => Ok(peer_id),
            None => self.find_user(username).await,
        }
    }
}

impl Client {
    pub(crate) async fn offer_trade(
        &mut self,
        offered_file_name: String,
        offered_file_bytes: Vec<u8>,
        recipient_username: String,
        requested_file_name: String,
        requested_file_path: PathBuf,
    ) -> Result<(), anyhow::Error> {
        let peer_id = self.get_peer_id(recipient_username).await?;

        self.sender
            .send(Command::MakeOffer {
                offered_file_name,
                offered_file_bytes,
                peer_id,
                requested_file_name,
                requested_file_path,
            })
            .await
            .expect("Command receiver not to be dropped.");

        Ok(())
    }

    pub(crate) async fn accept_trade(
        &mut self,
        username: String,
        requested_file_name: String,
        offered_file_name: String,
        requested_file_bytes: Vec<u8>,
    ) -> Result<Vec<u8>, anyhow::Error> {
        let peer_id = self.get_peer_id(username).await?;

        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::RespondTrade {
                peer_id,
                requested_file_name,
                offered_file_name,
                requested_file_bytes: Some(requested_file_bytes),
                offered_bytes_sender: Some(sender),
            })
            .await
            .expect("Command receiver not to be dropped");

        match receiver.await.expect("Sender not to be dropped") {
            Some(bytes) => Ok(bytes),
            None => Err(anyhow!("No bytes were received!")),
        }
    }

    pub(crate) async fn decline_trade(
        &mut self,
        username: String,
        offered_file_name: String,
        requested_file_name: String,
    ) -> Result<(), anyhow::Error> {
        let peer_id = self.get_peer_id(username).await?;
        self.sender
            .send(Command::RespondTrade {
                peer_id,
                requested_file_name,
                offered_file_name,
                requested_file_bytes: None,
                offered_bytes_sender: None,
            })
            .await
            .expect("Sender not to be dropped");
        Ok(())
    }

    pub(crate) async fn register_username(&mut self, username: String) {
        self.sender
            .send(Command::RegisterName { username })
            .await
            .expect("Name to register successfully");
    }

    pub(crate) async fn find_user(&mut self, username: String) -> Result<PeerId, anyhow::Error> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::FindUser {
                username: username.clone(),
                sender,
            })
            .await
            .expect("Error sending FindUser command");

        let peer_id = receiver.await.expect("Sender not be dropped.");

        if let Ok(peer_id) = peer_id {
            self.username_peer_id_map
                .lock()
                .unwrap()
                .insert(username, peer_id);
        }

        peer_id
    }

    pub(crate) async fn send_message(&mut self, message: String) {
        self.sender
            .send(Command::SendMessage { message })
            .await
            .expect("Message to send successfully");
    }

    pub(crate) async fn direct_message(
        &mut self,
        username: String,
        message: String,
    ) -> Result<(), anyhow::Error> {
        let (sender, receiver) = oneshot::channel();

        let peer_id = self.get_peer_id(username).await?;

        self.sender
            .send(Command::DirectMessage {
                peer_id,
                message,
                sender,
            })
            .await
            .expect("Direct message to send successfully");
        receiver.await.expect("Command receiver to not be dropped")
    }
}
