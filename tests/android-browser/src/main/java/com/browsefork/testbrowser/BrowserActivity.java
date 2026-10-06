package com.browsefork.testbrowser;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.widget.TextView;
import java.nio.charset.StandardCharsets;

/** Test fixture: captures links without making a network request. */
public class BrowserActivity extends Activity {
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        capture(getIntent());
    }

    @Override public void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        capture(intent);
    }

    private void capture(Intent intent) {
        TextView view = new TextView(this);
        view.setText("BrowseFork test browser: link received");
        view.setTextSize(20);
        view.setPadding(24, 64, 24, 24);
        setContentView(view);
        try (java.io.FileOutputStream output = openFileOutput("last-url.txt", MODE_PRIVATE)) {
            output.write(String.valueOf(intent.getDataString()).getBytes(StandardCharsets.UTF_8));
        } catch (java.io.IOException error) { throw new RuntimeException(error); }
    }
}
