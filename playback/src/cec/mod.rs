use crate::{config::VolumeCtrl, mixer::mappings::MappedCtrl};

use arrayvec::ArrayVec;
use cec_rs::{
    CecAdapterType, CecCommand, CecConnection, CecConnectionCfgBuilder, CecConnectionResult, 
    CecDatapacket, CecDeviceType, CecDeviceTypeVec, CecKeypress, CecLogLevel, CecLogMessage, 
    CecLogicalAddress, CecLogicalAddresses, CecOpcode, CecPowerStatus, 
    KnownAndRegisteredCecLogicalAddress, KnownCecLogicalAddress
};
use portable_atomic::{AtomicBool, AtomicU8};
use std::{collections::HashSet, ffi::CString, sync::{atomic::Ordering, Arc}, time::Duration};
use tokio::{sync::mpsc::UnboundedSender, task::JoinSet};

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum CecEvent {
    PowerIsOnChange(bool),
    /// Order: old, new
    VolumeChange(u16,u16),
}

pub struct CecClient {
    connection: CecConnection,
    volume_ctrl: VolumeCtrl,
    volume_steps: u16,
    enable_volume_control: bool,
    run: AtomicBool,
    is_active: AtomicBool,
}

// CEC state is static because cec-rs has static callbacks, quick and hacky
static DEVICE_VOLUME:AtomicU8 = AtomicU8::new(0);
static LOCAL_VOLUME:AtomicU8 = AtomicU8::new(0);
static VOLUME_IS_INIT:AtomicBool = AtomicBool::new(false);
static DEVICE_IS_ON:AtomicBool = AtomicBool::new(false);
static LOCAL_IS_ON:AtomicBool = AtomicBool::new(false);

impl CecClient {
    fn on_key_press(keypress: CecKeypress) {
        trace!(
            "Key Press: {:?}, keycode: {:?}, duration: {:?}",
            keypress,
            keypress.keycode,
            keypress.duration
        );
    }

    fn on_command_received(command: CecCommand) {
        trace!(
            "Command Received:  opcode: {:?}, initiator: {:?}, params: {:?}",
            command.opcode,
            command.initiator,
            command.parameters.0
        );
        // todo filter by initiator
        match command.opcode {
            CecOpcode::ReportPowerStatus => {
                let power = command.parameters.0[0] > 0;
                debug!("Got audio status: power: {power}");
                DEVICE_IS_ON.store(power, Ordering::SeqCst);
            },
            CecOpcode::ReportAudioStatus => {
                let mut volume = command.parameters.0[0];
                debug!("Got audio status: volume: {volume}");
                // When device is muted CEC reports that as volume + 0x80
                if volume > 0x80 {
                    volume -= 0x80;
                    debug!("Device is muted: actual volume: {volume}");
                }
                DEVICE_VOLUME.store(volume, Ordering::SeqCst);
                VOLUME_IS_INIT.store(true, Ordering::SeqCst);
            },
            CecOpcode::SetSystemAudioMode => {
                let power = command.parameters.0[0] > 0;
                debug!("Got audio status: power: {power}");
                DEVICE_IS_ON.store(power, Ordering::SeqCst);
            }
            _ => ()
        }
    }

    fn on_log_level(log_message: CecLogMessage) {
        // TODO better filtering
        if log_message.level != CecLogLevel::Debug {
            trace!(
                "Log Message Recieved:  time: {}, level: {}, message: {}",
                log_message.time.as_secs(),
                log_message.level,
                log_message.message
            );
        }
    }

    pub fn get_power_status(&self) -> bool {
        DEVICE_IS_ON.load(Ordering::SeqCst)
    }

    pub fn fetch_power_status(&self) {
        debug!("Fetching audiosystem power status");
        let command = CecCommand {
            opcode: CecOpcode::GiveDevicePowerStatus,
            initiator: CecLogicalAddress::Recordingdevice1,
            destination: CecLogicalAddress::Audiosystem,
            parameters: CecDatapacket(ArrayVec::new()),
            transmit_timeout: Duration::from_secs(5),
            ack: true,
            eom: true,
            opcode_set: true,
        };
        self.connection.transmit(command)
            .or_else(|err| {
                error!("Fetch audiosystem status failed with\n{err:?}");
                CecConnectionResult::Ok(())
            })
            .unwrap();
    }

    fn convert_volume(volume:u8) -> u16 {
        u16::try_from(
            (VolumeCtrl::MAX_VOLUME as u32) * (volume as u32) / 100
        ).unwrap_or_else(|_| VolumeCtrl::MAX_VOLUME)
    }

    pub fn get_volume(&self) -> u16 {
        Self::convert_volume(DEVICE_VOLUME.load(Ordering::SeqCst))
    }

    pub fn fetch_volume(&self) {
        debug!("Fetching audiosystem status");
        if self.enable_volume_control {
            let command = CecCommand {
                opcode: CecOpcode::GiveAudioStatus,
                initiator: CecLogicalAddress::Recordingdevice1,
                destination: CecLogicalAddress::Audiosystem,
                parameters: CecDatapacket(ArrayVec::new()),
                transmit_timeout: Duration::from_secs(5),
                ack: true,
                eom: true,
                opcode_set: true,
            };
            self.connection.transmit(command)
                .or_else(|err| {
                    error!("Fetch audiosystem status failed with\n{err:?}");
                    CecConnectionResult::Ok(())
                })
                .unwrap();
        }
    }

    pub fn is_volume_enabled(&self) -> bool {
        self.enable_volume_control.clone()
    }

    pub fn is_volume_init(&self) -> bool {
        VOLUME_IS_INIT.load(Ordering::SeqCst)
    }

    pub fn volume_up(&self) {
        trace!("Send volume up");
        if self.get_power_status() {
            self.connection
                .send_keypress(
                    CecLogicalAddress::Audiosystem, 
                    cec_rs::CecUserControlCode::VolumeUp, 
                    false
                )
                .or_else(|err| {
                    error!("Volume up send key press failed, retrying\n{err:?}");
                    self.connection
                        .send_keypress(
                            CecLogicalAddress::Audiosystem, 
                            cec_rs::CecUserControlCode::VolumeUp, 
                            false
                        )
                        .or_else(|err| {
                            error!("Volume up send key press twice, not retrying\n{err:?}");
                            CecConnectionResult::Ok(())
                        })
                }).unwrap();
        }
    }

    pub fn volume_down(&self) {
        trace!("Send volume down");
        if self.get_power_status() {
            self.connection
                .send_keypress(
                    CecLogicalAddress::Audiosystem, 
                    cec_rs::CecUserControlCode::VolumeDown, 
                    false
                )
                .or_else(|err| {
                    error!("Volume down send key press failed, retrying\n{err:?}");
                    self.connection
                    .send_keypress(
                        CecLogicalAddress::Audiosystem, 
                        cec_rs::CecUserControlCode::VolumeDown, 
                        false
                    )
                        .or_else(|err| {
                            error!("Volume down send key press twice, not retrying\n{err:?}");
                            CecConnectionResult::Ok(())
                        })
                }).unwrap();
        }
    }

    pub fn set_volume(&self, new_volume: u16) {
        if self.enable_volume_control {
            debug!("Updating volume");
            if self.get_power_status() {
                self.fetch_volume();
                let mapped_new_volume = self.volume_ctrl.to_mapped(new_volume);
                let old_volume = DEVICE_VOLUME.load(Ordering::SeqCst) as f64;
                let mapped_old_volume = old_volume / 100f64;
                let diff = mapped_new_volume - mapped_old_volume;
                let steps = self.volume_ctrl.to_steps(diff, self.volume_steps);
                debug!(
                    "New volume {} maps to {:.3} - old volume {} maps to {:.3} = diff {:.3} steps {}",
                    new_volume,
                    mapped_new_volume,
                    old_volume,
                    mapped_old_volume,
                    diff,
                    steps,
                );

                if steps > 0 {
                    for _ in 0..steps {
                        self.volume_up();
                    };
                } else if steps < 0 {
                    for _ in steps..0 {
                        self.volume_down();
                    };
                } else {
                    debug!("Volume is not changed, no action taken");
                }
                debug!("Volume updates sent over CEC");
            } else {
                debug!("Device is off, cannot update volume");
            }
        } else {
            debug!("Volume control disabled, no action taken");
        }
    }

    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::SeqCst)
    }

    pub fn activate_source(&self) {
        debug!("Setting active source");
        debug!("Turning on audiosystem");
        self.connection.send_power_on_devices(CecLogicalAddress::Audiosystem)
            .or_else(|err| {
                error!("Turning on audiosystem failed, retrying\n{err:?}");
                self.connection.send_power_on_devices(CecLogicalAddress::Audiosystem)
                    .or_else(|err| {
                        error!("Turning on audiosystem failed, not retrying\n{err:?}");
                        CecConnectionResult::Ok(())
                    })
            })
            .unwrap();
        
        debug!("Activating source");
        self.connection.set_active_source(self.connection.0.device_types.0[0])
            .or_else(|err| {
                error!("Activating source failed, retrying\n{err:?}");
                self.connection.set_active_source(self.connection.0.device_types.0[0])
                    .or_else(|err| {
                        error!("Activating source failed, not retrying\n{err:?}");
                        CecConnectionResult::Ok(())
                    })
            })
            .unwrap();
        
        self.is_active.store(true, Ordering::SeqCst);
    }

    pub fn deactivate_source(&self) {
        debug!("Setting inactive source");
        self.is_active.store(false, Ordering::SeqCst);

        // todo with some devices this will always return standby, replace with custom transmit
        debug!("Deactivating source and setting to playback device if on");
        if self.connection.get_device_power_status(CecLogicalAddress::Playbackdevice1) == CecPowerStatus::On {
            debug!("Playback is on, setting as active");
            self.connection.set_active_source(CecDeviceType::PlaybackDevice)
            .or_else(|err| {
                error!("Activating source failed, retrying\n{err:?}");
                self.connection.set_active_source(CecDeviceType::PlaybackDevice)
                    .or_else(|err| {
                        error!("Activating source failed, not retrying\n{err:?}");
                        CecConnectionResult::Ok(())
                    })
            })
            .unwrap();
        }

        debug!("Turning off audiosystem");
        self.connection.send_standby_devices(CecLogicalAddress::Audiosystem)
            .or_else(|err| {
                error!("Turning off audiosystem failed, retrying\n{err:?}");
                self.connection.send_standby_devices(CecLogicalAddress::Audiosystem)
                    .or_else(|err| {
                        error!("Turning off audiosystem failed, not retrying\n{err:?}");
                        CecConnectionResult::Ok(())
                    })
            })
            .unwrap();
    }

    /// listens for commands and publishes to subscription
    /// returns a function to close connection which itself returns a JoinSet
    pub fn run(cec:Arc<CecClient>, sender: UnboundedSender<CecEvent>) -> Box<dyn FnOnce() -> JoinSet<()> + Send> {
        info!("Running CEC Client");
        let mut set = JoinSet::new();
        cec.run.store(true, Ordering::SeqCst);

        // listen for device power changes
        let power_cec = cec.clone();
        let power_sender = sender.clone();
        set.spawn(async move {
            while power_cec.run.load(Ordering::SeqCst) {
                let power = DEVICE_IS_ON.load(Ordering::SeqCst);
                let current_power = LOCAL_IS_ON.load(Ordering::SeqCst);
                
                if power != current_power {
                    debug!("Power status changed from {current_power} to {power}");
                    LOCAL_IS_ON.store(power, Ordering::SeqCst);
                    power_sender.send(CecEvent::PowerIsOnChange(power)).unwrap();
                }
                tokio::task::yield_now().await;
            }
        });

        // Sync volume initially
        let init_volume_cec = cec.clone();
        let init_volume_sender = sender.clone();
        set.spawn(async move {
            debug!("Initial sync of volume from device");
            init_volume_cec.fetch_volume();
            let mut is_uninit = true;
            while is_uninit {
                let is_init = VOLUME_IS_INIT.load(Ordering::SeqCst);
                if is_init {
                    let volume = DEVICE_VOLUME.load(Ordering::SeqCst);
                    debug!("Volume intialized to {volume}");
                    LOCAL_VOLUME.store(volume, Ordering::SeqCst);
                    init_volume_sender.send(
                        CecEvent::VolumeChange(Self::convert_volume(volume),Self::convert_volume(volume))
                    ).unwrap();
                    is_uninit = false;
                }
                tokio::task::yield_now().await;
            }
        });

        // Listen for device volume changes
        let volume_cec = cec.clone();
        let volume_sender = sender.clone();
        set.spawn(async move {
            if volume_cec.enable_volume_control {
                // ask if source has changed volume once per second(ish) since that is not broadcast
                const AUDIO_POLL_FREQ: u64 = 200;
                const LOOP_SLEEP_MILLIS: u64 = 5;
                debug!("Volume change polling freq: {} ms", LOOP_SLEEP_MILLIS * AUDIO_POLL_FREQ);
                let mut loops: u64 = 0;
                while volume_cec.run.load(Ordering::SeqCst) {
                    if volume_cec.is_active() {
                        loops += 1;
                        if loops < AUDIO_POLL_FREQ {
                            let volume = DEVICE_VOLUME.load(Ordering::SeqCst);
                            let current_volume = LOCAL_VOLUME.load(Ordering::SeqCst);
                            if volume != current_volume {
                                debug!("Volume changed from {current_volume} to {volume}");
                                LOCAL_VOLUME.store(volume, Ordering::SeqCst);
                                volume_sender.send(
                                    CecEvent::VolumeChange(
                                        Self::convert_volume(current_volume),Self::convert_volume(volume)
                                    )
                                ).unwrap();
                            }
                            let _ = tokio::time::sleep(Duration::from_millis(LOOP_SLEEP_MILLIS)).await;
                        } else {
                            loops = 0;
                            volume_cec.fetch_volume();
                        }
                    }
                    tokio::task::yield_now().await;
                }
            }
        });

        // Return boxed function to close connection and join handles of tasks
        Box::new(move || {
            info!("Shutting CEC Client down");
            debug!("Stopping task loops");
            cec.run.store(false, Ordering::SeqCst);
            debug!("Deactivating");
            cec.deactivate_source();
            info!("CEC Client closed");
            set
        })
    }

    pub fn new/*<F>*/(
        device_name: String,
        port: CString,
        volume_ctrl: VolumeCtrl,
        volume_steps: u16,
        enable_volume_control: bool, 
    ) -> Arc<Self>
    // where
    //     F: FnOnce() -> Box<dyn Sink> + Send + 'static,
    {
        let default_devices = CecLogicalAddresses { 
            primary: KnownCecLogicalAddress::new(CecLogicalAddress::Audiosystem).unwrap(), 
            addresses: HashSet::<KnownAndRegisteredCecLogicalAddress>::new() 
        };
        let cfg = CecConnectionCfgBuilder::default()
            .port(port)
            .device_name(device_name)
            .key_press_callback(Box::new(Self::on_key_press))
            .command_received_callback(Box::new(Self::on_command_received))
            .log_message_callback(Box::new(Self::on_log_level))
            .device_types(CecDeviceTypeVec::new(CecDeviceType::RecordingDevice))
            .wake_devices(default_devices.clone())
            .power_off_devices(default_devices)
            .power_off_on_standby(false)
            .activate_source(false)
            .open_timeout(Duration::from_secs(10))
            .adapter_type(CecAdapterType::Linux)
            .build()
            .unwrap();
        let connection = cfg.open().unwrap();
        info!("CEC connection opened, volume control {enable_volume_control}");
        let cec = Self { 
            connection, 
            volume_ctrl,
            volume_steps,
            enable_volume_control, 
            run: AtomicBool::new(false),
            is_active: AtomicBool::new(false)  
        };
        // get intial values
        cec.fetch_power_status();
        if enable_volume_control {
            cec.fetch_volume();
        }
        Arc::new(cec)
    }
}