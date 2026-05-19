#!/usr/bin/env python3
"""
Game Guard — 自动检测全屏游戏，隐身/显示爱弥斯桌宠。

使用方式：python game_guard.py
后台运行，检测到全屏窗口时自动隐藏桌宠，切回桌面时自动显示。

原理：通过 Win32 API 检测当前前台窗口是否为全屏（覆盖整个屏幕）。
"""

import ctypes
import ctypes.wintypes
import time
import urllib.request
import json
import sys

PET_URL = "http://127.0.0.1:9527"
CHECK_INTERVAL = 3  # seconds

# 始终全屏的系统窗口，排除误判
SYSTEM_FULLSCREEN_TITLES = [
    "Program Manager",
    "Windows 输入体验",
    "Windows Input Experience",
]

# Win32 API
user32 = ctypes.windll.user32

class RECT(ctypes.Structure):
    _fields_ = [("left", ctypes.c_long), ("top", ctypes.c_long),
                 ("right", ctypes.c_long), ("bottom", ctypes.c_long)]

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

    # Get screen size
    screen_w = user32.GetSystemMetrics(0)
    screen_h = user32.GetSystemMetrics(1)

    # Check window style (WS_POPUP = fullscreen-like)
    style = user32.GetWindowLongW(hwnd, -16)  # GWL_STYLE
    is_popup = bool(style & 0x80000000)  # WS_POPUP

    return {
        "hwnd": hwnd,
        "title": title,
        "class": class_name,
        "width": width,
        "height": height,
        "screen_w": screen_w,
        "screen_h": screen_h,
        "is_fullscreen": (width >= screen_w and height >= screen_h) and not any(t in title for t in SYSTEM_FULLSCREEN_TITLES),
        "is_popup": is_popup,
    }

def pet_request(path):
    """Send HTTP request to pet server."""
    try:
        req = urllib.request.Request(
            f"{PET_URL}{path}",
            method="POST",
            data=b"",
        )
        urllib.request.urlopen(req, timeout=2)
        return True
    except Exception:
        return False

def main():
    was_hidden = False
    print("Game Guard started. Monitoring fullscreen windows...")
    print(f"Checking every {CHECK_INTERVAL}s. Press Ctrl+C to stop.")

    while True:
        try:
            info = get_foreground_window_info()
            if info:
                is_fs = info["is_fullscreen"]

                if is_fs and not was_hidden:
                    # Game detected → hide pet
                    if pet_request("/api/hide"):
                        print(f"[HIDDEN] Fullscreen: {info['title'][:50]} ({info['width']}x{info['height']})")
                    was_hidden = True

                elif not is_fs and was_hidden:
                    # Back to desktop → show pet
                    if pet_request("/api/show"):
                        print(f"[SHOWN] Window: {info['title'][:50]}")
                    was_hidden = False

        except KeyboardInterrupt:
            print("\nGame Guard stopped.")
            # Make sure pet is visible when we exit
            if was_hidden:
                pet_request("/api/show")
            sys.exit(0)
        except Exception as e:
            pass  # Silently continue on errors

        time.sleep(CHECK_INTERVAL)

if __name__ == "__main__":
    main()
