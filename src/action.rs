use std::str::FromStr;
use std::{io::Write, path::PathBuf};

use anyhow::anyhow;
use futures::FutureExt;

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
    let offered_path = PathBuf::from_str(offered_file_path)?;
    if !offered_path.is_file() {
        return Err(anyhow!(
            "The provided path to the offered file does not point to a file!"
        ));
    }
    let requested_path = PathBuf::from_str(requested_file_path)?;
    if requested_path.exists() {
        return Err(anyhow!("Provided path to place the requested file already exists!, please provided a path to write the file to"));
    }
    if let Some(parent) = requested_path.parent() {
        if !parent.is_dir() {
            return Err(anyhow!("Provided path to place the requested file is not valid. parent of provided filename must be a directory"));
        }
    }

    let requested_file_bytes = network_client
        .offer_trade(
            offered_file_name.to_string(),
            username.to_string(),
            requested_file_name.to_string(),
        )
        .await?;

    match requested_file_bytes {
        Some(bytes) => tokio::fs::write(requested_path, bytes).await?,
        None => println!("The recipient declined your trade offer!"),
    };

    Ok(())
}

pub(crate) async fn handle_get(
    name: &String,
    network_client: &mut Client,
) -> Result<(), anyhow::Error> {
    // Locate all nodes providing the file.
    let providers = network_client.get_providers(name.clone()).await;
    if providers.is_empty() {
        return Err(anyhow!("Could not find provider for file {name}."));
    }

    // Request the content of the file from each node.
    let requests = providers.into_iter().map(|p| {
        let mut network_client = network_client.clone();
        let name = name.clone();
        async move { network_client.request_file(p, name).await }.boxed()
    });

    // Await the requests, ignore the remaining once a single one succeeds.
    let file_content = futures::future::select_ok(requests)
        .await
        .map_err(|_| anyhow!("None of the providers returned file."))?
        .0;

    std::io::stdout().write_all(&file_content)?;

    Ok(())
}
