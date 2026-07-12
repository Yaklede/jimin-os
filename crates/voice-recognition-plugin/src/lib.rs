use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "io.jimin.voicerecognition";

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("voice-recognition")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            _api.register_android_plugin(PLUGIN_IDENTIFIER, "VoiceRecognitionPlugin")?;
            Ok(())
        })
        .build()
}
