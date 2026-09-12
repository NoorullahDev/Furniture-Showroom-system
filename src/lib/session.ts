const SESSION_KEY = "furniture-shop.sessionId";

export function getStoredSessionId(): string | null {
  try {
    return window.sessionStorage.getItem(SESSION_KEY);
  } catch {
    return null;
  }
}

export function storeSessionId(sessionId: string | null): void {
  try {
    if (sessionId) {
      window.sessionStorage.setItem(SESSION_KEY, sessionId);
    } else {
      window.sessionStorage.removeItem(SESSION_KEY);
    }
  } catch {
    // Storage unavailable (privacy mode, etc.); session stays in-memory.
  }
}