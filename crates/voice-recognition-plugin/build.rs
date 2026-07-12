const COMMANDS: &[&str] = &["start", "stop", "cancel"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("./android")
        .try_build()
        .expect("voice recognition plugin build configuration must be valid");
}
