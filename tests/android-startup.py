"""Measure real cold/warm frames on a disposable emulator (requires Pillow).

Use a read-only emulator clone on port 5556. This resets BrowseFork in that clone.
Never run against the interactive emulator or a phone. Timings include ADB and PNG
capture overhead; they are upper bounds, not precise input-to-photon latency.
"""
import io
import json
import os
import re
import subprocess
import threading
import time
from pathlib import Path

from PIL import Image, ImageChops

ROOT = Path(__file__).resolve().parent.parent
SERIAL = os.environ.get("BROWSEFORK_ANDROID_SERIAL", "emulator-5556")
CAPTURE_SECONDS = 3
RECORD_SECONDS = 4
RECORD_LEAD_SECONDS = 0.5
COMMAND_TIMEOUT_SECONDS = 15
CHOOSER_BACKGROUND = (28, 34, 41)  # ui/styles.css: #1c2229
WHITE_CHANNEL_MINIMUM = 235
MAX_WHITE_SCREEN_FRACTION = 0.5
OUTPUT = ROOT / "target/android/startup"


def adb(*args):
    return subprocess.run([str(ROOT / "scripts/android.sh"), "adb", "-s", SERIAL, *args],
                          cwd=ROOT, capture_output=True, timeout=COMMAND_TIMEOUT_SECONDS, check=True).stdout


def measure(name, stopped):
    if stopped:
        adb("shell", "am", "force-stop", "com.browsefork.app")
    adb("shell", "am", "start", "-W", "-n", "com.browsefork.testbrowser/.BrowserActivity")
    # am start -W waits for Activity launch, not the sender's animation. A real
    # Slack click starts from a steady source app; do not merge two transitions.
    previous = None
    stable = 0
    deadline = time.monotonic() + 5
    while stable < 2 and time.monotonic() < deadline:
        frame = adb("exec-out", "screencap", "-p")
        stable = stable + 1 if frame == previous else 0
        previous = frame
    assert stable == 2, "Source app did not settle before the launch measurement"
    frames = []
    errors = []
    remote_video = f"/sdcard/browsefork-startup-{name}.mp4"
    recorder = subprocess.Popen([str(ROOT / "scripts/android.sh"), "adb", "-s", SERIAL,
                                 "shell", "screenrecord", "--time-limit", str(RECORD_SECONDS), remote_video],
                                cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    time.sleep(RECORD_LEAD_SECONDS)
    start = time.monotonic()

    def launch():
        try:
            adb("shell", "am", "start", "-n", "com.browsefork.app/.MainActivity", "-a",
                "android.intent.action.VIEW", "-d", f"https://startup-measure.invalid/{name}")
        except Exception as error:
            errors.append(str(error))

    worker = threading.Thread(target=launch)
    worker.start()
    while time.monotonic() - start < CAPTURE_SECONDS:
        png = adb("exec-out", "screencap", "-p")
        image = Image.open(io.BytesIO(png)).convert("RGB")
        difference = ImageChops.difference(image, Image.new("RGB", image.size, CHOOSER_BACKGROUND)).convert("L")
        bounds = difference.point(lambda value: 255 if value < 2 else 0).getbbox()
        if bounds and bounds[2] - bounds[0] > 200 and bounds[3] - bounds[1] > 150:
            if not frames:
                (OUTPUT / f"{name}-first.png").write_bytes(png)
            frames.append({"ms": round((time.monotonic() - start) * 1000), "bounds": bounds})
    worker.join()
    assert not errors, errors
    _, recording_error = recorder.communicate(timeout=COMMAND_TIMEOUT_SECONDS)
    assert recorder.returncode == 0, recording_error.decode()
    video = OUTPUT / f"{name}.mp4"
    adb("pull", remote_video, str(video))
    adb("shell", "rm", remote_video)
    decoded = OUTPUT / f"{name}-frames"
    decoded.mkdir(exist_ok=True)
    subprocess.run([str(OUTPUT / "decode-frames"), str(video), str(decoded)], check=True)
    video_frames = json.loads((decoded / "frames.json").read_text())
    assert video_frames, f"{name}: no display frames recorded"
    white_flashes = []
    for frame in video_frames:
        image = Image.open(decoded / frame["file"]).convert("RGB")
        white = sum(min(pixel) >= WHITE_CHANNEL_MINIMUM for pixel in image.getdata())
        fraction = white / (image.width * image.height)
        if fraction > MAX_WHITE_SCREEN_FRACTION:
            white_flashes.append({**frame, "white_fraction": round(fraction, 3)})
    assert len(frames) >= 2, f"{name}: chooser was not visibly rendered"
    geometries = list(dict.fromkeys(tuple(frame["bounds"]) for frame in frames))
    sizes = list(dict.fromkeys((rect[2] - rect[0], rect[3] - rect[1]) for rect in geometries))
    result = {"case": name, "first_ms": frames[0]["ms"], "visible_bounds": geometries, "visible_sizes": sizes,
              "recorded_frames": len(video_frames), "white_flashes": white_flashes, "frames": frames}
    (OUTPUT / f"{name}.json").write_text(json.dumps(result, indent=2))
    print(json.dumps({key: value for key, value in result.items() if key != "frames"}), flush=True)
    assert not white_flashes, f"{name}: white full-screen startup frames: {white_flashes}"
    # Android may adjust the popup's position as system-bar insets settle. The
    # regression is a visible SIZE correction, so keep positions as diagnostics.
    assert len(sizes) == 1, f"{name}: chooser resized after first visible frame: {sizes}"
    return result


if __name__ == "__main__":
    assert os.environ.get("BROWSEFORK_ANDROID_STARTUP") == "1", "Use npm run test:android-startup"
    assert re.fullmatch(r"emulator-\d+", SERIAL) and SERIAL != "emulator-5554", "Use a disposable read-only emulator clone, not the interactive emulator or a phone"
    assert adb("shell", "getprop", "ro.kernel.qemu").strip() == b"1", "Emulator required"
    OUTPUT.mkdir(parents=True, exist_ok=True)
    subprocess.run(["swiftc", str(ROOT / "tests/android-video-frames.swift"), "-o", str(OUTPUT / "decode-frames")], check=True)
    adb("install", "-r", str(ROOT / "tests/android-browser/build/outputs/apk/debug/test-browser-debug.apk"))
    adb("shell", "pm", "clear", "com.browsefork.app")
    results = [measure("fresh-cold", True), measure("cached-cold", True),
               measure("warm-1", False), measure("warm-2", False)]
    (OUTPUT / "latest.json").write_text(json.dumps(results, indent=2))
    print("PASS: no recorded full-screen white frames; every sampled chooser frame used the same size")
