interface ToastAction {
  label: string;
  onClick: () => void;
}

class ToastState {
  message = $state<string | null>(null);
  action = $state<ToastAction | null>(null);
  private timer: ReturnType<typeof setTimeout> | null = null;

  show(message: string, opts: { ms?: number; action?: ToastAction } = {}) {
    this.message = message;
    this.action = opts.action ?? null;
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => this.dismiss(), opts.ms ?? 2000);
  }

  runAction() {
    this.action?.onClick();
    this.dismiss();
  }

  dismiss() {
    this.message = null;
    this.action = null;
    if (this.timer) clearTimeout(this.timer);
  }
}

export const toast = new ToastState();
