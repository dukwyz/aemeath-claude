class SpriteAnimator {
  constructor(spriteEl, validationData) {
    // Replace the div with a canvas for clean rendering
    this.containerEl = spriteEl;
    this.canvas = document.createElement('canvas');
    this.canvas.width = 192;
    this.canvas.height = 208;
    this.canvas.style.imageRendering = 'pixelated';
    this.canvas.style.transition = 'opacity 0.3s ease';
    this.ctx = this.canvas.getContext('2d');
    this.ctx.imageSmoothingEnabled = false;

    // Replace the sprite div with the canvas
    spriteEl.parentNode.replaceChild(this.canvas, spriteEl);
    this.el = this.canvas;

    this.frameInterval = 180;
    this.currentAnimation = null;
    this.frameIndex = 0;
    this.timer = null;

    // Load spritesheet image
    this.sheetImg = new Image();
    this.sheetImg.src = 'spritesheet.webp';

    // Build frame map from validation.json
    this.frameMap = {};
    for (const cell of validationData.cells) {
      if (!cell.used) continue;
      if (!this.frameMap[cell.state]) {
        this.frameMap[cell.state] = [];
      }
      this.frameMap[cell.state].push({
        x: cell.x || cell.column * 192,
        y: cell.y || cell.row * 208,
        w: cell.w || 192,
        h: cell.h || 208
      });
    }
  }

  play(state) {
    if (state === this.currentAnimation) return;

    const frames = this.frameMap[state];
    const animName = frames ? state : 'idle';

    this.currentAnimation = animName;
    this.frameIndex = 0;
    this.stop();
    this._tick();

    const animFrames = this.frameMap[animName];
    if (animFrames && animFrames.length > 1) {
      this.timer = setInterval(() => {
        this.frameIndex = (this.frameIndex + 1) % animFrames.length;
        this._tick();
      }, this.frameInterval);
    }
  }

  _tick() {
    const frames = this.frameMap[this.currentAnimation];
    if (!frames || frames.length === 0) return;
    if (!this.sheetImg.complete) return; // wait for image load

    const frame = frames[this.frameIndex % frames.length];

    // Clear canvas and draw the frame
    this.ctx.clearRect(0, 0, 192, 208);
    this.ctx.drawImage(
      this.sheetImg,
      frame.x, frame.y, frame.w, frame.h,  // source
      0, 0, frame.w, frame.h                // destination
    );
  }

  stop() {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }
}

window.SpriteAnimator = SpriteAnimator;
