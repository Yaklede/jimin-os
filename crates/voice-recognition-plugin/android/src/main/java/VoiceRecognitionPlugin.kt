package io.jimin.voicerecognition

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Bundle
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import android.util.Log
import androidx.activity.result.ActivityResult
import androidx.core.content.ContextCompat
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@TauriPlugin(
  permissions = [Permission(strings = [Manifest.permission.RECORD_AUDIO], alias = "microphone")],
)
class VoiceRecognitionPlugin(private val activity: Activity) : Plugin(activity) {
  private companion object {
    const val logTag = "JiminVoice"
  }

  private var recognition: SpeechRecognizer? = null
  private var activeInvoke: Invoke? = null

  @Command
  fun start(invoke: Invoke) {
    if (activeInvoke != null) {
      invoke.reject("음성 입력이 이미 진행 중이에요.", "VOICE_BUSY")
      return
    }

    if (ContextCompat.checkSelfPermission(activity, Manifest.permission.RECORD_AUDIO) !=
      PackageManager.PERMISSION_GRANTED
    ) {
      requestPermissionForAlias("microphone", invoke, "onMicrophonePermission")
      return
    }
    startRecognition(invoke)
  }

  @PermissionCallback
  fun onMicrophonePermission(invoke: Invoke) {
    startRecognition(invoke)
  }

  @Command
  fun stop(invoke: Invoke) {
    if (activeInvoke == null) {
      invoke.resolve()
      return
    }

    try {
      recognition?.stopListening()
      invoke.resolve()
    } catch (error: Exception) {
      invoke.reject("음성 입력을 마치지 못했어요.", "VOICE_STOP_FAILED", error)
    }
  }

  @Command
  fun cancel(invoke: Invoke) {
    val active = takeActiveInvoke()
    active?.reject("음성 입력을 취소했어요.", "VOICE_CANCELED")
    invoke.resolve()
  }

  override fun onDestroy(activity: androidx.appcompat.app.AppCompatActivity) {
    takeActiveInvoke()
  }

  private fun startRecognition(invoke: Invoke) {
    if (activeInvoke != null) return
    activeInvoke = invoke

    if (!SpeechRecognizer.isRecognitionAvailable(activity)) {
      Log.i(logTag, "Direct speech recognizer is unavailable; using the system activity.")
      startRecognitionActivity(invoke)
      return
    }

    try {
      val recognizer = SpeechRecognizer.createSpeechRecognizer(activity)
      recognition = recognizer
      recognizer.setRecognitionListener(
        object : RecognitionListener {
          override fun onReadyForSpeech(params: Bundle?) = Unit

          override fun onBeginningOfSpeech() = Unit

          override fun onRmsChanged(rmsdB: Float) = Unit

          override fun onBufferReceived(buffer: ByteArray?) = Unit

          override fun onEndOfSpeech() = Unit

          override fun onError(error: Int) {
            Log.w(logTag, "Direct speech recognizer returned error code $error.")
            when (error) {
              SpeechRecognizer.ERROR_NO_MATCH,
              SpeechRecognizer.ERROR_SPEECH_TIMEOUT,
              -> failActive("VOICE_NO_SPEECH")
              SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS ->
                failActive("VOICE_PERMISSION")
              else -> {
                recognition?.destroy()
                recognition = null
                startRecognitionActivity(invoke)
              }
            }
          }

          override fun onResults(results: Bundle?) {
            val transcript =
              results
                ?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
                ?.firstOrNull()
                ?.trim()
            if (transcript.isNullOrEmpty()) {
              failActive("VOICE_NO_SPEECH")
              return
            }
            resolveTranscript(transcript)
          }

          override fun onPartialResults(partialResults: Bundle?) = Unit

          override fun onEvent(eventType: Int, params: Bundle?) = Unit
        },
      )
      recognizer.startListening(
        Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
          putExtra(
            RecognizerIntent.EXTRA_LANGUAGE_MODEL,
            RecognizerIntent.LANGUAGE_MODEL_FREE_FORM,
          )
          putExtra(RecognizerIntent.EXTRA_LANGUAGE, "ko-KR")
          putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true)
          putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 1)
        },
      )
    } catch (error: SecurityException) {
      failActive("VOICE_PERMISSION")
    } catch (error: Exception) {
      Log.w(logTag, "Direct speech recognizer could not start; using the system activity.", error)
      recognition?.destroy()
      recognition = null
      startRecognitionActivity(invoke)
    }
  }

  private fun startRecognitionActivity(invoke: Invoke) {
    if (activeInvoke !== invoke) return

    try {
      val intent =
        Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
          putExtra(
            RecognizerIntent.EXTRA_LANGUAGE_MODEL,
            RecognizerIntent.LANGUAGE_MODEL_FREE_FORM,
          )
          putExtra(RecognizerIntent.EXTRA_LANGUAGE, "ko-KR")
          putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 1)
        }
      if (intent.resolveActivity(activity.packageManager) == null) {
        Log.w(logTag, "No system speech activity is available.")
        failActive("VOICE_UNAVAILABLE")
        return
      }
      startActivityForResult(invoke, intent, "onRecognitionActivityResult")
    } catch (error: Exception) {
      Log.w(logTag, "The system speech activity could not start.", error)
      failActive("VOICE_UNAVAILABLE")
    }
  }

  @ActivityCallback
  fun onRecognitionActivityResult(invoke: Invoke, result: ActivityResult) {
    if (activeInvoke !== invoke) return

    val transcript =
      result.data
        ?.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS)
        ?.firstOrNull()
        ?.trim()
    if (result.resultCode == Activity.RESULT_OK && !transcript.isNullOrEmpty()) {
      resolveTranscript(transcript)
    } else {
      failActive("VOICE_NO_SPEECH")
    }
  }

  private fun resolveTranscript(transcript: String) {
    val response = JSObject()
    response.put("transcript", transcript)
    val active = takeActiveInvoke()
    active?.resolve(response)
  }

  private fun failActive(code: String) {
    val active = takeActiveInvoke() ?: return
    active.reject(messageFor(code), code)
  }

  private fun takeActiveInvoke(): Invoke? {
    val active = activeInvoke
    activeInvoke = null
    recognition?.destroy()
    recognition = null
    return active
  }

  private fun messageFor(code: String): String =
    when (code) {
      "VOICE_NO_SPEECH" -> "말한 내용을 듣지 못했어요."
      "VOICE_PERMISSION" -> "마이크 권한이 필요해요."
      "VOICE_UNAVAILABLE" -> "이 기기에서 음성 인식을 사용할 수 없어요."
      else -> "음성 입력을 완료하지 못했어요."
    }
}
