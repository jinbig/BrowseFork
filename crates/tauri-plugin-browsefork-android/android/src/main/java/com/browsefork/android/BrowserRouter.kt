package com.browsefork.android

import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.content.pm.ResolveInfo
import android.graphics.Bitmap
import android.graphics.Canvas
import android.net.Uri
import android.util.Base64
import app.tauri.plugin.JSObject
import java.io.ByteArrayOutputStream
import org.json.JSONArray

internal const val BROWSER_ICON_PIXELS = 192
internal const val BROWSER_ICON_CACHE_VERSION = 2
internal const val WEB_PROBE_HOST = "browsefork.invalid"

/** Never parse incoming intent:// strings as Android intents or execute their extras. */
internal fun linkFromIntent(intent: Intent): String? = when (intent.action) {
    Intent.ACTION_VIEW -> intent.dataString?.takeIf { it.isNotBlank() }
    Intent.ACTION_SEND -> if (intent.type == "text/plain") {
        val text = intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString()?.trim().orEmpty()
        Regex("https?://[^\\s<>]+", RegexOption.IGNORE_CASE).find(text)?.value
    } else null
    else -> null
}

/** Only inspect the incoming intent: Activity.referrer/callingPackage can describe an older launch. */
internal fun sourceFromIntent(intent: Intent): String? {
    val referrer = if (android.os.Build.VERSION.SDK_INT >= 33)
        intent.getParcelableExtra(Intent.EXTRA_REFERRER, Uri::class.java)
    else {
        @Suppress("DEPRECATION")
        intent.getParcelableExtra<Uri>(Intent.EXTRA_REFERRER)
    } ?: intent.getStringExtra(Intent.EXTRA_REFERRER_NAME)?.let(Uri::parse)
    return referrer?.takeIf { it.scheme == "android-app" }?.host
}

internal fun browserCandidates(results: List<ResolveInfo>, ownPackage: String): List<ResolveInfo> =
    results.filter {
        // A scheme match accepts arbitrary hosts. Host/path matches are app links, not browsers.
        (it.match and IntentFilter.MATCH_CATEGORY_MASK) == IntentFilter.MATCH_CATEGORY_SCHEME &&
            it.activityInfo?.exported == true &&
            it.activityInfo.enabled && it.activityInfo.applicationInfo.enabled &&
            it.activityInfo.packageName != ownPackage
    }.distinctBy { it.activityInfo.packageName }

internal class BrowserRouter(private val context: Context) {
    private val pm = context.packageManager
    // Keep the original content URI grant until its link is opened. Web URLs need no grants.
    private val contentIntents = mutableMapOf<String, Intent>()

    fun retainGrant(intent: Intent, url: String) {
        if (Uri.parse(url).scheme == "content") contentIntents[url] = Intent(intent)
    }

    fun browsers(): JSONArray {
        val candidates = listOf("http", "https").flatMap { scheme ->
            pm.queryIntentActivities(
                Intent(Intent.ACTION_VIEW, Uri.parse("$scheme://$WEB_PROBE_HOST/"))
                    .addCategory(Intent.CATEGORY_BROWSABLE),
                PackageManager.MATCH_DEFAULT_ONLY or PackageManager.MATCH_ALL
            )
        }
        val browsers = browserCandidates(candidates, context.packageName)
            .sortedBy { it.loadLabel(pm).toString().lowercase() }
        return JSONArray().apply {
            browsers.forEach { info ->
                put(JSObject().apply {
                    put("id", info.activityInfo.packageName)
                    put("name", info.loadLabel(pm).toString())
                    // Rasterize native adaptive/vector icons at a size suitable for high density displays.
                    runCatching { put("icon_data_url", icon(info)) }
                })
            }
        }
    }

    fun sourceApps(): JSONArray = JSONArray().apply {
        pm.queryIntentActivities(Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER), 0)
            .filter { it.activityInfo.packageName != context.packageName }
            .distinctBy { it.activityInfo.packageName }
            .sortedBy { it.loadLabel(pm).toString().lowercase() }
            .forEach { info ->
                put(JSObject().apply {
                    put("id", info.activityInfo.packageName)
                    put("name", info.loadLabel(pm).toString())
                })
            }
    }

    fun launchIntent(browserId: String, url: String): Intent {
        require(browserId != context.packageName) { "BrowseFork cannot open a link in itself" }
        val uri = Uri.parse(url)
        require(!uri.scheme.isNullOrBlank()) { "The link has no URI scheme" }
        val browser = browserCandidates(
            pm.queryIntentActivities(
                Intent(Intent.ACTION_VIEW, Uri.parse("https://$WEB_PROBE_HOST/"))
                    .addCategory(Intent.CATEGORY_BROWSABLE).setPackage(browserId),
                PackageManager.MATCH_DEFAULT_ONLY or PackageManager.MATCH_ALL
            ), context.packageName
        ).firstOrNull() ?: error("Selected browser is no longer available")
        // Restrict the target to the chosen browser package. An implicit intent would route back here.
        val outbound = Intent(Intent.ACTION_VIEW, uri)
            .addCategory(Intent.CATEGORY_BROWSABLE)
            .setPackage(browser.activityInfo.packageName)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        contentIntents[url]?.let { original ->
            outbound.clipData = original.clipData
            outbound.addFlags(original.flags and Intent.FLAG_GRANT_READ_URI_PERMISSION)
            original.type?.let { outbound.setDataAndType(uri, it) }
        }
        return outbound
    }

    fun opened(url: String) { contentIntents.remove(url) }

    private fun icon(info: ResolveInfo): String {
        // Reuse the rasterized icon across process restarts, but validate it
        // against the installed package and appearance before using it.
        val packageName = info.activityInfo.packageName
        val packageInfo = pm.getPackageInfo(packageName, 0)
        val configuration = context.resources.configuration
        val stamp = "$BROWSER_ICON_CACHE_VERSION:${packageInfo.lastUpdateTime}:${configuration.uiMode}:${configuration.densityDpi}:${configuration.locales.toLanguageTags()}:$BROWSER_ICON_PIXELS"
        val cache = context.getSharedPreferences("browser-icons", Context.MODE_PRIVATE)
        if (cache.getString("$packageName.stamp", null) == stamp) {
            cache.getString("$packageName.icon", null)?.let { return it }
        }
        // Android supplies its managed/work badge. Google accounts within Chrome
        // are private to Chrome and are not separate ACTION_VIEW destinations.
        val drawable = pm.getUserBadgedIcon(info.activityInfo.loadUnbadgedIcon(pm), android.os.Process.myUserHandle())
        val bitmap = Bitmap.createBitmap(BROWSER_ICON_PIXELS, BROWSER_ICON_PIXELS, Bitmap.Config.ARGB_8888)
        drawable.setBounds(0, 0, bitmap.width, bitmap.height)
        drawable.draw(Canvas(bitmap))
        val bytes = ByteArrayOutputStream()
        bitmap.compress(Bitmap.CompressFormat.PNG, 100, bytes)
        bitmap.recycle()
        val result = "data:image/png;base64," + Base64.encodeToString(bytes.toByteArray(), Base64.NO_WRAP)
        cache.edit().putString("$packageName.stamp", stamp).putString("$packageName.icon", result).apply()
        return result
    }
}
