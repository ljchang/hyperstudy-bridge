/**
 * Toast notification store using Svelte 5 runes.
 *
 * Toast shape:
 *   { id, type, title, message, persistent, actions, autoDismissMs }
 *
 * Types: 'success' | 'info' | 'warning' | 'error' | 'update'
 * Actions: [{ label: string, onClick: (toastId) => void }]
 */

let toasts = $state([]);
let nextId = 0;

export function getToasts() {
  return toasts;
}

/**
 * Add a toast notification.
 * @param {{ type: string, title: string, message?: string, persistent?: boolean, actions?: Array, autoDismissMs?: number }} toast
 * @returns {number} The toast ID (for later removal or update)
 */
export function addToast(toast) {
  const id = nextId++;
  const newToast = { id, ...toast };
  toasts = [...toasts, newToast];

  if (!toast.persistent) {
    setTimeout(() => removeToast(id), toast.autoDismissMs || 4000);
  }
  return id;
}

export function removeToast(id) {
  toasts = toasts.filter(t => t.id !== id);
}

export function updateToast(id, partial) {
  toasts = toasts.map(t => (t.id === id ? { ...t, ...partial } : t));
}
