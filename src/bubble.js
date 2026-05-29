class Bubble {
  constructor(el) {
    this.el = el;
    this.queue = [];
    this.displayTimer = null;
    this.fadeTimer = null;
    this.displayMs = 4000;
    this.isPersistent = false;
  }

  show(text) {
    if (!text) {
      this.hide();
      return;
    }

    // If transitioning from persistent to non-persistent, clear queue first
    if (this.isPersistent) {
      this.isPersistent = false;
      this.el.classList.remove('persistent');
      this.queue = [];
    }

    // If bubble is already visible and not in fade-out, enqueue
    if (
      !this.el.classList.contains('hidden') &&
      !this.el.classList.contains('fade-out')
    ) {
      this.queue.push(text);
      return;
    }

    this._display(text, false);
  }

  showPersistent(text) {
    if (!text) {
      this.hide();
      return;
    }

    // Clear any pending fade timers and queue when going persistent
    if (this.fadeTimer) {
      clearTimeout(this.fadeTimer);
      this.fadeTimer = null;
    }
    this.queue = [];

    this.isPersistent = true;
    this.el.classList.add('persistent');
    this._display(text, true);
  }

  _display(text, persistent) {
    this.el.textContent = text;
    this.el.classList.remove('hidden', 'fade-out');
    this.el.classList.add('visible');

    if (this.displayTimer) clearTimeout(this.displayTimer);
    if (this.fadeTimer) clearTimeout(this.fadeTimer);

    if (!persistent) {
      this.displayTimer = setTimeout(() => {
        this.el.classList.add('fade-out');
        this.fadeTimer = setTimeout(() => {
          this.el.classList.add('hidden');
          this.el.classList.remove('visible', 'fade-out');
          this.fadeTimer = null;
          if (this.queue.length > 0) {
            const next = this.queue.shift();
            this._display(next, false);
          }
        }, 400);
      }, this.displayMs);
    }
    // persistent mode: no auto-hide timer, stays until explicitly hidden or overwritten
  }

  hide() {
    this.isPersistent = false;
    this.el.classList.remove('persistent');
    this.el.classList.add('hidden');
    this.el.classList.remove('visible', 'fade-out');
    this.queue = [];
    if (this.displayTimer) {
      clearTimeout(this.displayTimer);
      this.displayTimer = null;
    }
    if (this.fadeTimer) {
      clearTimeout(this.fadeTimer);
      this.fadeTimer = null;
    }
  }

  // ---- confirm button (permission) ----

  showConfirm(text) {
    const askBubble = document.getElementById('ask-bubble');
    const askPrompt = document.getElementById('ask-prompt');
    const askRow = document.getElementById('ask-confirm-row');
    if (!askBubble || !askPrompt || !askRow) return;

    // 如果已经在显示，只刷新内容，不重置 waiting 状态
    const alreadyVisible = askBubble.classList.contains('visible');

    // 权限气泡：不显示文字，只留 ✓ ✗ 按钮
    askPrompt.textContent = '';
    askPrompt.style.display = 'none';
    askRow.classList.remove('hidden');
    askBubble.classList.remove('hidden');
    askBubble.classList.add('visible');
    // 清除 schedulePermissionRecovery 设的 inline opacity，保持按钮 100% 可见
    askBubble.style.transition = '';
    askBubble.style.opacity = '';

    // 首次显示时：启动 10s 呼吸计时器 + 隐藏普通气泡
    if (!alreadyVisible) {
      this._waitingTimer = setTimeout(() => {
        if (askBubble.classList.contains('visible')) {
          askBubble.classList.add('waiting');
        }
      }, 10000);
      // 仅首次隐藏普通气泡，后续 poll 刷新不再压制
      this.hide();
    }
  }

  hideConfirm() {
    const askBubble = document.getElementById('ask-bubble');
    const askPrompt = document.getElementById('ask-prompt');
    const askRow = document.getElementById('ask-confirm-row');
    if (!askBubble) return;

    // 立即隐藏，display:none 无残影
    clearTimeout(this._waitingTimer);
    askBubble.classList.remove('visible', 'waiting');
    askBubble.classList.add('hidden');
    if (askRow) askRow.classList.add('hidden');
    // 重置 prompt 显示状态，下次 showInteractive 时能正常显示
    if (askPrompt) askPrompt.style.display = '';
  }

  // ---- interactive input (MCP oneshot: confirm / text / select) ----

  showInteractive(prompt, inputType, options) {
    const askBubble = document.getElementById('ask-bubble');
    const askPrompt = document.getElementById('ask-prompt');
    const confirmRow = document.getElementById('ask-confirm-row');
    const inputRow = document.getElementById('ask-input-row');
    const choicesDiv = document.getElementById('ask-choices');
    if (!askBubble) return;

    // Hide regular bubble and any lingering permission confirm
    this.hide();
    this.hideConfirm();

    // Hide all sub-areas first
    if (confirmRow) confirmRow.classList.add('hidden');
    if (inputRow) inputRow.classList.add('hidden');
    if (choicesDiv) { choicesDiv.innerHTML = ''; choicesDiv.classList.add('hidden'); }

    // Set prompt (MCP overlay 显示问题文字)
    if (askPrompt) {
      askPrompt.style.display = '';
      askPrompt.textContent = prompt || '';
    }

    // Show the right UI based on inputType
    switch (inputType) {
      case 'confirm':
        if (confirmRow) confirmRow.classList.remove('hidden');
        break;
      case 'text':
        if (inputRow) inputRow.classList.remove('hidden');
        const inputEl = document.getElementById('ask-input');
        if (inputEl) {
          inputEl.value = '';
          setTimeout(() => inputEl.focus(), 50);
        }
        break;
      case 'select':
        if (choicesDiv && options && options.length > 0) {
          choicesDiv.classList.remove('hidden');
          options.forEach((opt, i) => {
            const btn = document.createElement('button');
            btn.className = 'ask-choice-btn';
            btn.textContent = opt;
            btn.dataset.value = opt;
            btn.dataset.index = String(i);
            choicesDiv.appendChild(btn);
          });
        }
        break;
    }

    // Show the bubble
    askBubble.classList.remove('hidden');
    askBubble.classList.add('visible');
  }

  hideInteractive() {
    const askBubble = document.getElementById('ask-bubble');
    const confirmRow = document.getElementById('ask-confirm-row');
    const inputRow = document.getElementById('ask-input-row');
    const choicesDiv = document.getElementById('ask-choices');

    if (!askBubble) return;
    askBubble.classList.remove('visible');
    askBubble.classList.add('hidden');
    if (confirmRow) confirmRow.classList.add('hidden');
    if (inputRow) inputRow.classList.add('hidden');
    if (choicesDiv) { choicesDiv.innerHTML = ''; choicesDiv.classList.add('hidden'); }
  }
}

window.Bubble = Bubble;
