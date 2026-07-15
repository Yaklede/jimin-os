package io.jimin.localnotifications

import android.Manifest
import android.app.Activity
import android.app.AlarmManager
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.webkit.WebView
import androidx.core.app.ActivityCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONObject
import java.util.UUID

private const val channelId = "jimin_os_reminders_v1"
private const val extraItemType = "io.jimin.os.reminder.ITEM_TYPE"
private const val extraItemId = "io.jimin.os.reminder.ITEM_ID"
private const val extraDestination = "io.jimin.os.reminder.DESTINATION"
private const val extraProjectId = "io.jimin.os.reminder.PROJECT_ID"
private const val extraTargetAt = "io.jimin.os.reminder.TARGET_AT"
private const val extraInstallNonce = "io.jimin.os.reminder.INSTALL_NONCE"
private const val extraTitle = "io.jimin.os.reminder.TITLE"
private const val extraBody = "io.jimin.os.reminder.BODY"
private const val reminderActionPrefix = "io.jimin.os.reminder.ALARM."
private const val openActionPrefix = "io.jimin.os.reminder.OPEN."
private const val permissionPreferences = "io.jimin.os.local_notifications"
private const val postNotificationsRequested = "post_notifications_requested"
private const val scheduledReminderKeys = "scheduled_reminder_keys"
private const val installationNonceKey = "installation_nonce"
private const val reminderPayloadPrefix = "scheduled_reminder_payload:"
private const val restoreDeliveryDelayMillis = 1_000L

@InvokeArg
class ScheduleReminderArgs {
  lateinit var itemType: String
  lateinit var itemId: String
  lateinit var destination: String
  lateinit var title: String
  var body: String? = null
  var projectId: String? = null
  var targetAtEpochMillis: Long = 0
  var triggerAtEpochMillis: Long = 0
}

@InvokeArg
class CancelReminderArgs {
  lateinit var itemType: String
  lateinit var itemId: String
}

@InvokeArg
class ReconcileScheduledArgs {
  lateinit var activeKeys: Array<String>
}

private data class ReminderNavigation(
  val itemType: String,
  val itemId: String,
  val destination: String,
  val projectId: String?,
  val targetAtEpochMillis: Long?,
) {
  fun toJsObject(): JSObject =
    JSObject().apply {
      put("itemType", itemType)
      put("itemId", itemId)
      put("destination", destination)
      projectId?.let { put("projectId", it) }
      targetAtEpochMillis?.let { put("targetAtEpochMillis", it) }
    }

  companion object {
    fun from(context: Context, intent: Intent?): ReminderNavigation? {
      if (intent?.action?.startsWith(openActionPrefix) != true) return null
      if (intent.getStringExtra(extraInstallNonce) != installationNonce(context)) return null
      val itemType = intent.getStringExtra(extraItemType) ?: return null
      val itemId = intent.getStringExtra(extraItemId) ?: return null
      val destination = intent.getStringExtra(extraDestination) ?: return null
      val projectId = intent.getStringExtra(extraProjectId)
      val targetAtEpochMillis = intent.getLongExtra(extraTargetAt, 0)
      if (!validNavigationSemantics(itemType, itemId, destination, projectId, targetAtEpochMillis)) {
        return null
      }
      return ReminderNavigation(
        itemType,
        itemId,
        destination,
        projectId,
        targetAtEpochMillis,
      )
    }
  }
}

@TauriPlugin(
  permissions = [Permission(strings = [Manifest.permission.POST_NOTIFICATIONS], alias = "notifications")],
)
class LocalNotificationsPlugin(private val activity: Activity) : Plugin(activity) {
  private var pendingNavigation: ReminderNavigation? = ReminderNavigation.from(activity, activity.intent)

  override fun load(webView: WebView) {
    ensureReminderChannel(activity)
  }

  override fun onNewIntent(intent: Intent) {
    pendingNavigation = ReminderNavigation.from(activity, intent) ?: pendingNavigation
  }

  @Command
  fun permissionStatus(invoke: Invoke) {
    invoke.resolve(permissionStatusResult(activity))
  }

  @Command
  fun requestPermission(invoke: Invoke) {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
      hasPostNotificationsPermission(activity)
    ) {
      invoke.resolve(permissionStatusResult(activity))
      return
    }
    activity
      .getSharedPreferences(permissionPreferences, Context.MODE_PRIVATE)
      .edit()
      .putBoolean(postNotificationsRequested, true)
      .apply()
    requestPermissionForAlias("notifications", invoke, "onNotificationPermission")
  }

  @PermissionCallback
  fun onNotificationPermission(invoke: Invoke) {
    invoke.resolve(permissionStatusResult(activity))
  }

  @Command
  fun openSettings(invoke: Invoke) {
    try {
      val intent =
        Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).apply {
          putExtra(Settings.EXTRA_APP_PACKAGE, activity.packageName)
        }
      if (intent.resolveActivity(activity.packageManager) == null) {
        intent.action = Settings.ACTION_APPLICATION_DETAILS_SETTINGS
        intent.data = Uri.parse("package:${activity.packageName}")
      }
      activity.startActivity(intent)
      invoke.resolve()
    } catch (error: Exception) {
      invoke.reject("휴대폰 알림 설정을 열지 못했어요. 설정에서 Jimin OS를 찾아 주세요.", "NOTIFICATION_SETTINGS_FAILED", error)
    }
  }

  @Command
  fun schedule(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(ScheduleReminderArgs::class.java)
      val error = validateScheduleArgs(args)
      if (error != null) {
        invoke.reject(error.second, error.first)
        return
      }

      scheduleAlarm(activity, args)
      rememberScheduledReminder(activity, args)
      invoke.resolve()
    } catch (error: Exception) {
      invoke.reject("알림을 예약하지 못했어요. 잠시 후 다시 시도해 주세요.", "REMINDER_SCHEDULE_FAILED", error)
    }
  }

  @Command
  fun cancel(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(CancelReminderArgs::class.java)
      if (!validItemType(args.itemType) || !validIdentifier(args.itemId)) {
        invoke.reject("취소할 알림 정보를 확인해 주세요.", "REMINDER_INVALID")
        return
      }
      cancelScheduledReminder(activity, args.itemType, args.itemId)
      forgetScheduledReminder(activity, args.itemType, args.itemId)
      invoke.resolve()
    } catch (error: Exception) {
      invoke.reject("알림 예약을 취소하지 못했어요. 다시 시도해 주세요.", "REMINDER_CANCEL_FAILED", error)
    }
  }

  @Command
  fun reconcileScheduled(invoke: Invoke) {
    try {
      val args = invoke.parseArgs(ReconcileScheduledArgs::class.java)
      val activeKeys = args.activeKeys.toSet()
      if (activeKeys.any { parseReminderKey(it) == null }) {
        invoke.reject("알림 목록을 확인해 주세요.", "REMINDER_INVALID")
        return
      }
      val storedKeys = readScheduledReminderKeys(activity)
      for (key in storedKeys - activeKeys) {
        val (itemType, itemId) = parseReminderKey(key) ?: continue
        cancelScheduledReminder(activity, itemType, itemId)
        forgetScheduledReminder(activity, itemType, itemId)
      }
      writeScheduledReminderKeys(activity, activeKeys)
      invoke.resolve()
    } catch (error: Exception) {
      invoke.reject("지난 알림을 정리하지 못했어요. 다시 시도해 주세요.", "REMINDER_RECONCILE_FAILED", error)
    }
  }

  @Command
  fun takePendingNavigation(invoke: Invoke) {
    val navigation = pendingNavigation ?: ReminderNavigation.from(activity, activity.intent)
    clearPendingNavigation()
    if (navigation == null) invoke.resolve() else invoke.resolve(navigation.toJsObject())
  }

  @Command
  fun peekPendingNavigation(invoke: Invoke) {
    val navigation = pendingNavigation ?: ReminderNavigation.from(activity, activity.intent)
    if (navigation == null) invoke.resolve() else invoke.resolve(navigation.toJsObject())
  }

  @Command
  fun ackPendingNavigation(invoke: Invoke) {
    val args = invoke.parseArgs(CancelReminderArgs::class.java)
    val navigation = pendingNavigation ?: ReminderNavigation.from(activity, activity.intent)
    if (navigation?.itemType == args.itemType && navigation.itemId == args.itemId) {
      clearPendingNavigation()
      invoke.resolve(JSObject().apply { put("acknowledged", true) })
      return
    }
    invoke.resolve(JSObject().apply { put("acknowledged", false) })
  }

  private fun clearPendingNavigation() {
    pendingNavigation = null
    activity.intent?.apply {
      action = null
      removeExtra(extraItemType)
      removeExtra(extraItemId)
      removeExtra(extraDestination)
      removeExtra(extraProjectId)
      removeExtra(extraTargetAt)
      removeExtra(extraInstallNonce)
    }
  }
}

class ReminderReceiver : BroadcastReceiver() {
  override fun onReceive(context: Context, intent: Intent) {
    if (!notificationsEnabled(context)) return

    val itemType = intent.getStringExtra(extraItemType) ?: return
    val itemId = intent.getStringExtra(extraItemId) ?: return
    val destination = intent.getStringExtra(extraDestination) ?: return
    val projectId = intent.getStringExtra(extraProjectId)
    val targetAtEpochMillis =
      if (intent.hasExtra(extraTargetAt)) intent.getLongExtra(extraTargetAt, 0) else null
    val title = intent.getStringExtra(extraTitle) ?: return
    val body = intent.getStringExtra(extraBody)
    ensureReminderChannel(context)

    val launchIntent = context.packageManager.getLaunchIntentForPackage(context.packageName) ?: return
    launchIntent.apply {
      action = openActionPrefix + stableReminderKey(itemType, itemId)
      flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP
      putExtra(extraItemType, itemType)
      putExtra(extraItemId, itemId)
      putExtra(extraDestination, destination)
      projectId?.let { putExtra(extraProjectId, it) }
      targetAtEpochMillis?.let { putExtra(extraTargetAt, it) }
      putExtra(extraInstallNonce, installationNonce(context))
    }
    val contentIntent =
      PendingIntent.getActivity(
        context,
        stableRequestCode(itemType, itemId),
        launchIntent,
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
      )
    val notification =
      NotificationCompat.Builder(context, channelId)
        .setSmallIcon(R.drawable.jimin_notification)
        .setContentTitle(title)
        .setContentText(body)
        .setStyle(body?.let { NotificationCompat.BigTextStyle().bigText(it) })
        .setCategory(NotificationCompat.CATEGORY_REMINDER)
        .setPriority(NotificationCompat.PRIORITY_LOW)
        .setVisibility(NotificationCompat.VISIBILITY_PRIVATE)
        .setOnlyAlertOnce(true)
        .setAutoCancel(true)
        .setContentIntent(contentIntent)
        .build()
    NotificationManagerCompat.from(context).notify(stableRequestCode(itemType, itemId), notification)
    forgetScheduledReminder(context, itemType, itemId)
  }
}

class ReminderBootReceiver : BroadcastReceiver() {
  override fun onReceive(context: Context, intent: Intent) {
    if (intent.action != Intent.ACTION_BOOT_COMPLETED) return
    restoreScheduledReminders(context)
  }
}

private fun permissionStatusResult(activity: Activity): JSObject =
  JSObject().apply {
    val runtimePermissionRequired = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU
    val hasRuntimePermission = hasPostNotificationsPermission(activity)
    val granted = hasRuntimePermission && notificationsEnabled(activity)
    val requested =
      activity
        .getSharedPreferences(permissionPreferences, Context.MODE_PRIVATE)
        .getBoolean(postNotificationsRequested, false)
    val canShowPermissionRequest =
      runtimePermissionRequired &&
        !hasRuntimePermission &&
        (!requested ||
          ActivityCompat.shouldShowRequestPermissionRationale(
            activity,
            Manifest.permission.POST_NOTIFICATIONS,
          ))
    put("status", if (granted) "granted" else "denied")
    put("canRequest", canShowPermissionRequest)
  }

private fun hasPostNotificationsPermission(context: Context): Boolean =
  Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
    ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) ==
    PackageManager.PERMISSION_GRANTED

private fun notificationsEnabled(context: Context): Boolean =
  hasPostNotificationsPermission(context) && NotificationManagerCompat.from(context).areNotificationsEnabled()

private fun ensureReminderChannel(context: Context) {
  if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
  val manager = context.getSystemService(NotificationManager::class.java)
  if (manager.getNotificationChannel(channelId) != null) return
  val channel =
    NotificationChannel(channelId, "일정과 할 일 알림", NotificationManager.IMPORTANCE_LOW).apply {
      description = "일정 시작과 할 일 기한을 알려드려요."
      lockscreenVisibility = Notification.VISIBILITY_PRIVATE
      enableLights(false)
      enableVibration(false)
      setShowBadge(false)
      setSound(null, null)
    }
  manager.createNotificationChannel(channel)
}

private fun reminderIntent(context: Context, args: ScheduleReminderArgs): Intent =
  reminderIntent(context, args.itemType, args.itemId).apply {
    putExtra(extraDestination, args.destination)
    args.projectId?.let { putExtra(extraProjectId, it) }
    putExtra(extraTargetAt, args.targetAtEpochMillis)
    putExtra(extraTitle, args.title.trim())
    putExtra(extraBody, args.body?.trim()?.takeIf { it.isNotEmpty() })
  }

private fun reminderIntent(context: Context, itemType: String, itemId: String): Intent =
  Intent(context, ReminderReceiver::class.java).apply {
    action = reminderActionPrefix + stableReminderKey(itemType, itemId)
    putExtra(extraItemType, itemType)
    putExtra(extraItemId, itemId)
  }

private fun scheduleAlarm(context: Context, args: ScheduleReminderArgs) {
  ensureReminderChannel(context)
  val pendingIntent =
    PendingIntent.getBroadcast(
      context,
      stableRequestCode(args.itemType, args.itemId),
      reminderIntent(context, args),
      PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
    )
  context.getSystemService(AlarmManager::class.java).setAndAllowWhileIdle(
    AlarmManager.RTC_WAKEUP,
    args.triggerAtEpochMillis,
    pendingIntent,
  )
}

private fun cancelScheduledReminder(context: Context, itemType: String, itemId: String) {
  val pendingIntent =
    PendingIntent.getBroadcast(
      context,
      stableRequestCode(itemType, itemId),
      reminderIntent(context, itemType, itemId),
      PendingIntent.FLAG_NO_CREATE or PendingIntent.FLAG_IMMUTABLE,
    )
  if (pendingIntent != null) {
    context.getSystemService(AlarmManager::class.java).cancel(pendingIntent)
    pendingIntent.cancel()
  }
}

private fun readScheduledReminderKeys(context: Context): Set<String> =
  context
    .getSharedPreferences(permissionPreferences, Context.MODE_PRIVATE)
    .getStringSet(scheduledReminderKeys, mutableSetOf())
    ?.toSet()
    ?: emptySet()

private fun writeScheduledReminderKeys(context: Context, keys: Set<String>) {
  context
    .getSharedPreferences(permissionPreferences, Context.MODE_PRIVATE)
    .edit()
    .putStringSet(scheduledReminderKeys, keys.toMutableSet())
    .apply()
}

private fun rememberScheduledReminder(context: Context, args: ScheduleReminderArgs) {
  val key = stableReminderKey(args.itemType, args.itemId)
  val payload =
    JSONObject()
      .put("itemType", args.itemType)
      .put("itemId", args.itemId)
      .put("destination", args.destination)
      .put("title", args.title)
      .put("body", args.body)
      .put("projectId", args.projectId)
      .put("targetAtEpochMillis", args.targetAtEpochMillis)
      .put("triggerAtEpochMillis", args.triggerAtEpochMillis)
      .toString()
  val preferences = context.getSharedPreferences(permissionPreferences, Context.MODE_PRIVATE)
  preferences
    .edit()
    .putStringSet(
      scheduledReminderKeys,
      (readScheduledReminderKeys(context) + key).toMutableSet(),
    )
    .putString(reminderPayloadPrefix + key, payload)
    .apply()
}

private fun forgetScheduledReminder(context: Context, itemType: String, itemId: String) {
  val key = stableReminderKey(itemType, itemId)
  context
    .getSharedPreferences(permissionPreferences, Context.MODE_PRIVATE)
    .edit()
    .putStringSet(
      scheduledReminderKeys,
      (readScheduledReminderKeys(context) - key).toMutableSet(),
    )
    .remove(reminderPayloadPrefix + key)
    .apply()
}

private fun readStoredReminder(context: Context, key: String): ScheduleReminderArgs? {
  val payload =
    context
      .getSharedPreferences(permissionPreferences, Context.MODE_PRIVATE)
      .getString(reminderPayloadPrefix + key, null)
      ?: return null
  return try {
    val json = JSONObject(payload)
    val args =
      ScheduleReminderArgs().apply {
        itemType = json.getString("itemType")
        itemId = json.getString("itemId")
        destination = json.getString("destination")
        title = json.getString("title")
        body = json.optString("body").takeIf { it.isNotEmpty() && it != "null" }
        projectId = json.optString("projectId").takeIf { it.isNotEmpty() && it != "null" }
        targetAtEpochMillis = json.getLong("targetAtEpochMillis")
        triggerAtEpochMillis = json.getLong("triggerAtEpochMillis")
      }
    if (
      stableReminderKey(args.itemType, args.itemId) != key ||
      validateScheduleArgs(args, requireFutureTrigger = false) != null
    ) {
      null
    } else {
      args
    }
  } catch (_: Exception) {
    null
  }
}

private fun restoreScheduledReminders(context: Context) {
  val restoredKeys = mutableSetOf<String>()
  for (key in readScheduledReminderKeys(context)) {
    val args = readStoredReminder(context, key)
    val now = System.currentTimeMillis()
    if (args == null || args.targetAtEpochMillis <= now) {
      context
        .getSharedPreferences(permissionPreferences, Context.MODE_PRIVATE)
        .edit()
        .remove(reminderPayloadPrefix + key)
        .apply()
      continue
    }
    if (args.triggerAtEpochMillis <= now) {
      args.triggerAtEpochMillis =
        minOf(args.targetAtEpochMillis, now + restoreDeliveryDelayMillis)
    }
    try {
      scheduleAlarm(context, args)
      restoredKeys += key
    } catch (_: Exception) {
      // Keep the future payload so the next app reconciliation can retry it.
      restoredKeys += key
    }
  }
  writeScheduledReminderKeys(context, restoredKeys)
}

private fun installationNonce(context: Context): String =
  synchronized(LocalNotificationsPlugin::class.java) {
    val preferences = context.getSharedPreferences(permissionPreferences, Context.MODE_PRIVATE)
    preferences.getString(installationNonceKey, null)?.takeIf { it.length >= 32 }
      ?: UUID.randomUUID().toString().also { nonce ->
        preferences.edit().putString(installationNonceKey, nonce).apply()
      }
  }

private fun parseReminderKey(value: String): Pair<String, String>? {
  val separator = value.indexOf(':')
  if (separator <= 0 || separator == value.lastIndex) return null
  val itemType = value.substring(0, separator)
  val itemId = value.substring(separator + 1)
  return if (validItemType(itemType) && validIdentifier(itemId)) itemType to itemId else null
}

private fun validNavigationSemantics(
  itemType: String,
  itemId: String,
  destination: String,
  projectId: String?,
  targetAtEpochMillis: Long,
): Boolean =
  validItemType(itemType) &&
    validIdentifier(itemId) &&
    destination in setOf("home", "calendar", "projects") &&
    (projectId == null || validIdentifier(projectId)) &&
    (destination != "projects" || projectId != null) &&
    targetAtEpochMillis > 0

private fun validateScheduleArgs(
  args: ScheduleReminderArgs,
  requireFutureTrigger: Boolean = true,
): Pair<String, String>? {
  if (!validItemType(args.itemType) || !validIdentifier(args.itemId)) {
    return "REMINDER_INVALID" to "알림 대상을 확인해 주세요."
  }
  if (args.destination !in setOf("home", "calendar", "projects")) {
    return "REMINDER_INVALID" to "알림에서 열 화면을 확인해 주세요."
  }
  if (args.destination == "projects" && !validIdentifier(args.projectId.orEmpty())) {
    return "REMINDER_INVALID" to "알림에 연결할 프로젝트를 확인해 주세요."
  }
  if (args.projectId != null && !validIdentifier(args.projectId.orEmpty())) {
    return "REMINDER_INVALID" to "알림에 연결할 프로젝트를 확인해 주세요."
  }
  if (args.title.trim().isEmpty() || args.title.length > 120 || (args.body?.length ?: 0) > 240) {
    return "REMINDER_INVALID" to "알림 제목과 내용을 확인해 주세요."
  }
  if (requireFutureTrigger && args.triggerAtEpochMillis <= System.currentTimeMillis()) {
    return "REMINDER_TIME_PAST" to "알림 시간은 지금보다 나중이어야 해요."
  }
  if (args.targetAtEpochMillis < args.triggerAtEpochMillis) {
    return "REMINDER_INVALID" to "알림 대상 시간을 확인해 주세요."
  }
  return null
}

private fun validItemType(value: String): Boolean = value == "task" || value == "schedule"

private fun validIdentifier(value: String): Boolean =
  value.length in 1..128 && value.all { it.isLetterOrDigit() || it == '-' || it == '_' }

private fun stableReminderKey(itemType: String, itemId: String): String = "$itemType:$itemId"

private fun stableRequestCode(itemType: String, itemId: String): Int =
  stableReminderKey(itemType, itemId).hashCode() and Int.MAX_VALUE
