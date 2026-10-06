package com.browsefork.android

import android.app.Activity
import android.app.role.RoleManager
import android.content.ActivityNotFoundException
import android.content.Intent
import android.os.Build
import android.provider.Settings
import android.webkit.WebView
import android.view.View
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.app.AppCompatActivity
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

internal const val DEFAULT_BROWSER_REQUEST_CODE = 101

@InvokeArg
internal class StartArgs { lateinit var channel: Channel }

@InvokeArg
internal class OpenUrlArgs {
    lateinit var browserId: String
    lateinit var url: String
}

@TauriPlugin
class BrowseForkPlugin(private val activity: Activity) : Plugin(activity) {
    private val router = BrowserRouter(activity)
    private var channel: Channel? = null
    private val waiting = mutableListOf<JSObject>()
    private var content: WebView? = null
    private var paused = false

    override fun load(webView: WebView) {
        content = webView
        webView.visibility = View.INVISIBLE
        capture(activity.intent, initial = true)
        (activity as? AppCompatActivity)?.onBackPressedDispatcher?.addCallback(
            activity, object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    if (channel == null) activity.moveTaskToBack(true)
                    else send(JSObject().put("kind", "back"))
                }
            }
        )
    }

    override fun onNewIntent(intent: Intent) {
        // Replace the old intent before inspecting its referrer; never reuse a previous sender.
        activity.intent = intent
        content?.visibility = View.INVISIBLE
        sendState()
        capture(intent)
    }

    override fun onPause(activity: AppCompatActivity) { paused = true }

    override fun onResume(activity: AppCompatActivity) {
        sendState()
        if (paused && channel != null) send(JSObject().put("kind", "resume"))
        paused = false
    }

    @Command
    fun start(invoke: Invoke) {
        channel = invoke.parseArgs(StartArgs::class.java).channel
        sendState()
        val initial = waiting.toList()
        waiting.clear()
        initial.forEach { channel?.send(it) }
        send(JSObject().put("kind", "ready"))
        invoke.resolve()
    }

    @Command
    fun present(invoke: Invoke) {
        content?.visibility = View.VISIBLE
        invoke.resolve()
    }

    @Command
    fun openUrl(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(OpenUrlArgs::class.java)
            activity.startActivity(router.launchIntent(args.browserId, args.url))
            router.opened(args.url)
            invoke.resolve()
        } catch (_: ActivityNotFoundException) {
            invoke.reject("This browser cannot open the link. Choose another browser.")
        } catch (error: Exception) {
            invoke.reject(error.message ?: "Could not open this link")
        }
    }

    @Command
    fun dismiss(invoke: Invoke) {
        activity.moveTaskToBack(true)
        invoke.resolve()
    }

    @Command
    fun requestDefaultBrowser(invoke: Invoke) {
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                val roles = activity.getSystemService(RoleManager::class.java)
                if (roles.isRoleAvailable(RoleManager.ROLE_BROWSER) &&
                    !roles.isRoleHeld(RoleManager.ROLE_BROWSER)) {
                    // RoleManager identifies the requester from the result recipient.
                    @Suppress("DEPRECATION")
                    activity.startActivityForResult(
                        roles.createRequestRoleIntent(RoleManager.ROLE_BROWSER),
                        DEFAULT_BROWSER_REQUEST_CODE
                    )
                }
            } else {
                val action = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N)
                    Settings.ACTION_MANAGE_DEFAULT_APPS_SETTINGS else Settings.ACTION_SETTINGS
                activity.startActivity(Intent(action))
            }
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject(error.message ?: "Open Settings and select BrowseFork as the default browser")
        }
    }

    private fun capture(intent: Intent, initial: Boolean = false) {
        val url = linkFromIntent(intent)
        if (url != null) {
            router.retainGrant(intent, url)
            val event = JSObject().put("kind", "link").put("url", url)
            // Best effort only: most ACTION_VIEW senders expose no caller. A referrer can be
            // supplied by the sending app and is a routing preference, never a security identity.
            val source = sourceFromIntent(intent) ?: if (initial) activity.callingPackage else null
            if (source != null && source != activity.packageName) event.put("source_app_id", source)
            send(event)
        } else if (intent.action == Intent.ACTION_MAIN) {
            send(JSObject().put("kind", "launcher"))
        } else if (intent.action == Intent.ACTION_SEND || intent.action == Intent.ACTION_VIEW) {
            send(JSObject().put("kind", "error").put("message", "The request contains no usable link."))
        }
    }

    private fun sendState() {
        if (channel == null) return
        try {
            val event = JSObject().put("kind", "state")
                .put("browsers", router.browsers()).put("source_apps", router.sourceApps())
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                event.put("is_default_browser", activity.getSystemService(RoleManager::class.java)
                    .isRoleHeld(RoleManager.ROLE_BROWSER))
            }
            send(event)
        } catch (error: Exception) {
            send(JSObject().put("kind", "error").put("message", error.message ?: "Browser discovery failed"))
        }
    }

    private fun send(event: JSObject) {
        channel?.send(event) ?: waiting.add(event)
    }
}
