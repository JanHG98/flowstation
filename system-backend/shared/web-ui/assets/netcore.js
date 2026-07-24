const DEFAULT_TIMEOUT_MS = 10000;

export class NetCoreApiClient {
  constructor({ baseUrl = "", timeoutMs = DEFAULT_TIMEOUT_MS } = {}) {
    this.baseUrl = baseUrl.replace(/\/$/, "");
    this.timeoutMs = timeoutMs;
  }

  async request(path, { method = "GET", body, headers = {}, signal } = {}) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(new DOMException("Request timeout", "TimeoutError")), this.timeoutMs);
    const relayAbort = () => controller.abort(signal.reason);
    signal?.addEventListener("abort", relayAbort, { once: true });
    try {
      const response = await fetch(`${this.baseUrl}${path}`, {
        method,
        signal: controller.signal,
        headers: { "Accept": "application/json", ...(body === undefined ? {} : { "Content-Type": "application/json" }), ...headers },
        body: body === undefined ? undefined : JSON.stringify(body),
      });
      const contentType = response.headers.get("content-type") || "";
      const payload = contentType.includes("application/json") ? await response.json() : await response.text();
      if (!response.ok) {
        const error = new Error(payload?.detail || payload?.title || `HTTP ${response.status}`);
        error.status = response.status;
        error.problem = payload;
        throw error;
      }
      return payload;
    } finally {
      clearTimeout(timeout);
      signal?.removeEventListener("abort", relayAbort);
    }
  }

  get(path, options) { return this.request(path, { ...options, method: "GET" }); }
  post(path, body, options) { return this.request(path, { ...options, method: "POST", body }); }
  put(path, body, options) { return this.request(path, { ...options, method: "PUT", body }); }
  delete(path, options) { return this.request(path, { ...options, method: "DELETE" }); }
}

export function statusBadge(status, text = status) {
  const span = document.createElement("span");
  span.className = "nc-status";
  span.dataset.status = String(status || "unknown").toLowerCase();
  span.textContent = text;
  return span;
}

export async function confirmAction({ title, message, confirmLabel = "Bestätigen", dangerous = false }) {
  const dialog = document.createElement("dialog");
  dialog.className = "nc-dialog";
  const heading = document.createElement("h2");
  heading.textContent = title;
  const paragraph = document.createElement("p");
  paragraph.textContent = message;
  const actions = document.createElement("div");
  actions.className = "nc-dialog-actions";
  const cancel = document.createElement("button");
  cancel.className = "nc-button";
  cancel.textContent = "Abbrechen";
  const confirm = document.createElement("button");
  confirm.className = "nc-button";
  if (dangerous) confirm.dataset.variant = "danger";
  confirm.textContent = confirmLabel;
  actions.append(cancel, confirm);
  dialog.append(heading, paragraph, actions);
  document.body.append(dialog);
  return new Promise(resolve => {
    const finish = value => { dialog.close(); dialog.remove(); resolve(value); };
    cancel.addEventListener("click", () => finish(false));
    confirm.addEventListener("click", () => finish(true));
    dialog.addEventListener("cancel", event => { event.preventDefault(); finish(false); });
    dialog.showModal();
  });
}

export function toast(message, level = "info", timeoutMs = 5000) {
  let host = document.querySelector(".nc-toast-host");
  if (!host) {
    host = document.createElement("div");
    host.className = "nc-toast-host";
    host.setAttribute("aria-live", "polite");
    document.body.append(host);
  }
  const item = document.createElement("div");
  item.className = "nc-toast";
  item.dataset.level = level;
  item.textContent = message;
  host.append(item);
  const timer = setTimeout(() => item.remove(), timeoutMs);
  item.addEventListener("click", () => { clearTimeout(timer); item.remove(); });
  return item;
}

export async function loadTranslations(url, fallback = {}) {
  try {
    const response = await fetch(url, { headers: { "Accept": "application/json" } });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return { ...fallback, ...await response.json() };
  } catch {
    return fallback;
  }
}

export function translate(dictionary, key, variables = {}) {
  const template = dictionary[key] ?? key;
  return Object.entries(variables).reduce((value, [name, replacement]) => value.replaceAll(`{${name}}`, String(replacement)), template);
}
