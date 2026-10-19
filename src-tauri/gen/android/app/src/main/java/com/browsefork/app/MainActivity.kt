package com.browsefork.app

import android.os.Bundle
import android.os.Build
import android.app.Activity
import android.graphics.Color
import android.view.View
import android.view.WindowManager
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onWebViewCreate(webView: WebView) {
    // Wry calls this before attaching the WebView to the Activity. Plugin.load
    // is later: hiding there alone leaves its initial white surface exposed.
    webView.setBackgroundColor(Color.TRANSPARENT)
    webView.visibility = View.INVISIBLE
    super.onWebViewCreate(webView)
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    if (Build.VERSION.SDK_INT >= 34) {
      overrideActivityTransition(Activity.OVERRIDE_TRANSITION_OPEN, 0, 0)
      overrideActivityTransition(Activity.OVERRIDE_TRANSITION_CLOSE, 0, 0)
    }
    // Android 15+ enforces edge-to-edge windows. Keep the shared UI out of the
    // status/navigation bars, display cutouts, and the on-screen keyboard.
    val content = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val fullScreen = window.attributes.width == WindowManager.LayoutParams.MATCH_PARENT
      val types = if (fullScreen) WindowInsetsCompat.Type.systemBars() or
        WindowInsetsCompat.Type.displayCutout() or WindowInsetsCompat.Type.ime()
        else WindowInsetsCompat.Type.ime()
      val safe = insets.getInsets(types)
      view.setPadding(safe.left, safe.top, safe.right, safe.bottom)
      WindowInsetsCompat.CONSUMED
    }
    ViewCompat.requestApplyInsets(content)
  }

  override fun onResume() {
    super.onResume()
    // A chooser is a popup over the sender, not an activity sliding/scaling in.
    @Suppress("DEPRECATION")
    overridePendingTransition(0, 0)
  }
}
