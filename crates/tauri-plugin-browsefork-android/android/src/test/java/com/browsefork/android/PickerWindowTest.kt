package com.browsefork.android

import android.app.Activity
import android.view.View
import android.view.WindowManager
import android.webkit.WebView
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class PickerWindowTest {
    @Test fun chooserAndSettingsKeepTheSameNativeBoundsAndWaitForTheirPreparedFrame() {
        val controller = Robolectric.buildActivity(Activity::class.java).setup()
        val activity = controller.get()
        val themeAnimations = activity.window.attributes.windowAnimations
        val webView = WebView(activity)
        activity.setContentView(webView)
        val presenter = PickerWindow(activity, webView)
        for (settings in listOf(false, true, false)) {
            presenter.present(settings)
            assertEquals(WindowManager.LayoutParams.MATCH_PARENT, activity.window.attributes.width)
            assertEquals(WindowManager.LayoutParams.MATCH_PARENT, activity.window.attributes.height)
        }
        presenter.prepare()
        assertEquals(View.INVISIBLE, webView.visibility)
        assertEquals(1f, activity.window.attributes.alpha)
        assertEquals(0, activity.window.attributes.flags and WindowManager.LayoutParams.FLAG_DIM_BEHIND)
        assertEquals(themeAnimations, activity.window.attributes.windowAnimations)
        assertFalse("A dismissed/newer request must not reveal an older frame", presenter.present(false, 0))
        assertTrue(presenter.present(false, 1))
        controller.pause().stop().destroy()
    }
}
