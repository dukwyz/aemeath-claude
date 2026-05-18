let animator;
let bubble;
let lastBubble = '';
let toolLockUntil = 0;
let idleStart = 0;
let idleAnimTimer = null;
let permissionPending = false;
let permissionTimer = null;
let permissionFadeTimer = null;
let permissionClearTimer = null;
let idleRemindTimer = null;

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
  'running', 'chatting', 'fetching', 'searching', 'analyzing', 'building'
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
  if (idleRemindTimer) return;
  idleRemindTimer = setTimeout(doIdleRemind, IDLE_REMIND_INTERVAL);
}
function doIdleRemind() {
  idleRemindTimer = null;
  if (!idleStart) return;
  const elapsed = Date.now() - idleStart;
  if (elapsed >= 300000) { // 5+ minutes idle
    const msg = IDLE_REMINDERS[Math.floor(Math.random() * IDLE_REMINDERS.length)];
    if (window._petBubble) window._petBubble.showPersistent(msg);
  }
  scheduleIdleRemind();
}
function cancelIdleRemind() {
  if (idleRemindTimer) { clearTimeout(idleRemindTimer); idleRemindTimer = null; }
}

async function pollState() {
  while (true) {
    try {
      const r = await fetch('http://127.0.0.1:9527/api/current');
      if (r.ok) {
        const data = await r.json();
        if (data.animation) {
          window._petAnimator.play(data.animation);
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
              permissionTimer = setInterval(() => {
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
  if (permissionTimer) { clearInterval(permissionTimer); permissionTimer = null; }
  if (permissionFadeTimer) { clearTimeout(permissionFadeTimer); permissionFadeTimer = null; }
  if (permissionClearTimer) { clearTimeout(permissionClearTimer); permissionClearTimer = null; }
  // Reset opacity if it was faded
  if (window._petBubble) {
    window._petBubble.el.style.transition = '';
    window._petBubble.el.style.opacity = '';
  }
}

function schedulePermissionRecovery() {
  // 15s: start fading the persistent bubble
  permissionFadeTimer = setTimeout(() => {
    if (permissionPending && window._petBubble) {
      window._petBubble.el.style.transition = 'opacity 2s ease';
      window._petBubble.el.style.opacity = '0.4';
    }
  }, 15000);
  // 60s: fully clear permission state
  permissionClearTimer = setTimeout(() => {
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
  if (idleAnimTimer) return;
  idleAnimTimer = setTimeout(doIdleAnim, 15000 + Math.random() * 30000);
}
function doIdleAnim() {
  idleAnimTimer = null;
  if (!idleStart) return;
  const pick = ['jumping','waving','chatting'][Math.floor(Math.random()*3)];
  window._petAnimator.play(pick);
  setTimeout(() => { if (window._petAnimator) window._petAnimator.play('idle'); scheduleIdleAnim(); }, 2000);
}
function cancelIdleAnim() {
  if (idleAnimTimer) { clearTimeout(idleAnimTimer); idleAnimTimer = null; }
}
document.addEventListener('DOMContentLoaded', init);
