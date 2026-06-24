class ToastState {
  message = $state<string | null>(null);
  private timer: ReturnType<typeof setTimeout> | null = null;

  show(message: string, ms = 2000) {
    this.message = message;
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => (this.message = null), ms);
  }
}

export const toast = new ToastState();
