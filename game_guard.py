#!/usr/bin/env python3
"""
Game Guard - auto-detect fullscreen windows, hide/show Aemeath desktop pet.

Usage: python game_guard.py
Runs in background, hides pet when fullscreen window detected, shows when back to desktop.
Supports browser fullscreen (Bilibili), game fullscreen, and borderless fullscreen.
"""

import ctypes
import ctypes.wintypes
import time
import urllib.request
import sys
import os
import subprocess

PET_URL = "http://127.0.0.1:9527"
CHECK_INTERVAL = 3  # seconds
SLEEP_THRESHOLD = 8  # seconds — if gap > this, assume resume from sleep

# Log file for debugging (pythonw has no console)
LOG_DIR = os.path.dirname(os.path.abspath(__file__))
LOG_FILE = os.path.join(LOG_DIR, "game_guard.log")
PET_EXE = os.path.join(LOG_DIR, "aemeath-claude.exe")

# System fullscreen windows to exclude
SYSTEM_FULLSCREEN_TITLES = [
    "Program Manager",
]

# Window classes to exclude (desktop shell)
EXCLUDED_CLASSES = ["Progman", "WorkerW"]

# Win32 API constants
WS_POPUP = 0x80000000
WS_CAPTION = 0x00C00000  # WS_BORDER | WS_DLGFRAME

# Win32 API
user32 = ctypes.windll.user32

class RECT(ctypes.Structure):
    _fields_ = [("left", ctypes.c_long), ("top", ctypes.c_long),
                 ("right", ctypes.c_long), ("bottom", ctypes.c_long)]

def log(msg):
    """Write to log file."""
    try:
        with open(LOG_FILE, "a", encoding="utf-8") as f:
            f.write("[%s] %s\n" % (time.strftime("%Y-%m-%d %H:%M:%S"), msg))
    except Exception:
        pass

def get_foreground_window_info():
    """Get the foreground window's title, class, and dimensions."""
    hwnd = user32.GetForegroundWindow()
    if not hwnd:
        return None

    # Get window title
    length = user32.GetWindowTextLengthW(hwnd)
    buf = ctypes.create_unicode_buffer(length + 1)
    user32.GetWindowTextW(hwnd, buf, length + 1)
    title = buf.value

    # Get window class
    class_buf = ctypes.create_unicode_buffer(256)
    user32.GetClassNameW(hwnd, class_buf, 256)
    class_name = class_buf.value

    # Get window rect
    rect = RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(rect))
    width = rect.right - rect.left
    height = rect.bottom - rect.top
    x = rect.left
    y = rect.top

    # Get screen size (logical pixels)
    screen_w = user32.GetSystemMetrics(0)
    screen_h = user32.GetSystemMetrics(1)

    # Check window styles
    style = user32.GetWindowLongW(hwnd, -16)  # GWL_STYLE
    is_popup = bool(style & WS_POPUP)
    has_caption = bool(style & WS_CAPTION)

    # === Fullscreen detection (2 methods) ===

    # Method 1: Traditional fullscreen (game exclusive/borderless)
    #   Size ~= screen AND (WS_POPUP or no caption)
    #   Works for: game fullscreen, game borderless fullscreen
    is_traditional = (
        width >= screen_w - 15 and height >= screen_h - 15
        and (is_popup or not has_caption)
    )

    # Method 2: Browser fullscreen (HTML5 Fullscreen API, F11)
    #   Size >= 95% screen AND no caption bar
    #   The no-caption check distinguishes from maximized windows (which have caption)
    #   Works for: Bilibili, YouTube, any browser fullscreen
    is_browser = (
        width >= int(screen_w * 0.95) and height >= int(screen_h * 0.95)
        and not has_caption
    )

    # Exclude system windows
    is_excluded = (
        class_name in EXCLUDED_CLASSES
        or any(t in title for t in SYSTEM_FULLSCREEN_TITLES)
    )

    is_fullscreen = (is_traditional or is_browser) and not is_excluded

    return {
        "hwnd": hwnd,
        "title": title,
        "class": class_name,
        "width": width,
        "height": height,
        "x": x,
        "y": y,
        "screen_w": screen_w,
        "screen_h": screen_h,
        "is_fullscreen": is_fullscreen,
        "is_traditional": is_traditional,
        "is_browser": is_browser,
        "is_popup": is_popup,
        "has_caption": has_caption,
        "is_excluded": is_excluded,
    }

def pet_request(path):
    """Send HTTP request to pet server."""
    try:
        req = urllib.request.Request(
            "%s%s" % (PET_URL, path),
            method="POST",
            data=b"",
        )
        urllib.request.urlopen(req, timeout=2)
        return True
    except Exception:
        return False

def is_pet_running():
    """Check if pet HTTP server is responsive."""
    try:
        req = urllib.request.Request(
            "%s/api/heartbeat" % PET_URL,
            method="GET",
        )
        urllib.request.urlopen(req, timeout=2)
        return True
    except Exception:
        return False

def ensure_pet_running():
    """Start pet exe if not already running."""
    if is_pet_running():
        return
    log("RESUME: pet not running, starting %s" % PET_EXE)
    try:
        subprocess.Popen(
            [PET_EXE],
            cwd=os.path.dirname(PET_EXE),
            creationflags=getattr(subprocess, 'DETACHED_PROCESS', 0x00000008),
        )
    except Exception as e:
        log("RESUME: failed to start pet: %s" % e)

def main():
    was_hidden = False
    last_check = time.time()
    log("STARTED pid=%d" % os.getpid())
    print("Game Guard started. Monitoring fullscreen windows...")
    print("Checking every %ds. Press Ctrl+C to stop." % CHECK_INTERVAL)

    while True:
        try:
            # Sleep resume detection: if gap > threshold, we likely woke from sleep
            now = time.time()
            gap = now - last_check
            if gap > SLEEP_THRESHOLD:
                log("RESUME: detected sleep gap of %ds" % int(gap))
                ensure_pet_running()
            last_check = now

            info = get_foreground_window_info()
            if info:
                is_fs = info["is_fullscreen"]

                if is_fs and not was_hidden:
                    if pet_request("/api/hide"):
                        log("[HIDDEN] %s (%dx%d at %d,%d) cls=%s trad=%s browser=%s popup=%s caption=%s" % (
                            info["title"][:40],
                            info["width"], info["height"],
                            info["x"], info["y"],
                            info["class"],
                            info["is_traditional"], info["is_browser"],
                            info["is_popup"], info["has_caption"],
                        ))
                    was_hidden = True

                elif not is_fs and was_hidden:
                    if pet_request("/api/show"):
                        log("[SHOWN] %s (%dx%d)" % (
                            info["title"][:40],
                            info["width"], info["height"],
                        ))
                    was_hidden = False

        except KeyboardInterrupt:
            log("STOPPED")
            print("\nGame Guard stopped.")
            if was_hidden:
                pet_request("/api/show")
            sys.exit(0)
        except Exception:
            pass

        time.sleep(CHECK_INTERVAL)

if __name__ == "__main__":
    main()
