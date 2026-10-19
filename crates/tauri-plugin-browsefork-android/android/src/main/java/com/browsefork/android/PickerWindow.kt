package com.browsefork.android

import android.app.Activity
import android.content.pm.ApplicationInfo
import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.os.SystemClock
import android.util.Log
import android.view.View
import android.view.WindowManager
import android.webkit.WebView

internal const val PICKER_DIM_AMOUNT = 0.22f

/** The shared UI sizes the rounded popup inside a stable transparent host.
 * Native bounds never change between chooser and Settings, including first launch.
 */
internal class PickerWindow(private val activity: Activity, private val webView: WebView) {
    private var generation = 0L
    private var revealed = false
    private var latestRevision = -1L
    private var minimumRevision = 0L

    init {
        activity.window.setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))
        activity.window.setLayout(WindowManager.LayoutParams.MATCH_PARENT, WindowManager.LayoutParams.MATCH_PARENT)
        prepare()
    }

    fun prepare() {
        generation++
        minimumRevision = maxOf(minimumRevision, latestRevision + 1)
        revealed = false
        webView.visibility = View.INVISIBLE
        activity.window.clearFlags(WindowManager.LayoutParams.FLAG_DIM_BEHIND)
        activity.window.decorView.invalidate()
    }

    fun present(settings: Boolean, revision: Long = 0): Boolean {
        if (revision < maxOf(minimumRevision, latestRevision)) return false
        latestRevision = maxOf(latestRevision, revision)
        val request = ++generation
        // JS has already decoded the assets and laid out the final popup at
        // this viewport width. Wait for that actual frame without any resize IPC.
        webView.postVisualStateCallback(request, object : WebView.VisualStateCallback() {
            override fun onComplete(requestId: Long) {
                if (requestId != generation) return
                val window = activity.window
                if (settings) window.clearFlags(WindowManager.LayoutParams.FLAG_DIM_BEHIND)
                else window.addFlags(WindowManager.LayoutParams.FLAG_DIM_BEHIND)
                window.attributes = window.attributes.apply { dimAmount = if (settings) 0f else PICKER_DIM_AMOUNT }
                webView.visibility = View.VISIBLE
                if (!revealed && activity.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0) {
                    Log.d("BrowseForkPresentation", "reveal t=${SystemClock.elapsedRealtime()} revision=$revision")
                }
                revealed = true
            }
        })
        return true
    }
}
