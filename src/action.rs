use std::io::Write;

use anyhow::anyhow;
use futures::FutureExt;
use futures::{Stream, StreamExt};

use crate::network::{Client, Event};

pub(crate) async fn handle_send(message: &str, network_client: &mut Client) {
    network_client.send_message(message.to_string()).await;
}
pub(crate) async fn handle_provide(
    name: &String,
    file_path: &str,
    network_client: &mut Client,
    network_events: &mut (impl Stream<Item = Event> + Unpin),
) -> Result<(), anyhow::Error> {
    // Advertise oneself as a provider of the file on the DHT.
    network_client.start_providing(name.clone()).await;

    loop {
        match network_events.next().await {
            // Reply with the content of the file on incoming requests.
            Some(Event::InboundRequest { request, channel }) => {
                if &request == name {
                    network_client
                        .respond_file(std::fs::read(file_path)?, channel)
                        .await;
                }
            }
            e => todo!("{:?}", e),
        }
    }
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
