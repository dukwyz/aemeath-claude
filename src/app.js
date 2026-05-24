let animator;
let bubble;
let lastBubble = '';
let toolLockUntil = 0;
let idleStart = 0;
let permissionPending = false;
let idleAnimActive = false;

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
  pollState();
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
        if (data.animation) {
          // 空闲动画播放中，跳过 idle → idle 的覆盖（但非 idle 状态仍正常切换）
          if (!(idleAnimActive && data.animation === 'idle')) {
            window._petAnimator.play(data.animation);
          }
          // Approve/new message: clear permission on non-waving, non-idle
          if (permissionPending && data.animation !== 'waving' && data.animation !== 'idle') {
            exitPermission();
            window._petBubble.hide();
          }
          // ESC/deny → idle: clear permission silently
          if (permissionPending && data.animation === 'idle') {
            exitPermission();
            window._petBubble.hide();
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
          if (data.bubble.includes('等待指示')) {
            if (!permissionPending) {
              permissionPending = true;
              setNamedInterval('permissionRepeat', () => {
                if (permissionPending && window._petBubble) {
                  window._petBubble.showPersistent('等待指示...');
                }
              }, 3000);
              // Progressive recovery: fade at 15s, clear at 60s
              schedulePermissionRecovery();
            }
            lastBubble = data.bubble;
            window._petBubble.showPersistent(data.bubble);
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
    await new Promise(r => setTimeout(r, 400));
  }
}

function exitPermission() {
  permissionPending = false;
  lastBubble = '';  // 重置，让下一次 poll 能正确刷新气泡
  clearNamedTimer('permissionRepeat');
  clearNamedTimer('permissionFade');
  clearNamedTimer('permissionClear');
  // Reset opacity if it was faded
  if (window._petBubble) {
    window._petBubble.el.style.transition = '';
    window._petBubble.el.style.opacity = '';
  }
}

function schedulePermissionRecovery() {
  // 15s: start fading the persistent bubble
  setNamedTimeout('permissionFade', () => {
    if (permissionPending && window._petBubble) {
      window._petBubble.el.style.transition = 'opacity 2s ease';
      window._petBubble.el.style.opacity = '0.4';
    }
  }, 15000);
  // 60s: fully clear permission state
  setNamedTimeout('permissionClear', () => {
    if (permissionPending) {
      if (window._petBubble) {
        window._petBubble.el.style.transition = 'opacity 0.5s ease';
        window._petBubble.el.style.opacity = '';
        exitPermission();
        window._petBubble.hide();
      }
    }
  }, 60000);
}

function scheduleIdleAnim() {
  if (timerManager._timers.has('idleAnim')) return;
  setNamedTimeout('idleAnim', doIdleAnim, 15000 + Math.random() * 30000);
}
function doIdleAnim() {
  if (!idleStart) return;
  idleAnimActive = true;
  const pick = ['jumping','waving','chatting'][Math.floor(Math.random()*3)];
  window._petAnimator.play(pick);
  setNamedTimeout('idleAnimReturn', () => {
    idleAnimActive = false;
    if (!idleStart) return; // 已离开 idle，不再回退
    if (window._petAnimator) window._petAnimator.play('idle');
    scheduleIdleAnim();
  }, 2000);
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
