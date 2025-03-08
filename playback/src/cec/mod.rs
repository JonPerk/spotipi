use log::trace;

use cec_rs::{
    CecAdapterType, CecCommand, CecConnection, CecConnectionCfgBuilder, CecConnectionResult, CecDeviceType, CecDeviceTypeVec, CecKeypress, CecLogMessage, CecLogicalAddress, CecLogicalAddresses, CecVendorId, KnownAndRegisteredCecLogicalAddress, KnownCecAudioStatus, KnownCecLogicalAddress, TryFromCecAudioStatusError
};
use std::{collections::HashSet, ffi::CString, sync::Arc, time::Duration};

pub struct CecClient {
    connection: CecConnection,
    audiosystem_status: KnownCecAudioStatus,
}

impl CecClient {
    fn on_key_press(keypress: CecKeypress) {
        trace!(
            "onKeyPress: {:?}, keycode: {:?}, duration: {:?}",
            keypress,
            keypress.keycode,
            keypress.duration
        );
    }

    fn on_command_received(command: CecCommand) {
        trace!(
            "onCommandReceived:  opcode: {:?}, initiator: {:?}",
            command.opcode,
            command.initiator
        );
    }

    fn on_log_level(log_message: CecLogMessage) {
        trace!(
            "logMessageRecieved:  time: {}, level: {}, message: {}",
            log_message.time.as_secs(),
            log_message.level,
            log_message.message
        );
    }

    pub fn fetch_audiosystem_status(&self) -> KnownCecAudioStatus {
        debug!("Fetching audiosystem status");
        self.connection.audio_get_status()
            .or_else(|err| {
                error!("Fetch Audio Status failed with\n{err:?}");
                Ok::<KnownCecAudioStatus, TryFromCecAudioStatusError>(self.audiosystem_status)
            })
            .inspect(|audio| {
                debug!("New audiosystem status\n{audio:?}");
                // self.audiosystem_status = *audio;
            })
            .unwrap()
    }

    pub fn get_audiosystem_status(&self) -> KnownCecAudioStatus {
        self.audiosystem_status
    }

    pub fn update_volume(&self, new_volume: u16) -> KnownCecAudioStatus {
        let old_volume = self.fetch_audiosystem_status().volume() as i32;
        let diff = new_volume as i32 - old_volume;

        if diff > 0 {
            for _ in 0..diff {
                let _ = self.connection.volume_up(true)
                    .or_else(|err| {
                        error!("Volume up failed, retrying\n{err:?}");
                        self.connection.volume_up(true).or_else(|err| {
                            error!("Volume up failed twice, not retrying\n{err:?}");
                            Ok::<KnownCecAudioStatus, TryFromCecAudioStatusError>(self.audiosystem_status)
                        })
                    }).unwrap();
            };
        } else if diff < 0 {
            for _ in diff..0 {
                let _ = self.connection.volume_down(true)
                    .or_else(|err| {
                        error!("Volume down failed, retrying\n{err:?}");
                        self.connection.volume_down(true).or_else(|err| {
                            error!("Volume down failed twice, not retrying\n{err:?}");
                            Ok::<KnownCecAudioStatus, TryFromCecAudioStatusError>(self.audiosystem_status)
                        })
                    }).unwrap();
            };
        } else {
            return self.audiosystem_status
        }
        self.fetch_audiosystem_status()
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
    }

    pub fn deactivate_source(&self) {
        debug!("Setting inactive source");
        debug!("Deactivating source and setting to playback device");
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

    pub fn new/*<F>*/(
        device_name: String,
        port: CString
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
            // .physical_address(1100)
            .key_press_callback(Box::new(Self::on_key_press))
            .command_received_callback(Box::new(Self::on_command_received))
            .log_message_callback(Box::new(Self::on_log_level))
            .device_types(CecDeviceTypeVec::new(CecDeviceType::RecordingDevice))
            .wake_devices(default_devices.clone())
            .autowake_avr(true)
            .power_off_devices(default_devices)
            .power_off_on_standby(false)
            .tv_vendor(CecVendorId::Samsung.repr())
            .activate_source(false)
            .open_timeout(Duration::from_secs(10))
            .adapter_type(CecAdapterType::Linux)
            .build()
            .unwrap();
        let connection = cfg.open().unwrap();
        info!("CEC connection opened");
        let cec = Self { connection, audiosystem_status: KnownCecAudioStatus::new(0, true)  };
        Arc::new(cec)
    }
}