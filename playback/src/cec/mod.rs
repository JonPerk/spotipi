use log::trace;

use cec_rs::{
    CecCommand, CecConnection, CecConnectionCfgBuilder, CecDeviceType, CecDeviceTypeVec, CecKeypress,
    CecLogMessage,
};
use std::{ffi::CString, sync::Arc};

pub struct CecClient {
    connection: CecConnection,
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

    pub fn new/*<F>*/(
        device_name: String,
        port: CString
    ) -> Arc<Self>
    // where
    //     F: FnOnce() -> Box<dyn Sink> + Send + 'static,
    {
        let cfg = CecConnectionCfgBuilder::default()
            .port(port)
            .device_name(device_name)
            .key_press_callback(Box::new(Self::on_key_press))
            .command_received_callback(Box::new(Self::on_command_received))
            .log_message_callback(Box::new(Self::on_log_level))
            .device_types(CecDeviceTypeVec::new(CecDeviceType::RecordingDevice))
            .build()
            .unwrap();
        let connection = cfg.open().unwrap();
        trace!("CEC is Active Source: {:?}", connection.get_active_source());
        info!("CEC Audio Status: {:?}", connection.audio_get_status());
        // thread::sleep(time::Duration::from_secs(99_999_999));
        Arc::new(Self { connection  })
    }
}