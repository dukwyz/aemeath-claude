let animator;
let bubble;
let lastBubble = '';
let toolLockUntil = 0;
let idleStart = 0;
let permissionPending = false;
let permissionResolvedAt = 0;  // 防止点击后立刻重新触发
let confirmShownAt = 0;  // 按钮最少显示 3 秒
let idleAnimActive = false;
let overlayActive = false;    // MCP overlay 显示中
let overlayType = '';         // "confirm" | "select" | "text"
let lastAnimSwitchAt = 0;     // 动画防抖：距上次切换的时间戳

// Timer manager to prevent leaks
const timerManager = {
  _timers: new Map(),
  set(name, fn, delay) {
    this.clear(name);
    const id = setTimeout(() => {
      this._timers.delete(name);
      fn();
    }, delay);
    this._timers.set(name, { id, type: 'timeout' });
    return id;
  },
  setInterval(name, fn, delay) {
    this.clear(name);
    const id = setInterval(fn, delay);
    this._timers.set(name, { id, type: 'interval' });
    return id;
  },
  clear(name) {
    const timer = this._timers.get(name);
    if (!timer) return;
    if (timer.type === 'timeout') clearTimeout(timer.id);
    else clearInterval(timer.id);
    this._timers.delete(name);
  },
  clearAll() {
    for (const [name, timer] of this._timers) {
      if (timer.type === 'timeout') clearTimeout(timer.id);
      else clearInterval(timer.id);
    }
    this._timers.clear();
  }
};

// Named timer aliases
function setNamedTimeout(name, fn, delay) { return timerManager.set(name, fn, delay); }
function setNamedInterval(name, fn, delay) { return timerManager.setInterval(name, fn, delay); }
function clearNamedTimer(name) { timerManager.clear(name); }

const IDLE_REMINDERS = [
  "小主还在吗~",
  "Claude 等你好久了哦",
  "要回来干活了吗？",
  "爱弥斯有点无聊了...",
  "需要帮忙的话随时叫我~",
  "工作还没做完呢~",
  "别忘了还有 Claude 在等你~",
  "要不要回来看看？",
];
const IDLE_REMIND_INTERVAL = 120000 + Math.random() * 60000; // 2-3 min

const PERSISTENT_STATES = new Set([
  'running', 'running-left', 'running-right',
  'chatting', 'fetching', 'searching', 'analyzing', 'building'
]);

async function init() {
  const resp = await fetch('validation.json');
  const validationData = await resp.json();
  const spriteEl = document.getElementById('sprite');
  const bubbleEl = document.getElementById('bubble');
  animator = new SpriteAnimator(spriteEl, validationData);
  bubble = new Bubble(bubbleEl);
  animator.play('waving');
  bubble.show('爱弥斯已上线~');
  window._petBubble = bubble;
  window._petAnimator = animator;
  const ipc = window.__TAURI_INTERNALS__;
  let lastClickTime = 0;
  let clickTimer = null;
  document.addEventListener('mousedown', (e) => {
    if (e.button !== 0) return;
    // Ask-bubble 内的点击不触发拖拽/双击逻辑
    if (e.target.closest('#ask-bubble')) return;
    const now = Date.now();
    if (now - lastClickTime < 200) {
      // 双击：打开 Obsidian
      clearTimeout(clickTimer);
      clickTimer = null;
      lastClickTime = 0;
      try { if (ipc && ipc.invoke) ipc.invoke('open_obsidian'); } catch (_) {}
    } else {
      // 单击：50ms 后拖拽（期间如果双击则取消）
      lastClickTime = now;
      clickTimer = setTimeout(() => {
        try { if (ipc && ipc.invoke) ipc.invoke('start_drag'); } catch (_) {}
      }, 50);
    }
  });
  setupConfirmButtons();
  setupInteractiveInputs();
  // 拦截 ask-bubble 上所有 mousedown，阻止冒泡到 document 触发拖拽
  const askBubble = document.getElementById('ask-bubble');
  if (askBubble) {
    askBubble.addEventListener('mousedown', (e) => {
      e.stopPropagation();
    });  // bubbling phase，在按钮 handler 之后拦截
  }
  pollState();
  pollPendingInput();
}

// ---- Interactive input handlers (MCP overlay) ----

function setupInteractiveInputs() {
  const ipc = window.__TAURI_INTERNALS__;

  // Yes / No buttons (permission mode)
  const btnYes = document.getElementById('ask-confirm-yes');
  const btnNo = document.getElementById('ask-confirm-no');
  if (btnYes) {
    btnYes.addEventListener('mousedown', (e) => {
      e.stopPropagation();
      e.preventDefault();
      if (overlayActive) {
        submitInteractive('yes');
      } else {
        try { if (ipc && ipc.invoke) ipc.invoke('approve_permission'); } catch (_) {}
        exitPermission();
        window._petBubble.hideConfirm();
      }
    });
  }
  if (btnNo) {
    btnNo.addEventListener('mousedown', (e) => {
      e.stopPropagation();
      e.preventDefault();
      if (overlayActive) {
        submitInteractive('no');
      } else {
        try { if (ipc && ipc.invoke) ipc.invoke('deny_permission'); } catch (_) {}
        exitPermission();
        window._petBubble.hideConfirm();
      }
    });
  }

  // Send / Back buttons (text input)
  const btnSend = document.getElementById('ask-send');
  const btnBack = document.getElementById('ask-back');
  if (btnSend) {
    btnSend.addEventListener('mousedown', (e) => {
      e.stopPropagation();
      e.preventDefault();
      if (overlayActive) {
        const input = document.getElementById('ask-input');
        submitInteractive((input && input.value) || '');
      }
    });
  }
  if (btnBack) {
    btnBack.addEventListener('mousedown', (e) => {
      e.stopPropagation();
      e.preventDefault();
      submitInteractive('dismiss');
    });
  }

  // Enter key on text input
  const askInput = document.getElementById('ask-input');
  if (askInput) {
    askInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        if (overlayActive) submitInteractive(askInput.value || '');
      }
    });
  }
}

async function submitInteractive(answer) {
  overlayActive = false;  // 乐观：立即标记，防止重新触发
  overlayType = '';
  if (window._petBubble) window._petBubble.hideConfirm();
  exitPermission();
  await submitPendingInput(answer);  // 确保后端收到再返回
}

function setupConfirmButtons() {
  // Button handlers are now in setupInteractiveInputs()
}

// ---- MCP oneshot pending input ----

async function submitPendingInput(answer) {
  try {
    await fetch('http://127.0.0.1:9527/api/user/input', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ answer }),
    });
  } catch (_) {}
}

async function pollPendingInput() {
  // Overlay UI is entirely driven by pollState via the overlay field.
  // This loop is kept only as a safety net for edge cases where
  // pollState misses a timeout. It does NOT manage overlayActive.
  while (true) {
    await new Promise(r => setTimeout(r, 2000));
  }
}

function isToolBubble(text) {
  return text && (
    text.includes('正在读取') || text.includes('正在写') ||
    text.includes('正在执行') || text.includes('正在调度') ||
    text.includes('正在搜索') || text.includes('正在获取') ||
    text.includes('正在分析') || text.includes('正在构建') ||
    text.includes('工作中')
  );
}

function shouldPersist(animation) {
  return PERSISTENT_STATES.has(animation);
}

function scheduleIdleRemind() {
  if (timerManager._timers.has('idleRemind')) return;
  setNamedTimeout('idleRemind', doIdleRemind, IDLE_REMIND_INTERVAL);
}
function doIdleRemind() {
  if (!idleStart) return;
  const elapsed = Date.now() - idleStart;
  if (elapsed >= 300000) { // 5+ minutes idle
    const msg = IDLE_REMINDERS[Math.floor(Math.random() * IDLE_REMINDERS.length)];
    lastBubble = msg;
    if (window._petBubble) window._petBubble.showPersistent(msg);
  }
  scheduleIdleRemind();
}
function cancelIdleRemind() {
  clearNamedTimer('idleRemind');
}

async function pollState() {
  while (true) {
    try {
      const r = await fetch('http://127.0.0.1:9527/api/current');
      if (r.ok) {
        const data = await r.json();

        // Overlay check FIRST — skip all bubble logic when overlay is active
        if (data.overlay === 'input') {
          if (!overlayActive) {
            overlayActive = true;
            overlayType = data.input_type || 'confirm';
            // 用 showConfirm，跟权限气泡完全一致
            if (window._petBubble) {
              window._petBubble.showConfirm(data.question || '确认？');
            }
          }
          // Always keep regular bubble hidden during overlay
          if (window._petBubble) window._petBubble.hide();
          await new Promise(r => setTimeout(r, 200));
          continue;
        }
        // No overlay: hide if was showing
        if (overlayActive) {
          overlayActive = false;
          overlayType = '';
          if (window._petBubble) window._petBubble.hideConfirm();
        }

        if (data.animation) {
          // 动画防抖：非 idle 状态切换间隔 < 800ms 时跳过，防止抽搐
          const now = Date.now();
          const isIdleAnim = idleAnimActive && data.animation === 'idle';
          const tooFast = data.animation !== 'idle' && (now - lastAnimSwitchAt) < 800;
          if (!isIdleAnim && !tooFast) {
            window._petAnimator.play(data.animation);
            lastAnimSwitchAt = now;
            if (data.animation !== 'idle') {
              cancelIdleAnim();
            }
          }

          // Approve/new message: clear permission on non-waving, non-idle
          // 但按钮最少显示 3 秒，防止状态切换太快把按钮清掉
          if (permissionPending && data.animation !== 'waving' && data.animation !== 'idle') {
            if (Date.now() - confirmShownAt > 3000) {
              exitPermission();
              window._petBubble.hide();
            }
          }
          // ESC/deny → idle: clear permission silently
          if (permissionPending && data.animation === 'idle') {
            if (Date.now() - confirmShownAt > 3000) {
              exitPermission();
              window._petBubble.hide();
            }
          }
          // Idle animation + reminders
          if (data.animation === 'idle') {
            if (!idleStart) idleStart = Date.now();
            scheduleIdleAnim();
            scheduleIdleRemind();
          } else {
            idleStart = 0;
            cancelIdleAnim();
            cancelIdleRemind();
          }
          // failed → persistent
          if (data.animation === 'failed' && data.bubble) {
            lastBubble = data.bubble;
            window._petBubble.showPersistent(data.bubble);
            continue;
          }
        }
        if (data.bubble && data.bubble !== lastBubble) {
          const now = Date.now();
          // Skip permission bubble while MCP overlay is active
          if (data.bubble.includes('等待指示') && !overlayActive) {
              // 隐藏普通气泡，只显示权限气泡
              if (window._petBubble) window._petBubble.hide();
            // 点击按钮后 2 秒内不重新触发 permission 模式
            if (!permissionPending && Date.now() - permissionResolvedAt > 2000) {
              permissionPending = true;
              confirmShownAt = Date.now();
              setNamedInterval('permissionRepeat', () => {
                if (permissionPending && window._petBubble) {
                  window._petBubble.showConfirm('等待指示...');
                }
              }, 3000);
              // Progressive recovery: fade at 15s, clear at 60s
              schedulePermissionRecovery();
            }
            lastBubble = data.bubble;
            // 只在 permissionPending 为 true 时显示按钮
            if (permissionPending) {
              window._petBubble.showConfirm(data.bubble);
            }
          } else if (permissionPending) {
            if (isToolBubble(data.bubble)) {
              exitPermission();
              lastBubble = data.bubble;
              window._petBubble.showPersistent(data.bubble);
              toolLockUntil = now + 1200;
            }
          } else {
            const anim = data.animation || '';
            if (shouldPersist(anim) && isToolBubble(data.bubble)) {
              // Active tool execution: persistent bubble
              lastBubble = data.bubble;
              window._petBubble.showPersistent(data.bubble);
              toolLockUntil = now + 1200;
            } else if (isToolBubble(lastBubble) && now < toolLockUntil && !isToolBubble(data.bubble)) {
              // keep
            } else {
              lastBubble = data.bubble;
              window._petBubble.show(data.bubble);
              if (isToolBubble(data.bubble)) toolLockUntil = now + 1200;
            }
          }
        }
      }
    } catch (_) {}
    await new Promise(r => setTimeout(r, 200));
  }
}

function exitPermission() {
  permissionPending = false;
  permissionResolvedAt = Date.now();  // 2秒内不重新触发
  lastBubble = '';  // 重置，让下一次 poll 能正确刷新气泡
  clearNamedTimer('permissionRepeat');
  clearNamedTimer('permissionFade');
  clearNamedTimer('permissionClear');
  // Reset opacity if it was faded
  if (window._petBubble) {
    window._petBubble.el.style.transition = '';
    window._petBubble.el.style.opacity = '';
    window._petBubble.hideConfirm();
  }
}

function schedulePermissionRecovery() {
  // 15s: start fading the ask-bubble
  setNamedTimeout('permissionFade', () => {
    if (permissionPending) {
      const askBubble = document.getElementById('ask-bubble');
      if (askBubble) {
        askBubble.style.transition = 'opacity 2s ease';
        askBubble.style.opacity = '0.4';
      }
    }
  }, 15000);
  // 60s: fully clear permission state
  setNamedTimeout('permissionClear', () => {
    if (permissionPending) {
      const askBubble = document.getElementById('ask-bubble');
      if (askBubble) {
        askBubble.style.transition = 'opacity 0.5s ease';
        askBubble.style.opacity = '';
      }
      exitPermission();
      if (window._petBubble) window._petBubble.hideConfirm();
    }
  }, 60000);
}

function scheduleIdleAnim() {
  if (timerManager._timers.has('idleAnim')) return;
  setNamedTimeout('idleAnim', doIdleAnim, 20000 + Math.random() * 40000);
}
function doIdleAnim() {
  if (!idleStart) return;
  idleAnimActive = true;
  const pick = ['jumping','waving'][Math.floor(Math.random()*2)];
  window._petAnimator.play(pick);
  setNamedTimeout('idleAnimReturn', () => {
    idleAnimActive = false;
    if (!idleStart) return; // 已离开 idle，不再回退
    if (window._petAnimator) window._petAnimator.play('idle');
    scheduleIdleAnim();
  }, 3000);
}
function cancelIdleAnim() {
  idleAnimActive = false;
  clearNamedTimer('idleAnim');
  clearNamedTimer('idleAnimReturn');
}

// Clean up all timers on page unload
window.addEventListener('beforeunload', () => {
  timerManager.clearAll();
});

document.addEventListener('DOMContentLoaded', init);
