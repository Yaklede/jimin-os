const COMMANDS: &[&str] = &[
    "permissionStatus",
    "requestPermission",
    "openSettings",
    "missedCalls",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("./android")
        .try_build()
        .expect("device signals plugin build configuration must be valid");
}
