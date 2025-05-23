use std::path::PathBuf;
use std::str::FromStr;

use anyhow::bail;
use libp2p::gossipsub;

use crate::network::Client;

pub(crate) async fn handle_send(
    message: &str,
    network_client: &mut Client,
) -> Result<(), gossipsub::PublishError> {
    network_client.send_message(message.to_string()).await
}
pub(crate) async fn handle_trade(
    offered_file_name: &str,
    offered_file_path_string: &str,
    username: &str,
    requested_file_name: &str,
    requested_file_path_string: &str,
    network_client: &mut Client,
) -> Result<(), anyhow::Error> {
    let offered_file_path = PathBuf::from_str(offered_file_path_string)?;
    if !offered_file_path.is_file() {
        bail!("'{offered_file_path_string}' does not point to a file!");
    }
    let requested_file_path = PathBuf::from_str(requested_file_path_string)?;
    if requested_file_path.exists() {
        bail!("A file already exists at '{requested_file_path_string}'!\nPlease provide an empty path to write the requested file to");
    }

    let offered_file_bytes = tokio::fs::read(offered_file_path).await?;

    network_client
        .offer_trade(
            offered_file_name.to_owned(),
            offered_file_bytes,
            username.to_owned(),
            requested_file_name.to_owned(),
            requested_file_path,
        )
        .await?;

    Ok(())
}

pub(crate) async fn handle_accept_trade(
    username: &str,
    offered_file_name: &str,
    offered_file_path_string: &str,
    requested_file_name: &str,
    requested_file_path_string: &str,
    network_client: &mut Client,
) -> Result<(), anyhow::Error> {
    let requested_file_path = PathBuf::from_str(requested_file_path_string)?;
    if !requested_file_path.is_file() {
        bail!("'{requested_file_path_string}' does not point to a file!");
    }

    let offered_file_path = PathBuf::from_str(offered_file_path_string)?;
    if offered_file_path.exists() {
        bail!("A file already exists at '{offered_file_path_string}'! Please provide an empty path to write the offered file to");
    }

    let requested_file_bytes = tokio::fs::read(requested_file_path).await?;
    let offered_file_bytes = network_client
        .accept_trade(
            username.to_owned(),
            requested_file_name.to_owned(),
            offered_file_name.to_owned(),
            requested_file_bytes,
        )
        .await?;

    if let Some(parent_directory) = offered_file_path.parent() {
        tokio::fs::create_dir_all(parent_directory)
            .await
            .expect("Failed to create parent directories");
    }
    tokio::fs::write(offered_file_path, offered_file_bytes).await?;
    println!(
        "{username}'s '{offered_file_name}' file is now available at '{offered_file_path_string}'"
    );

    Ok(())
}
