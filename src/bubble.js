class Bubble {
  constructor(el) {
    this.el = el;
    this.queue = [];
    this.displayTimer = null;
    this.displayMs = 4000;
    this.isPersistent = false;
  }

  show(text) {
    if (!text) {
      this.hide();
      return;
    }

    this.isPersistent = false;
    this.el.classList.remove('persistent');

    // If bubble is already visible and not persistent, enqueue
    if (
      !this.el.classList.contains('hidden') &&
      !this.el.classList.contains('fade-out') &&
      !this.isPersistent
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

    this.isPersistent = true;
    this.el.classList.add('persistent');
    this._display(text, true);
  }

  _display(text, persistent) {
    this.el.textContent = text;
    this.el.classList.remove('hidden', 'fade-out');
    this.el.classList.add('visible');

    if (this.displayTimer) clearTimeout(this.displayTimer);

    if (!persistent) {
      this.displayTimer = setTimeout(() => {
        this.el.classList.add('fade-out');
        setTimeout(() => {
          this.el.classList.add('hidden');
          this.el.classList.remove('visible', 'fade-out');
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
  }
}

window.Bubble = Bubble;
