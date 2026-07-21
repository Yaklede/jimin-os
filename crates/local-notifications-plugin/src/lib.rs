use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "io.jimin.localnotifications";

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("local-notifications")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            _api.register_android_plugin(PLUGIN_IDENTIFIER, "LocalNotificationsPlugin")?;
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod tests {
    #[test]
    fn android_implementation_keeps_the_private_reminder_contract() {
        const SOURCE: &str = include_str!("../android/src/main/java/LocalNotificationsPlugin.kt");

        assert!(SOURCE.contains("NotificationManager.IMPORTANCE_DEFAULT"));
        assert!(SOURCE.contains("Notification.VISIBILITY_PRIVATE"));
        assert!(SOURCE.contains("setAndAllowWhileIdle"));
        assert!(SOURCE.contains("stableReminderKey"));
        assert!(SOURCE.contains("PendingIntent.FLAG_IMMUTABLE"));
        assert!(SOURCE.contains("Settings.ACTION_APP_NOTIFICATION_SETTINGS"));
        assert!(SOURCE.contains("ActivityCompat.shouldShowRequestPermissionRationale"));
        assert!(SOURCE.contains("postNotificationsRequested"));
        assert!(SOURCE.contains("fun peekPendingNavigation"));
        assert!(SOURCE.contains("fun ackPendingNavigation"));
        assert!(SOURCE.contains("extraTargetAt"));
        assert!(SOURCE.contains("fun reconcileScheduled"));
        assert!(SOURCE.contains("scheduledReminderKeys"));
        assert!(SOURCE.contains("getStringExtra(extraInstallNonce) != installationNonce(context)"));
        assert!(SOURCE.contains("validNavigationSemantics"));
        assert!(SOURCE.contains("rememberScheduledReminder(activity, args)"));
        assert!(SOURCE.contains("restoreScheduledReminders(context)"));
        assert!(SOURCE.contains("validateScheduleArgs(args, requireFutureTrigger = false)"));
        assert!(SOURCE.contains("args.targetAtEpochMillis <= now"));
        assert!(SOURCE.contains("now + restoreDeliveryDelayMillis"));
        assert_eq!(SOURCE.matches("scheduleAlarm(activity, args)").count(), 1);
        assert_eq!(SOURCE.matches("setShowBadge(false)").count(), 1);
        assert!(SOURCE.contains("forgetScheduledReminder(context, itemType, itemId)"));
        assert!(SOURCE.contains("class JiminFirebaseMessagingService"));
        assert!(SOURCE.contains("override fun onNewToken"));
        assert!(SOURCE.contains("override fun onMessageReceived"));
        assert!(SOURCE.contains("fun pushToken"));
    }

    #[test]
    fn android_manifest_registers_a_private_alarm_receiver() {
        const MANIFEST: &str = include_str!("../android/src/main/AndroidManifest.xml");

        assert!(MANIFEST.contains("android.permission.POST_NOTIFICATIONS"));
        assert!(MANIFEST.contains("android.permission.RECEIVE_BOOT_COMPLETED"));
        assert!(MANIFEST.contains("io.jimin.localnotifications.ReminderReceiver"));
        assert!(MANIFEST.contains("io.jimin.localnotifications.ReminderBootReceiver"));
        assert!(MANIFEST.contains("io.jimin.localnotifications.JiminFirebaseMessagingService"));
        assert!(MANIFEST.contains("com.google.firebase.MESSAGING_EVENT"));
        assert!(MANIFEST.contains("android.intent.action.BOOT_COMPLETED"));
        assert!(MANIFEST.contains("android:exported=\"false\""));
    }
}
