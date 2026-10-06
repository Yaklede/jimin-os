package io.jimin.os

import android.content.res.Configuration
import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.webkit.WebView
import android.webkit.JavascriptInterface
import androidx.activity.OnBackPressedCallback
import androidx.core.content.ContextCompat
import androidx.core.view.WindowCompat
import io.crates.keyring.Keyring

class MainActivity : TauriActivity() {
  private var appWebView: WebView? = null
  private var appearanceMode: String? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    Keyring.initializeNdkContext(applicationContext)
    appearanceMode = getPreferences(MODE_PRIVATE).getString("appearanceMode", null)
    super.onCreate(savedInstanceState)
    onBackPressedDispatcher.addCallback(
      this,
      object : OnBackPressedCallback(true) {
        override fun handleOnBackPressed() {
          handleAppBack()
        }
      },
    )
    WindowCompat.setDecorFitsSystemWindows(window, true)
    updateSystemBarAppearance()
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    appWebView = webView
    // This bridge accepts only a visual mode; it exposes no device data or permissions.
    webView.addJavascriptInterface(AppearanceBridge(), "JiminAppearance")
    webView.settings.setSupportZoom(false)
    webView.settings.builtInZoomControls = false
    webView.settings.displayZoomControls = false
  }

  override fun onConfigurationChanged(newConfig: Configuration) {
    super.onConfigurationChanged(newConfig)
    updateSystemBarAppearance()
  }

  private fun updateSystemBarAppearance() {
    val darkMode = when (appearanceMode) {
      "dark" -> true
      "light" -> false
      else -> resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK ==
        Configuration.UI_MODE_NIGHT_YES
    }
    val systemBarColor = if (appearanceMode == null) {
      ContextCompat.getColor(this, R.color.system_bar)
    } else {
      Color.parseColor(if (darkMode) "#222222" else "#f4f5f7")
    }
    window.statusBarColor = systemBarColor
    window.navigationBarColor = systemBarColor
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
      window.navigationBarDividerColor = systemBarColor
    }
    WindowCompat.getInsetsController(window, window.decorView).apply {
      isAppearanceLightStatusBars = !darkMode
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O_MR1) {
        isAppearanceLightNavigationBars = !darkMode
      }
    }
  }

  private inner class AppearanceBridge {
    @JavascriptInterface
    fun setMode(mode: String) {
      if (mode != "dark" && mode != "light") return
      runOnUiThread {
        appearanceMode = mode
        getPreferences(MODE_PRIVATE).edit().putString("appearanceMode", mode).apply()
        updateSystemBarAppearance()
      }
    }
  }

  private fun handleAppBack() {
    val webView = appWebView
    if (webView == null) {
      moveTaskToBack(true)
      return
    }
    webView.evaluateJavascript(
      """
      (() => {
        const handler = window.__JIMIN_OS_ANDROID_BACK__;
        return typeof handler === "function" ? handler() === true : false;
      })()
      """.trimIndent(),
    ) { handled ->
      if (handled != "true") {
        moveTaskToBack(true)
      }
    }
  }
}
