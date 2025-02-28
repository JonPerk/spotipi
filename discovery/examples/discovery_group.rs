use futures::StreamExt;
use spotipi_core::SessionConfig;
use spotipi_discovery::DeviceType;
use sha1::{Digest, Sha1};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let name = "Spotipi Group";
    let device_id = hex::encode(Sha1::digest(name.as_bytes()));

    let mut server =
        spotipi_discovery::Discovery::builder(device_id, SessionConfig::default().client_id)
            .name(name)
            .device_type(DeviceType::Speaker)
            .is_group(true)
            .launch()
            .unwrap();

    while let Some(x) = server.next().await {
        println!("Received {:?}", x);
    }
}
