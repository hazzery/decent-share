use std::{
    borrow::ToOwned,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, bail};
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use libp2p::PeerId;

use super::{event_loop::Command, username_store::UsernameStore};

#[derive(Clone)]
pub(crate) struct Client {
    pub(super) sender: mpsc::Sender<Command>,
    pub(super) username_store: Arc<Mutex<UsernameStore>>,
}

impl Client {
    pub(crate) async fn get_peer_id(&mut self, username: String) -> Option<PeerId> {
        let mut peer_id = self
            .username_store
            .lock()
            .unwrap()
            .get_peer_id(&username)
            .map(ToOwned::to_owned);

        if peer_id.is_none() {
            peer_id = self.find_user(username).await;
        }
        peer_id
    }

    pub(crate) async fn get_username(&mut self, peer_id: PeerId) -> Result<String, anyhow::Error> {
        let username = self
            .username_store
            .lock()
            .unwrap()
            .get_username(&peer_id)
            .map(ToOwned::to_owned);

        match username {
            Some(username) => Ok(username),
            None => self.find_peer_username(peer_id).await,
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
        let Some(peer_id) = self.get_peer_id(recipient_username.clone()).await else {
            bail!("'{recipient_username}' is not a registered user");
        };

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
        let Some(peer_id) = self.get_peer_id(username.clone()).await else {
            bail!("'{username}' is not a register user");
        };

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
            Ok(Some(bytes)) => Ok(bytes),
            Ok(None) => Err(anyhow!("No bytes were received!")),
            Err(error) => Err(error),
        }
    }

    pub(crate) async fn decline_trade(
        &mut self,
        username: String,
        offered_file_name: String,
        requested_file_name: String,
    ) -> Result<(), anyhow::Error> {
        let Some(peer_id) = self.get_peer_id(username.clone()).await else {
            bail!("'{username}' is not a registered user");
        };
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

    async fn find_user(&mut self, username: String) -> Option<PeerId> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::FindUser {
                username: username.clone(),
                sender,
            })
            .await
            .expect("Error sending FindUser command");

        let peer_id = receiver.await.expect("Sender not be dropped.");

        if let Some(peer_id) = peer_id {
            self.username_store
                .lock()
                .unwrap()
                .insert(username, peer_id);
        }

        peer_id
    }

    async fn find_peer_username(&mut self, peer_id: PeerId) -> Result<String, anyhow::Error> {
        let (username_sender, username_receiver) = oneshot::channel();
        self.sender
            .send(Command::FindPeerUsername {
                peer_id,
                username_sender,
            })
            .await
            .expect("Error sending FindPeerUsername command");
        let username = username_receiver.await.expect("sender not to be dropped");

        if let Ok(ref username) = username {
            self.username_store
                .lock()
                .unwrap()
                .insert(username.to_owned(), peer_id);
        }

        username
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

        let Some(peer_id) = self.get_peer_id(username.clone()).await else {
            bail!("'{username}' is not a registered user");
        };

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
