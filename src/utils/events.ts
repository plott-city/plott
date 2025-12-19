type EventHandler = (...args: unknown[]) => void;

/** Simple typed event emitter for internal agent lifecycle events */
export class EventBus {
  private handlers = new Map<string, EventHandler[]>();

  on(event: string, handler: EventHandler): void {
    const existing = this.handlers.get(event) || [];
    existing.push(handler);
    this.handlers.set(event, existing);
  }

  off(event: string, handler: EventHandler): void {
    const existing = this.handlers.get(event) || [];
    this.handlers.set(
      event,
      existing.filter((h) => h !== handler)
    );
  }

  emit(event: string, ...args: unknown[]): void {
    const handlers = this.handlers.get(event) || [];
    for (const handler of handlers) {
      try {
        handler(...args);
      } catch {
        // Swallow handler errors to prevent event propagation issues
      }
    }
  }

  removeAll(): void {
    this.handlers.clear();
  }
}

export const globalBus = new EventBus();
