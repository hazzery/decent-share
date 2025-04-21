use std::path::PathBuf;
use std::str::FromStr;

use anyhow::bail;

use crate::network::Client;

pub(crate) async fn handle_send(message: &str, network_client: &mut Client) {
    network_client.send_message(message.to_string()).await;
}
pub(crate) async fn handle_trade(
    offered_file_name: &str,
    offered_file_path: &str,
    username: &str,
    requested_file_name: &str,
    requested_file_path: &str,
    network_client: &mut Client,
) -> Result<(), anyhow::Error> {
    let offered_file_path = PathBuf::from_str(offered_file_path)?;
    if !offered_file_path.is_file() {
        bail!("The provided path to the offered file does not point to a file!");
    }
    let requested_file_path = PathBuf::from_str(requested_file_path)?;
    if requested_file_path.exists() {
        bail!("Provided path to place the requested file already exists!, please provided a path to write the file to");
    }
    if let Some(parent) = requested_file_path.parent() {
        if !parent.is_dir() {
            bail!("Provided path to place the requested file is not valid. parent of provided filename must be a directory");
        }
    }

    let offered_file_bytes = tokio::fs::read(offered_file_path).await?;

    network_client
        .offer_trade(
            offered_file_name.to_string(),
            offered_file_bytes,
            username.to_string(),
            requested_file_name.to_string(),
            requested_file_path,
        )
        .await?;

    Ok(())
}

pub(crate) async fn handle_accept_trade(
    username: String,
    requested_file_name: String,
    requested_file_path: &str,
    offered_file_name: String,
    offered_file_path: &str,
    network_client: &mut Client,
) -> Result<(), anyhow::Error> {
    let requested_file_path = PathBuf::from_str(requested_file_path)?;
    if !requested_file_path.is_file() {
        bail!("Path to requested file ({requested_file_path:?}) does not point to a file!");
    }

    let offered_file_path = PathBuf::from_str(offered_file_path)?;
    if offered_file_path.exists() {
        bail!("Path to place offered file ({offered_file_path:?}) already exists!");
    }

    let requested_file_bytes = tokio::fs::read(requested_file_path).await?;
    let offered_file_bytes = network_client
        .accept_trade(
            username,
            requested_file_name,
            offered_file_name,
            requested_file_bytes,
        )
        .await?;

    tokio::fs::write(offered_file_path, offered_file_bytes).await?;

    Ok(())
}
