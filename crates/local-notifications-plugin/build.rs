const COMMANDS: &[&str] = &[
    "permissionStatus",
    "requestPermission",
    "openSettings",
    "schedule",
    "cancel",
    "reconcileScheduled",
    "takePendingNavigation",
    "peekPendingNavigation",
    "ackPendingNavigation",
    "pushToken",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("./android")
        .try_build()
        .expect("local notifications plugin build configuration must be valid");
}
