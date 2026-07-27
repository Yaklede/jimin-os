package io.jimin.devicesignals

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.CallLog
import android.provider.Settings
import androidx.core.content.ContextCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

private const val preferencesName = "io.jimin.os.device_signals"
private const val callLogPermissionRequested = "call_log_permission_requested"
private const val maximumCalls = 200
private const val retentionMillis = 90L * 24L * 60L * 60L * 1_000L

@InvokeArg
class MissedCallsArgs {
  var sinceEpochMillis: Long = 0
  var limit: Int = 50
}

@TauriPlugin(
  permissions = [Permission(strings = [Manifest.permission.READ_CALL_LOG], alias = "callLog")],
)
class DeviceSignalsPlugin(private val activity: Activity) : Plugin(activity) {
  @Command
  fun permissionStatus(invoke: Invoke) {
    invoke.resolve(permissionStatusResult())
  }

  @Command
  fun requestPermission(invoke: Invoke) {
    if (hasCallLogPermission()) {
      invoke.resolve(permissionStatusResult())
      return
    }
    activity
      .getSharedPreferences(preferencesName, Context.MODE_PRIVATE)
      .edit()
      .putBoolean(callLogPermissionRequested, true)
      .apply()
    requestPermissionForAlias("callLog", invoke, "onCallLogPermission")
  }

  @PermissionCallback
  fun onCallLogPermission(invoke: Invoke) {
    invoke.resolve(permissionStatusResult())
  }

  @Command
  fun openSettings(invoke: Invoke) {
    try {
      activity.startActivity(
        Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
          data = Uri.parse("package:${activity.packageName}")
        },
      )
      invoke.resolve()
    } catch (error: Exception) {
      invoke.reject(
        "휴대폰 설정을 열지 못했어요. 설정에서 Jimin OS를 찾아 주세요.",
        "CALL_LOG_SETTINGS_FAILED",
        error,
      )
    }
  }

  @Command
  fun missedCalls(invoke: Invoke) {
    if (!hasCallLogPermission()) {
      invoke.reject(
        "부재중 전화를 확인하려면 통화 기록 권한을 허용해 주세요.",
        "CALL_LOG_PERMISSION_REQUIRED",
      )
      return
    }
    try {
      val args = invoke.parseArgs(MissedCallsArgs::class.java)
      val now = System.currentTimeMillis()
      if (args.limit !in 1..maximumCalls ||
        args.sinceEpochMillis <= 0 ||
        args.sinceEpochMillis > now ||
        args.sinceEpochMillis < now - retentionMillis
      ) {
        invoke.reject("확인할 통화 기록 범위를 다시 선택해 주세요.", "CALL_LOG_RANGE_INVALID")
        return
      }
      val calls = JSArray()
      val projection =
        arrayOf(
          CallLog.Calls._ID,
          CallLog.Calls.NUMBER,
          CallLog.Calls.CACHED_NAME,
          CallLog.Calls.DATE,
        )
      val selection =
        "${CallLog.Calls.TYPE} = ? AND ${CallLog.Calls.DATE} >= ?"
      val selectionArgs =
        arrayOf(
          CallLog.Calls.MISSED_TYPE.toString(),
          args.sinceEpochMillis.toString(),
        )
      val sortOrder = "${CallLog.Calls.DATE} DESC LIMIT ${args.limit}"
      activity.contentResolver
        .query(
          CallLog.Calls.CONTENT_URI,
          projection,
          selection,
          selectionArgs,
          sortOrder,
        )
        ?.use { cursor ->
          val idIndex = cursor.getColumnIndexOrThrow(CallLog.Calls._ID)
          val numberIndex = cursor.getColumnIndexOrThrow(CallLog.Calls.NUMBER)
          val nameIndex = cursor.getColumnIndexOrThrow(CallLog.Calls.CACHED_NAME)
          val dateIndex = cursor.getColumnIndexOrThrow(CallLog.Calls.DATE)
          while (cursor.moveToNext()) {
            calls.put(
              JSObject().apply {
                put("sourceId", cursor.getLong(idIndex).toString())
                put("occurredAtEpochMillis", cursor.getLong(dateIndex))
                cursor.getString(nameIndex)?.trim()?.takeIf(String::isNotEmpty)?.let {
                  put("callerName", it)
                }
                cursor.getString(numberIndex)?.trim()?.takeIf(String::isNotEmpty)?.let {
                  put("phoneNumber", it)
                }
              },
            )
          }
        }
      invoke.resolve(
        JSObject().apply {
          put("calls", calls)
          put("platformVersion", Build.VERSION.RELEASE ?: Build.VERSION.SDK_INT.toString())
        },
      )
    } catch (error: SecurityException) {
      invoke.reject(
        "부재중 전화를 확인하려면 통화 기록 권한을 허용해 주세요.",
        "CALL_LOG_PERMISSION_REQUIRED",
        error,
      )
    } catch (error: Exception) {
      invoke.reject(
        "부재중 전화 기록을 불러오지 못했어요. 잠시 후 다시 시도해 주세요.",
        "CALL_LOG_READ_FAILED",
        error,
      )
    }
  }

  private fun hasCallLogPermission(): Boolean =
    ContextCompat.checkSelfPermission(activity, Manifest.permission.READ_CALL_LOG) ==
      PackageManager.PERMISSION_GRANTED

  private fun permissionStatusResult(): JSObject {
    val requested =
      activity
        .getSharedPreferences(preferencesName, Context.MODE_PRIVATE)
        .getBoolean(callLogPermissionRequested, false)
    val status =
      when {
        hasCallLogPermission() -> "granted"
        requested -> "denied"
        else -> "not_determined"
      }
    return JSObject().apply {
      put("status", status)
      put("canRequest", status == "not_determined")
      put("platformVersion", Build.VERSION.RELEASE ?: Build.VERSION.SDK_INT.toString())
    }
  }
}
