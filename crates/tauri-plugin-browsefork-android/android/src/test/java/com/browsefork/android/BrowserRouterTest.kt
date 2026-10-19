package com.browsefork.android

import android.content.ClipData
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ActivityInfo
import android.content.pm.ApplicationInfo
import android.content.pm.ResolveInfo
import android.content.pm.PackageInfo
import android.net.Uri
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class BrowserRouterTest {
    private val context = RuntimeEnvironment.getApplication()

    private fun browser(id: String, handlesAll: Boolean = true): ResolveInfo = ResolveInfo().apply {
        match = if (handlesAll) IntentFilter.MATCH_CATEGORY_SCHEME else IntentFilter.MATCH_CATEGORY_HOST
        isDefault = true
        activityInfo = ActivityInfo().apply {
            packageName = id
            name = "$id.BrowserActivity"
            enabled = true
            exported = true
            applicationInfo = ApplicationInfo().apply { packageName = id; enabled = true }
        }
    }

    private fun register(id: String) {
        shadowOf(context.packageManager).addResolveInfoForIntent(
            Intent(Intent.ACTION_VIEW, Uri.parse("https://$WEB_PROBE_HOST/"))
                .addCategory(Intent.CATEGORY_BROWSABLE).setPackage(id), browser(id)
        )
    }

    @Test fun viewLinksKeepTheirFullEncodedUrl() {
        val url = "https://example.com/login?a=a%2Bb&state=secret#fragment"
        assertEquals(url, linkFromIntent(Intent(Intent.ACTION_VIEW, Uri.parse(url))))
        assertEquals("custom://auth/callback", linkFromIntent(Intent(Intent.ACTION_VIEW, Uri.parse("custom://auth/callback"))))
    }

    @Test fun sharesAcceptUrlsAndRejectNonUrlText() {
        val intent = Intent(Intent.ACTION_SEND).setType("text/plain")
        intent.putExtra(Intent.EXTRA_TEXT, "Some title\nhttps://example.com/path?a=1&b=2")
        assertEquals("https://example.com/path?a=1&b=2", linkFromIntent(intent))
        intent.putExtra(Intent.EXTRA_TEXT, "Just a title")
        assertNull(linkFromIntent(intent))
        assertNull(linkFromIntent(Intent(Intent.ACTION_MAIN)))
    }

    @Test fun discoveryExcludesSelfDeepLinkAppsDisabledAppsAndDuplicatePackages() {
        val disabled = browser("disabled").apply { activityInfo.enabled = false }
        val notExported = browser("private").apply { activityInfo.exported = false }
        val list = listOf(browser("self"), browser("deep.link", false), disabled,
            notExported, browser("chrome"), browser("chrome"), browser("firefox"))
        assertEquals(listOf("chrome", "firefox"), browserCandidates(list, "self").map { it.activityInfo.packageName })
    }

    @Test fun launchExplicitlyTargetsChosenBrowserWithoutInterpretingIntentUrls() {
        register("test.browser")
        val url = "intent://example/#Intent;package=unwanted.app;end"
        val outbound = BrowserRouter(context).launchIntent("test.browser", url)
        assertEquals("test.browser", outbound.`package`)
        assertEquals(Intent.ACTION_VIEW, outbound.action)
        assertEquals(url, outbound.dataString)
        assertNull(outbound.component)
        assertTrue(outbound.flags and Intent.FLAG_ACTIVITY_NEW_TASK != 0)
    }

    @Test fun contentGrantsAreForwardedWithoutWritePermissionsOrSenderExtras() {
        register("test.browser")
        val url = "content://test.provider/auth.html"
        val incoming = Intent(Intent.ACTION_VIEW, Uri.parse(url))
            .setType("text/html").setDataAndType(Uri.parse(url), "text/html")
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
            .putExtra("untrusted", "not forwarded")
        incoming.clipData = ClipData.newRawUri("HTML", Uri.parse(url))
        val router = BrowserRouter(context)
        router.retainGrant(incoming, url)
        val outbound = router.launchIntent("test.browser", url)
        assertEquals("text/html", outbound.type)
        assertNotNull(outbound.clipData)
        assertFalse(outbound.hasExtra("untrusted"))
        assertTrue(outbound.flags and Intent.FLAG_GRANT_READ_URI_PERMISSION != 0)
        assertEquals(0, outbound.flags and Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
    }

    @Test fun sourceBelongsToThisIntentOnlyAndRequiresAnAppReferrer() {
        val known = Intent(Intent.ACTION_VIEW, Uri.parse("https://example.com"))
            .putExtra(Intent.EXTRA_REFERRER, Uri.parse("android-app://com.test.sender"))
        assertEquals("com.test.sender", sourceFromIntent(known))
        assertNull(sourceFromIntent(Intent(Intent.ACTION_VIEW, Uri.parse("https://example.com"))))
        known.putExtra(Intent.EXTRA_REFERRER, Uri.parse("https://com.test.sender"))
        assertNull(sourceFromIntent(known))
    }

    @Test fun missingBrowserAndSelfLaunchFailRatherThanLooping() {
        val router = BrowserRouter(context)
        assertThrows(IllegalArgumentException::class.java) { router.launchIntent(context.packageName, "https://example.com/") }
        assertThrows(IllegalStateException::class.java) { router.launchIntent("missing.browser", "https://example.com/") }
    }

    @Test fun iconsSurviveProcessRestartsButPackageUpdatesInvalidateThem() {
        val id = "cached.browser"
        val installed = PackageInfo().apply {
            packageName = id
            applicationInfo = ApplicationInfo().apply { packageName = id; enabled = true }
            lastUpdateTime = 10
        }
        val pm = shadowOf(context.packageManager)
        pm.installPackage(installed)
        pm.addResolveInfoForIntent(Intent(Intent.ACTION_VIEW, Uri.parse("https://$WEB_PROBE_HOST/"))
            .addCategory(Intent.CATEGORY_BROWSABLE), browser(id))
        val first = BrowserRouter(context).browsers().getJSONObject(0).getString("icon_data_url")
        assertTrue(first.startsWith("data:image/png;base64,"))
        // A fresh router simulates a restarted process; unchanged packages must
        // use the persisted icon rather than load and rasterize it again.
        val cache = context.getSharedPreferences("browser-icons", android.content.Context.MODE_PRIVATE)
        cache.edit().putString("$id.icon", "cached-icon-sentinel").commit()
        assertEquals("cached-icon-sentinel", BrowserRouter(context).browsers().getJSONObject(0).getString("icon_data_url"))
        installed.lastUpdateTime = 20
        pm.installPackage(installed)
        assertNotEquals("cached-icon-sentinel", BrowserRouter(context).browsers().getJSONObject(0).getString("icon_data_url"))
    }
}
