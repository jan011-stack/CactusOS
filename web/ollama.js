const CHAT_URL = '/api/ollama';
const MAX_HISTORY_MESSAGES = 12;

const BASE_PROMPT = 'Jesteś asystentem terminala CactusOS (HyperOS). Rozmawiasz swobodnie z operatorem Mateuszem Mitkiem, '
  + 'po polsku, zwięźle (2-4 zdania). Nie masz samodzielnego dostępu do powłoki, plików ani jądra, '
  + 'więc nigdy nie twierdź, że coś wykonałeś. Polecenia systemowe są przygotowywane oddzielnie i wymagają zatwierdzenia. '
  + 'Nie zmyślaj faktów o systemie i nie ujawniaj tokenów.';

function buildMessages(history, system) {
  const now = new Date();
  const zone = Intl.DateTimeFormat().resolvedOptions().timeZone;
  const clock = ` Aktualny czas przeglądarki: ${now.toLocaleString('pl-PL')} (${zone}); ISO: ${now.toISOString()}. Gdy pytanie dotyczy aktualnej godziny, użyj tego odczytu i podaj strefę.`;
  return [{ role: 'system', content: system + clock }, ...history.slice(-MAX_HISTORY_MESSAGES)];
}

function trimChatHistory(history) {
  if (history.length > MAX_HISTORY_MESSAGES) {
    history.splice(0, history.length - MAX_HISTORY_MESSAGES);
  }
}

function isPingExecutionRequest(text) {
  const normalized = text.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase().trim();
  if (!/\bping\b/.test(normalized)) return false;
  if (/\b(co to|czym jest|co robi|wyjasnij|jak dziala|do czego sluzy)\b/.test(normalized)) return false;
  const explicitAction = /\b(wyslij\w*|wykonaj\w*|uruchom\w*|sprawdz\w*|przetestuj\w*|zrob\w*|kaze\w*|kaz\w*|poprosze|polecam)\b/.test(normalized);
  return /^ping(?:\s|$)/.test(normalized) || explicitAction;
}

function isCommandExecutionRequest(text) {
  const normalized = text.normalize('NFD').replace(/[\u0300-\u036f]/g, '').replace(/ł/g, 'l').toLowerCase().trim();
  if (/\b(co to|czym jest|co robi|wyjasnij|jak dziala|jak polaczyc|what is|what does|explain|how does|how to)\b/.test(normalized)) {
    return false;
  }
  return /^(ls|pwd|whoami|id|date|uname|cat|head|tail|grep|find|git|cargo|python3?|node|npm|npx|pytest|make|echo|printf|touch|mkdir|rm|cp|mv|chmod|chown|ps|df|du|free|env|printenv|which|whereis|systemctl|journalctl|ping|curl|wget|apt|pip|rustc|rustup|gh)(?:\s|$)/.test(normalized)
    || /\b(uruchom\w*|wykonaj\w*|sprawdz\w*|przetestuj\w*|zainstaluj\w*|napraw\w*|zmien\w*|ulepsz\w*|utworz\w*|usun\w*|kasuj\w*|zbuduj\w*|skompiluj\w*|zapisz\w*|skopiuj\w*|polacz\w*|dolacz\w*|run|execute|check|test|install|fix|update|create|delete|build|compile|write|copy|connect\w*)\b/.test(normalized)
    || isPingExecutionRequest(normalized);
}

async function modelStatus() {
  try {
    const response = await fetch('/api/model/status', { signal: AbortSignal.timeout(5000) });
    if (!response.ok) return null;
    const status = await response.json();
    if (status.primary_provider === 'Gemini' || status.primary_provider === 'Cactus SI') {
      return { ...status, available: true };
    }
    const ollama = await fetch(`${CHAT_URL}/version`, { signal: AbortSignal.timeout(5000) });
    return { ...status, available: ollama.ok };
  } catch {
    return null;
  }
}

async function terminalFetch(path, payload, signal) {
  const sessionMeta = document.querySelector('meta[name="terminal-csrf"]');
  if (!sessionMeta) throw new Error('Otwórz terminal przez serwer CactusOS na porcie 4180, nie statyczny podgląd 4173.');
  const send = () => fetch(path, {
    method: 'POST', signal,
    headers: { 'Content-Type': 'application/json', 'X-Terminal-CSRF': sessionMeta.content },
    body: JSON.stringify(payload),
  });
  let response = await send();
  if (response.status === 403) {
    const failure = await response.clone().json().catch(() => ({}));
    if (failure.code === 'csrf') {
      const session = await fetch('/api/session', { cache: 'no-store', signal });
      if (!session.ok) throw new Error('Nie udało się odnowić sesji terminala. Odśwież stronę.');
      const credentials = await session.json();
      if (typeof credentials.csrf !== 'string' || !credentials.csrf) throw new Error('Nieprawidłowa sesja terminala.');
      sessionMeta.content = credentials.csrf;
      response = await send();
    } else if (failure.code === 'origin' || failure.code === 'host') {
      throw new Error('Serwer odrzucił adres strony (HTTP 403). Otwórz prywatny port 4180 CactusOS; to nie jest błąd Ollamy.');
    }
  }
  return response;
}

/** Streams a chat reply, calling onToken with the text so far; resolves to the full reply. */
async function streamChat(messages, onToken, signal, onProvider = () => {}) {
  const response = await terminalFetch(`${CHAT_URL}/chat`, {
      messages,
      stream: true,
  }, signal);
  if (!response.ok) {
    throw new Error(`Model zwrócił HTTP ${response.status}: ${(await response.text()).slice(0, 160)}`);
  }
  onProvider(response.headers.get('X-Cactus-Provider'));

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffered = '';
  let reply = '';
  const consume = (line) => {
    if (!line.trim()) return;
    const chunk = JSON.parse(line);
    if (chunk.error) throw new Error(chunk.error);
    if (chunk.message && chunk.message.content) {
      reply += chunk.message.content;
      onToken(reply);
    }
  };
  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    buffered += decoder.decode(value, { stream: true });
    const lines = buffered.split('\n');
    buffered = lines.pop();
    lines.forEach(consume);
  }
  consume(buffered + decoder.decode());
  return reply.trim();
}

if (typeof module !== 'undefined') {
  module.exports = { BASE_PROMPT, buildMessages, trimChatHistory, isPingExecutionRequest, isCommandExecutionRequest, streamChat, modelStatus, terminalFetch };
}
