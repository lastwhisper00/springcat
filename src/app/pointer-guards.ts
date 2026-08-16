interface ButtonDefaultEvent {
  readonly button: number;
  preventDefault(): void;
}

/**
 * WebView engines use a middle-button press to start native autoscroll. The
 * indicator is larger than SpringCat's collapsed window and can cover the orb.
 */
export function blockMiddleButtonDefault(event: ButtonDefaultEvent): void {
  if (event.button === 1) event.preventDefault();
}
