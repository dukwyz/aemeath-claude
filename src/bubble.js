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
    const askText = document.getElementById('ask-text');
    const askRow = document.getElementById('ask-confirm-row');
    if (!askBubble || !askText || !askRow) return;

    askText.textContent = text || '等待指示...';
    askRow.classList.remove('hidden');
    askBubble.classList.remove('hidden');
    askBubble.classList.add('visible');

    // Hide regular bubble while confirm is showing
    this.hide();
  }

  hideConfirm() {
    const askBubble = document.getElementById('ask-bubble');
    const askRow = document.getElementById('ask-confirm-row');
    if (!askBubble) return;

    askBubble.classList.remove('visible');
    askBubble.classList.add('hidden');
    if (askRow) askRow.classList.add('hidden');
  }
}

window.Bubble = Bubble;
