const SESSION_KEY = "furniture-shop.sessionId";

export function getStoredSessionId(): string | null {
  try {
    return window.localStorage.getItem(SESSION_KEY);
  } catch {
    return null;
  }
}

export function storeSessionId(sessionId: string | null): void {
  try {
    if (sessionId) {
      window.localStorage.setItem(SESSION_KEY, sessionId);
    } else {
      window.localStorage.removeItem(SESSION_KEY);
    }
  } catch {
    // Storage unavailable (privacy mode, etc.); session stays in-memory.
  }
}