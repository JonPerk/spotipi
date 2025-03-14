//! Advertises this device to Spotify clients in the local network.
//!
//! This device will show up in the list of "available devices".
//! Once it is selected from the list, [`Credentials`] are received.
//! Those can be used to establish a new Session with [`spotipi_core`].
//!
//! This library uses mDNS and DNS-SD so that other devices can find it,
//! and spawns an http server to answer requests of Spotify clients.

mod avahi;
mod server;

use std::{
    borrow::Cow,
    error::Error as StdError,
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::Stream;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use self::server::DiscoveryServer;

pub use crate::core::Error;
use spotipi_core as core;

/// Credentials to be used in [`spotipi`](`spotipi_core`).
pub use crate::core::authentication::Credentials;

/// Determining the icon in the list of available devices.
pub use crate::core::config::DeviceType;

pub enum DiscoveryEvent {
    Credentials(Credentials),
    ServerError(DiscoveryError),
    ZeroconfError(DiscoveryError),
}

enum ZeroconfCmd {
    Shutdown,
}

pub struct DnsSdHandle {
    task_handle: tokio::task::JoinHandle<()>,
    shutdown_tx: oneshot::Sender<ZeroconfCmd>,
}

impl DnsSdHandle {
    async fn shutdown(self) {
        log::debug!("Shutting down zeroconf responder");
        let Self {
            task_handle,
            shutdown_tx,
        } = self;
        if shutdown_tx.send(ZeroconfCmd::Shutdown).is_err() {
            log::warn!("Zeroconf responder unexpectedly disappeared");
        } else {
            let _ = task_handle.await;
            log::debug!("Zeroconf responder stopped");
        }
    }
}

pub type DnsSdServiceBuilder = fn(
    Cow<'static, str>,
    Vec<std::net::IpAddr>,
    u16,
    mpsc::UnboundedSender<DiscoveryEvent>,
) -> Result<DnsSdHandle, Error>;

// Default goes first: This matches the behaviour when feature flags were exlusive, i.e. when there
// was only `feature = "with-dns-sd"` or `not(feature = "with-dns-sd")`
pub const BACKENDS: &[(
    &str,
    // If None, the backend is known but wasn't compiled.
    Option<DnsSdServiceBuilder>,
)] = &[
    #[cfg(feature = "with-avahi")]
    ("avahi", Some(launch_avahi)),
    #[cfg(not(feature = "with-avahi"))]
    ("avahi", None),
    #[cfg(feature = "with-dns-sd")]
    ("dns-sd", Some(launch_dns_sd)),
    #[cfg(not(feature = "with-dns-sd"))]
    ("dns-sd", None),
    #[cfg(feature = "with-libmdns")]
    ("libmdns", Some(launch_libmdns)),
    #[cfg(not(feature = "with-libmdns"))]
    ("libmdns", None),
];

pub fn find(name: Option<&str>) -> Result<DnsSdServiceBuilder, Error> {
    if let Some(ref name) = name {
        match BACKENDS.iter().find(|(id, _)| name == id) {
            Some((_id, Some(launch_svc))) => Ok(*launch_svc),
            Some((_id, None)) => Err(Error::unavailable(format!(
                "spotipi built without '{}' support",
                name
            ))),
            None => Err(Error::not_found(format!(
                "unknown zeroconf backend '{}'",
                name
            ))),
        }
    } else {
        BACKENDS
            .iter()
            .find_map(|(_, launch_svc)| *launch_svc)
            .ok_or(Error::unavailable(
                "spotipi built without zeroconf backends",
            ))
    }
}

/// Makes this device visible to Spotify clients in the local network.
///
/// `Discovery` implements the [`Stream`] trait. Every time this device
/// is selected in the list of available devices, it yields [`Credentials`].
pub struct Discovery {
    server: DiscoveryServer,

    /// An opaque handle to the DNS-SD service. Dropping this will unregister the service.
    #[allow(unused)]
    svc: DnsSdHandle,

    event_rx: mpsc::UnboundedReceiver<DiscoveryEvent>,
}

/// A builder for [`Discovery`].
pub struct Builder {
    server_config: server::Config,
    port: u16,
    zeroconf_ip: Vec<std::net::IpAddr>,
    zeroconf_backend: Option<DnsSdServiceBuilder>,
}

/// Errors that can occur while setting up a [`Discovery`] instance.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("Creating SHA1 block cipher failed")]
    AesError(#[from] aes::cipher::InvalidLength),

    #[error("Setting up dns-sd failed: {0}")]
    DnsSdError(#[source] Box<dyn StdError + Send + Sync>),

    #[error("Creating SHA1 HMAC failed for base key {0:?}")]
    HmacError(Vec<u8>),

    #[error("Setting up the HTTP server failed: {0}")]
    HttpServerError(#[from] hyper::Error),

    #[error("Missing params for key {0}")]
    ParamsError(&'static str),
}

#[cfg(feature = "with-avahi")]
impl From<zbus::Error> for DiscoveryError {
    fn from(error: zbus::Error) -> Self {
        Self::DnsSdError(Box::new(error))
    }
}

impl From<DiscoveryError> for Error {
    fn from(err: DiscoveryError) -> Self {
        match err {
            DiscoveryError::AesError(_) => Error::unavailable(err),
            DiscoveryError::DnsSdError(_) => Error::unavailable(err),
            DiscoveryError::HmacError(_) => Error::invalid_argument(err),
            DiscoveryError::HttpServerError(_) => Error::unavailable(err),
            DiscoveryError::ParamsError(_) => Error::invalid_argument(err),
        }
    }
}

#[allow(unused)]
const DNS_SD_SERVICE_NAME: &str = "_spotify-connect._tcp";
#[allow(unused)]
const TXT_RECORD: [&str; 2] = ["VERSION=1.0", "CPath=/"];

#[cfg(feature = "with-avahi")]
async fn avahi_task(
    name: Cow<'static, str>,
    port: u16,
    entry_group: &mut Option<avahi::EntryGroupProxy<'_>>,
) -> Result<(), DiscoveryError> {
    use self::avahi::{EntryGroupState, ServerProxy};
    use futures_util::StreamExt;

    let conn = zbus::Connection::system().await?;

    // Wait for the daemon to show up.
    // On error: Failed to listen for NameOwnerChanged signal => Fatal DBus issue
    let bus = zbus::fdo::DBusProxy::new(&conn).await?;
    let mut stream = bus
        .receive_name_owner_changed_with_args(&[(0, "org.freedesktop.Avahi")])
        .await?;

    loop {
        // Wait for Avahi daemon to be started
        'wait_avahi: {
            while let Poll::Ready(Some(_)) = futures_util::poll!(stream.next()) {
                // Drain queued name owner changes, since we're going to connect in a second
            }

            // Ping after we connected to the signal since it might have shown up in the meantime
            if let Ok(avahi_peer) =
                zbus::fdo::PeerProxy::new(&conn, "org.freedesktop.Avahi", "/").await
            {
                if avahi_peer.ping().await.is_ok() {
                    log::debug!("Pinged Avahi: Available");
                    break 'wait_avahi;
                }
            }
            log::warn!("Failed to connect to Avahi, zeroconf discovery will not work until avahi-daemon is started. Check that it is installed and running");

            // If it didn't, wait for the signal
            match stream.next().await {
                Some(_signal) => {
                    log::debug!("Avahi appeared");
                    break 'wait_avahi;
                }
                // The stream ended, but this should never happen
                None => {
                    return Err(zbus::Error::Failure("DBus disappeared".to_owned()).into());
                }
            }
        }

        // Connect to Avahi and publish the service
        let avahi_server = ServerProxy::new(&conn).await?;
        log::trace!("Connected to Avahi");

        *entry_group = Some(avahi_server.entry_group_new().await?);

        let mut entry_group_state_stream = entry_group
            .as_mut()
            .unwrap()
            .receive_state_changed()
            .await?;

        entry_group
            .as_mut()
            .unwrap()
            .add_service(
                -1, // AVAHI_IF_UNSPEC
                -1, // IPv4 and IPv6
                0,  // flags
                &name,
                DNS_SD_SERVICE_NAME, // type
                "",                  // domain: let the server choose
                "",                  // host: let the server choose
                port,
                &TXT_RECORD.map(|s| s.as_bytes()),
            )
            .await?;

        entry_group.as_mut().unwrap().commit().await?;
        log::debug!("Commited zeroconf service with name {}", &name);

        'monitor_service: loop {
            tokio::select! {
                Some(state_changed) = entry_group_state_stream.next() => {
                    let (state, error) = match state_changed.args() {
                        Ok(sc) => (sc.state, sc.error),
                        Err(e) => {
                            log::warn!("Error on receiving EntryGroup state from Avahi: {}", e);
                            continue 'monitor_service;
                        }
                    };
                    match state {
                        EntryGroupState::Uncommited | EntryGroupState::Registering => {
                            // Not yet registered, ignore.
                        }
                        EntryGroupState::Established => {
                            log::info!("Published zeroconf service");
                        }
                        EntryGroupState::Collision => {
                            // This most likely means that spotipi has unintentionally been started twice.
                            // Thus, don't retry with a new name, but abort.
                            //
                            // Note that the error would usually already be returned by
                            // entry_group.add_service above, so this state_changed handler
                            // won't be hit.
                            //
                            // EntryGroup has been withdrawn at this point already!
                            log::error!("zeroconf collision for name '{}'", &name);
                            return Err(zbus::Error::Failure(format!("zeroconf collision for name: {}", name)).into());
                        }
                        EntryGroupState::Failure => {
                            // TODO: Back off/treat as fatal?
                            // EntryGroup has been withdrawn at this point already!
                            // There seems to be no code in Avahi that actually sets this state.
                            log::error!("zeroconf failure: {}", error);
                            return Err(zbus::Error::Failure(format!("zeroconf failure: {}", error)).into());
                        }
                    }
                }
                _name_owner_change = stream.next() => {
                    break 'monitor_service;
                }
            }
        }

        // Avahi disappeared (or the service was immediately taken over by a
        // new daemon) => drop all handles, and reconnect
        log::info!("Avahi disappeared, trying to reconnect");
    }
}

#[cfg(feature = "with-avahi")]
fn launch_avahi(
    name: Cow<'static, str>,
    _zeroconf_ip: Vec<std::net::IpAddr>,
    port: u16,
    status_tx: mpsc::UnboundedSender<DiscoveryEvent>,
) -> Result<DnsSdHandle, Error> {
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let task_handle = tokio::spawn(async move {
        let mut entry_group = None;
        tokio::select! {
            res = avahi_task(name, port, &mut entry_group) => {
                if let Err(e) = res {
                    log::error!("Avahi error: {}", e);
                    let _ = status_tx.send(DiscoveryEvent::ZeroconfError(e));
                }
            },
            _ = shutdown_rx => {
                if let Some(entry_group) = entry_group.as_mut() {
                    if let Err(e) = entry_group.free().await {
                        log::warn!("Failed to un-publish zeroconf service: {}", e);
                    } else {
                        log::debug!("Un-published zeroconf service");
                    }
                }
            },
        }
    });

    Ok(DnsSdHandle {
        task_handle,
        shutdown_tx,
    })
}

#[cfg(feature = "with-dns-sd")]
fn launch_dns_sd(
    name: Cow<'static, str>,
    _zeroconf_ip: Vec<std::net::IpAddr>,
    port: u16,
    status_tx: mpsc::UnboundedSender<DiscoveryEvent>,
) -> Result<DnsSdHandle, Error> {
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let task_handle = tokio::task::spawn_blocking(move || {
        let inner = move || -> Result<(), DiscoveryError> {
            let svc = dns_sd::DNSService::register(
                Some(name.as_ref()),
                DNS_SD_SERVICE_NAME,
                None,
                None,
                port,
                &TXT_RECORD,
            )
            .map_err(|e| DiscoveryError::DnsSdError(Box::new(e)))?;

            let _ = shutdown_rx.blocking_recv();

            std::mem::drop(svc);

            Ok(())
        };

        if let Err(e) = inner() {
            log::error!("dns_sd error: {}", e);
            let _ = status_tx.send(DiscoveryEvent::ZeroconfError(e));
        }
    });

    Ok(DnsSdHandle {
        shutdown_tx,
        task_handle,
    })
}

#[cfg(feature = "with-libmdns")]
fn launch_libmdns(
    name: Cow<'static, str>,
    zeroconf_ip: Vec<std::net::IpAddr>,
    port: u16,
    status_tx: mpsc::UnboundedSender<DiscoveryEvent>,
) -> Result<DnsSdHandle, Error> {
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let task_handle = tokio::task::spawn_blocking(move || {
        let inner = move || -> Result<(), DiscoveryError> {
            let responder = if !zeroconf_ip.is_empty() {
                libmdns::Responder::spawn_with_ip_list(
                    &tokio::runtime::Handle::current(),
                    zeroconf_ip,
                )
            } else {
                libmdns::Responder::spawn(&tokio::runtime::Handle::current())
            }
            .map_err(|e| DiscoveryError::DnsSdError(Box::new(e)))?;

            let svc = responder.register(
                DNS_SD_SERVICE_NAME.to_owned(),
                name.into_owned(),
                port,
                &TXT_RECORD,
            );

            let _ = shutdown_rx.blocking_recv();

            std::mem::drop(svc);

            Ok(())
        };

        if let Err(e) = inner() {
            log::error!("libmdns error: {}", e);
            let _ = status_tx.send(DiscoveryEvent::ZeroconfError(e));
        }
    });

    Ok(DnsSdHandle {
        shutdown_tx,
        task_handle,
    })
}

impl Builder {
    /// Starts a new builder using the provided device and client IDs.
    pub fn new<T: Into<String>>(device_id: T, client_id: T) -> Self {
        Self {
            server_config: server::Config {
                name: "Spotipi".into(),
                device_type: DeviceType::default(),
                is_group: false,
                device_id: device_id.into(),
                client_id: client_id.into(),
            },
            port: 0,
            zeroconf_ip: vec![],
            zeroconf_backend: None,
        }
    }

    /// Sets the name to be displayed. Default is `"SpotiPi"`.
    pub fn name(mut self, name: impl Into<Cow<'static, str>>) -> Self {
        self.server_config.name = name.into();
        self
    }

    /// Sets the device type which is visible as icon in other Spotify clients. Default is `AVR`.
    pub fn device_type(mut self, device_type: DeviceType) -> Self {
        self.server_config.device_type = device_type;
        self
    }

    /// Sets whether the device is a group. This affects the icon in Spotify clients. Default is `false`.
    pub fn is_group(mut self, is_group: bool) -> Self {
        self.server_config.is_group = is_group;
        self
    }

    /// Set the ip addresses on which it should listen to incoming connections. The default is all interfaces.
    pub fn zeroconf_ip(mut self, zeroconf_ip: Vec<std::net::IpAddr>) -> Self {
        self.zeroconf_ip = zeroconf_ip;
        self
    }

    /// Set the zeroconf (MDNS and DNS-SD) implementation to use.
    pub fn zeroconf_backend(mut self, zeroconf_backend: DnsSdServiceBuilder) -> Self {
        self.zeroconf_backend = Some(zeroconf_backend);
        self
    }

    /// Sets the port on which it should listen to incoming connections.
    /// The default value `0` means any port.
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Sets up the [`Discovery`] instance.
    ///
    /// # Errors
    /// If setting up the mdns service or creating the server fails, this function returns an error.
    pub fn launch(self) -> Result<Discovery, Error> {
        let name = self.server_config.name.clone();
        let zeroconf_ip = self.zeroconf_ip;

        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let mut port = self.port;
        let server = DiscoveryServer::new(self.server_config, &mut port, event_tx.clone())?;

        let launch_svc = self.zeroconf_backend.unwrap_or(find(None)?);
        let svc = launch_svc(name, zeroconf_ip, port, event_tx)?;
        Ok(Discovery {
            server,
            svc,
            event_rx,
        })
    }
}

impl Discovery {
    /// Starts a [`Builder`] with the provided device id.
    pub fn builder<T: Into<String>>(device_id: T, client_id: T) -> Builder {
        Builder::new(device_id, client_id)
    }

    /// Create a new instance with the specified device id and default paramaters.
    pub fn new<T: Into<String>>(device_id: T, client_id: T) -> Result<Self, Error> {
        Self::builder(device_id, client_id).launch()
    }

    pub async fn shutdown(self) {
        tokio::join!(self.server.shutdown(), self.svc.shutdown(),);
    }
}

impl Stream for Discovery {
    type Item = Credentials;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.event_rx).poll_recv(cx) {
            // Yields credentials
            Poll::Ready(Some(DiscoveryEvent::Credentials(creds))) => Poll::Ready(Some(creds)),
            // Also terminate the stream on fatal server or MDNS/DNS-SD errors.
            Poll::Ready(Some(
                DiscoveryEvent::ServerError(_) | DiscoveryEvent::ZeroconfError(_),
            )) => Poll::Ready(None),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
