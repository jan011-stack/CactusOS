const iconPaths = {
  grid: '<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/>',
  users: '<path d="M16 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="10" cy="7" r="4"/><path d="M20 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75"/>',
  terminal: '<path d="m4 17 6-6-6-6M12 19h8"/>',
  activity: '<path d="M3 12h4l3-8 4 16 3-8h4"/>',
  layers: '<path d="m12 3 9 5-9 5-9-5 9-5Z"/><path d="m3 12 9 5 9-5M3 16l9 5 9-5"/>',
  sliders: '<path d="M4 21v-7m0-4V3m8 18v-9m0-4V3m8 18v-5m0-4V3M2 14h4m4-6h4m4 8h4"/>',
  refresh: '<path d="M20 7v5h-5M4 17v-5h5"/><path d="M5.5 9a7 7 0 0 1 11.6-2L20 12M4 12l2.9 5a7 7 0 0 0 11.6-2"/>',
  arrow: '<path d="M7 17 17 7M7 7h10v10"/>',
  send: '<path d="m22 2-7 20-4-9-9-4Z"/><path d="M22 2 11 13"/>',
  cpu: '<rect x="5" y="5" width="14" height="14" rx="2"/><path d="M9 9h6v6H9zM9 1v4m6-4v4M9 19v4m6-4v4M19 9h4m-4 6h4M1 9h4m-4 6h4"/>',
  shield: '<path d="M12 22s8-4 8-11V5l-8-3-8 3v6c0 7 8 11 8 11Z"/><path d="m9 12 2 2 4-4"/>',
  clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
  memory: '<rect x="4" y="4" width="16" height="16" rx="2"/><path d="M9 9h6v6H9zM9 1v3m6-3v3M9 20v3m6-3v3M1 9h3m-3 6h3m16-6h3m-3 6h3"/>',
  check: '<path d="m5 12 4 4L19 6"/>',
  plus: '<path d="M12 5v14m-7-7h14"/>',
  file: '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6M8 13h8m-8 4h8"/>',
  image: '<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5" cy="8.5" r="1.5"/><path d="m21 15-5-5L5 21"/>',
};

const icon = (name, size = 16) => `<svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${iconPaths[name] || iconPaths.grid}</svg>`;

const agents = [
  { name: 'Alex', initials: 'AX', role: 'Dowódca infrastruktury', status: 'Gotowy', ready: true },
  { name: 'Astyria', initials: 'AS', role: 'Diagnostyka i naprawa', status: 'Gotowa', ready: true },
  { name: 'Aurelius', initials: 'AU', role: 'Historia i integralność', status: 'Gotowy', ready: true },
  { name: 'SI_1', initials: 'SI', role: 'Kanał systemowy', status: 'Oczekuje', ready: false },
];

const initialEvents = [
  { time: '18:42:07', type: 'CORE', message: 'POST · walidacja siatki PI=4 zakończona', code: '0x00' },
  { time: '18:41:53', type: 'MEM', message: 'HyperAllocator · pule statyczne gotowe', code: '64 × 8 MB' },
  { time: '18:40:21', type: 'MODEL', message: 'Gemini 3.1 Flash-Lite · model główny', code: 'GEMINI' },
  { time: '18:39:04', type: 'GUARD', message: 'HyperShield · pierścień 0 zablokowany', code: 'RING-0' },
];

const pageTitles = {
  hyperspace: ['HIPERTERMINAL', 'Terminal binarnej hiperprzestrzeni', 'Kontener Codespaces'],
  dashboard: ['PULPIT', 'Pulpit systemu', 'Stan rdzenia, agentów i lokalnych usług w jednym miejscu.'],
  agents: ['AGENTY', 'Kanały agentów', 'Stan izolowanych kanałów komunikacyjnych HyperAgentBridge.'],
  terminal: ['TERMINAL', 'Konsola HyperOS', 'Podgląd poleceń i status kolejki terminala.'],
  chronolog: ['CHRONOLOG', 'Strumień zdarzeń', 'Ostatnie wpisy rejestru systemowego.'],
  models: ['MODELE', 'Magazyn modeli', 'Lokalne wagi multimodalne i status runtime’u.'],
  settings: ['USTAWIENIA', 'Konfiguracja panelu', 'Preferencje lokalnego podglądu CactusOS.'],
};

const pageContent = document.querySelector('#page-content');
const navButtons = [...document.querySelectorAll('.nav-item')];
const breadcrumb = document.querySelector('#breadcrumb-current');
const approvalDialog = document.querySelector('#approval-dialog');
const approvalConsent = document.querySelector('#approval-consent');
let currentPage = 'dashboard';
let events = [...initialEvents];
let commandHistory = [
  { text: 'Pisz swobodnie: odpowiada skonfigurowany model (Cactus SI, Gemini lub Ollama). @alex, @astyria, @aurelius, @si_1 wybierają rolę, clear czyści terminal.', kind: 'notice' },
];
let chatHistories = {};
let selectedChatAgent = 'alex';
let missionState = null;
let missionBusy = false;
let repoFiles = [];
let repoFilesLoaded = false;
let repoFilesAttempted = false;
let repoFilesLoading = false;
let repoFilesError = '';
let repoSelectedFiles = [];
let repoSelectedAgent = 'alex';
let repoChanges = null;
let repoChangeError = '';
let repoBusy = false;
let pendingMediaFiles = [];
let chatBusy = false;
let ollamaStatusBusy = false;
let lastChatProvider = null;
let toastTimer;
let clockTimer = null;
let terminalUpdateFrame = 0;
let pendingTerminalEntry = null;
const MAX_MEDIA_FILE_BYTES = 20 * 1024 * 1024;
const MAX_MEDIA_IMAGE_BYTES = 1024 * 1024;
const MAX_MEDIA_TOTAL_BYTES = 3 * 1024 * 1024;
const MAX_AGENT_HISTORY_MESSAGES = 12;

function metricCard(label, value, unit, note, iconName, color) {
  return `<article class="metric" style="--metric-color:${color}"><div class="metric-head"><span>${label}</span><span class="mini-icon">${icon(iconName, 14)}</span></div><div class="metric-value">${value}<small>${unit}</small></div><div class="metric-foot">${note}</div></article>`;
}

function agentRow(agent) {
  return `<div class="agent-row"><span class="agent-avatar">${agent.initials}</span><span class="agent-details"><span class="agent-name">${agent.name}</span><span class="agent-role">${agent.role}</span></span><span class="agent-status ${agent.ready ? 'ready' : ''}"><i></i>${agent.status}</span></div>`;
}

function eventRows(list) {
  return list.map((event) => `<tr><td>${event.time}</td><td><span class="event-mark ${event.type === 'GUARD' ? 'warn' : ''}"><i></i>${event.type}</span></td><td>${escapeHtml(event.message)}</td><td>${event.code}</td></tr>`).join('');
}

function terminalLines() {
  return commandHistory.map((entry) => `<div class="terminal-line ${entry.kind}"><span class="time-mark">${entry.time ? `[${entry.time}]` : ''}</span><span class="prompt-mark">${entry.prompt ? '›' : '·'}</span><span class="line-text">${escapeHtml(entry.prompt ? entry.prompt : entry.text)}</span></div>${entry.prompt ? `<div class="terminal-line ${entry.kind}"><span class="time-mark"></span><span class="prompt-mark">↳</span><span class="line-text">${escapeHtml(entry.text)}</span></div>` : ''}`).join('');
}

function terminalPanel() {
  const commandDisabled = chatBusy || userTerminalState.busy || Boolean(userTerminalState.proposal);
  return `<section class="panel terminal-panel">
    <div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('terminal')}</span>Terminal operatora</div><span class="panel-meta">GEMINI · OLLAMA FALLBACK · LLAMA VISION</span></div>
    <div class="panel-body">
      <div class="terminal-toolbar"><i class="window-dot"></i><i class="window-dot"></i><i class="window-dot"></i><span class="terminal-location">/hyperos/session · operator@cactus</span></div>
      <div class="terminal-output" id="terminal-output" aria-live="polite">${terminalLines()}</div>
      <form class="terminal-composer" id="command-form">
        <span class="composer-prompt">❯</span>
        <input class="terminal-input" id="command-input" maxlength="2000" placeholder="Rozmowa lub polecenie… (@alex, @astyria, @aurelius, @si_1)" autocomplete="off" aria-label="Polecenie terminala" ${commandDisabled ? 'disabled' : ''}>
        <button class="icon-button attach-button" type="button" id="attachment-trigger" aria-label="Dodaj obrazy lub wideo" title="Dodaj obrazy lub wideo" ${commandDisabled ? 'disabled' : ''}>${icon('file', 15)}</button>
        <input id="attachment-input" type="file" accept="image/*,video/*" multiple hidden>
        <button class="send-button" type="submit" aria-label="Wyślij polecenie" ${commandDisabled ? 'disabled' : ''}>${icon('send', 15)}</button>
      </form>
      <div class="attachment-preview" id="attachment-preview">${pendingMediaFiles.map((file, index) => `<span class="attachment-chip">${icon(file.type.startsWith('video/') ? 'image' : 'file', 13)} ${escapeHtml(file.name)}<button type="button" data-remove-attachment="${index}" aria-label="Usuń załącznik ${escapeHtml(file.name)}">×</button></span>`).join('')}</div>
      <div class="terminal-caption">Zwykły tekst obsługuje aktywny dostawca (Gemini/Ollama), a obrazy trafiają lokalnie do Llama Vision przez Ollama. Z wideo analizowane są maksymalnie dwie klatki. Nie wysyłaj sekretów. Polecenia systemowe wymagają zgody i działają jako użytkownik serwera.</div>
      <p id="user-command-error" class="root-error" role="alert"></p>
      <pre id="user-command-result" class="root-text"></pre>
    </div>
  </section>`;
}

function dashboardPage() {
  return `<section class="hero"><div class="hero-glow" aria-hidden="true"></div><div class="hero-main"><div class="eyebrow"><i class="live-dot"></i> SYSTEM ONLINE · HYPERSPACE DESKTOP</div><h1>Enter the <span class="gradient-text">Hyperspace</span></h1><p class="hero-copy">Jeden pulpit dla poleceń jądra, kanałów agentów i lokalnego modelu. Wybierz moduł albo napisz do terminala swobodnie.</p><div class="heading-actions"><button class="button primary" data-page-link="terminal">${icon('terminal', 14)} Pełny terminal ${icon('arrow', 12)}</button><button class="button" data-page-link="hyperspace">${icon('shield', 14)} Hiperterminal root ${icon('arrow', 12)}</button><button class="button" data-page-link="agents">${icon('users', 14)} Kanały agentów ${icon('arrow', 12)}</button><button class="button subtle" data-action="refresh">${icon('refresh', 14)} Odśwież widok</button></div></div><div class="hero-chips"><span class="chip ok">JĄDRO: PODGLĄD</span><span class="chip">GEMINI 3.1 FLASH-LITE</span><span class="chip">OLLAMA · FALLBACK</span><span class="chip accent">PROTOKÓŁ AGENTÓW · SPSC</span></div></section>
    <div class="metric-grid">${metricCard('Pamięć systemu', '30', 'GiB pool', 'STATIC · PODGLĄD', 'memory', '#a9dc72')}${metricCard('Kolejka agentów', '04', 'kanały', 'SPSC · READY', 'users', '#7fc8a1')}${metricCard('ChronoLog', '64', 'sloty', 'RING BUFFER · IDLE', 'activity', '#d6b56f')}${metricCard('Model lokalny', '7.8', 'GB', 'WAGI W CACHE', 'layers', '#95be79')}</div>
  <div class="dashboard-grid"><div class="stack">${terminalPanel()}<section class="panel"><div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('activity')}</span>Ostatnia aktywność</div><button class="button small subtle" data-page-link="chronolog">Pełny rejestr ${icon('arrow', 12)}</button></div><div class="panel-body" style="padding:0"><table class="activity-table"><thead><tr><th>Czas</th><th>Źródło</th><th>Zdarzenie</th><th>Wynik</th></tr></thead><tbody id="activity-rows">${eventRows(events.slice(0, 4))}</tbody></table></div></section></div><aside class="stack"><section class="panel"><div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('users')}</span>Agenci</div><span class="panel-meta">4 KANAŁY</span></div><div class="panel-body"><div class="agent-list">${agents.map(agentRow).join('')}</div><button class="button small subtle" style="width:100%;margin-top:14px" data-page-link="agents">Zarządzaj kanałami ${icon('arrow', 12)}</button></div></section><section class="panel"><div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('layers')}</span>Runtime modelu</div><span class="panel-meta">OLLAMA</span></div><div class="panel-body"><div class="runtime-row"><span class="runtime-logo">${icon('image', 17)}</span><span><span class="runtime-name">llama3.2-vision</span><span class="runtime-detail">Wagi Q4_K_M zapisane lokalnie. Działa na Ollama 0.6.8 (silnik Go); starsze wersje nie znają architektury mllama.</span><span class="runtime-tag">CACHE READY · INFERENCE OK</span></span></div></div></section><section class="panel"><div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('shield')}</span>Granica bezpieczeństwa</div><span class="panel-meta">RING 0</span></div><div class="panel-body"><div class="agent-row" style="padding-top:0;border:0"><span class="agent-avatar">${icon('shield', 16)}</span><span class="agent-details"><span class="agent-name">HyperShield</span><span class="agent-role">Prywatny stan jądra · blokada aktywna</span></span><span class="agent-status ready"><i></i>LOCKED</span></div></div></section></aside></div>
  <section class="modules"><div class="modules-head"><h2><i class="live-dot"></i> Moduły systemowe</h2><span class="panel-meta">CACTUSOS · NARZĘDZIA HYPEROS</span></div><div class="module-grid">${[
    ['terminal', 'terminal', 'Terminal', 'Rozmowa z modelem lokalnym i agentami', '03'],
    ['hyperspace', 'shield', 'Hiperterminal root', 'Polecenia root po lokalnym zatwierdzeniu', 'ROOT'],
    ['agents', 'users', 'Agenci', 'Alex, Astyria, Aurelius, SI_1', '02'],
    ['chronolog', 'activity', 'ChronoLog', 'Rejestr zdarzeń systemowych', '04'],
    ['models', 'layers', 'Modele', 'Wagi lokalne i runtime Ollama', '05'],
    ['settings', 'sliders', 'Ustawienia', 'Preferencje interfejsu', '06'],
  ].map(([page, symbol, name, description, code]) => `<button class="module-card" data-page-link="${page}"><span class="module-top"><span class="module-symbol">${icon(symbol, 16)}</span><span class="module-code">${code}</span></span><span class="module-name">${name}</span><span class="module-description">${description}</span></button>`).join('')}</div></section>`;
}

function agentsPage() {
  const missionAgents = Object.entries(AGENT_ROLES).map(([key, role]) => {
    const result = missionState?.agents[key];
    const status = result?.status || 'Oczekuje';
    const response = result?.reply || 'Odpowiedź pojawi się po wysłaniu misji.';
    return `<article class="mission-card"><div class="mission-card-head"><strong>${role[0]}</strong><span class="mission-agent-status ${result?.status === 'Błąd' ? 'error' : ''}">${escapeHtml(status)}</span></div><p class="mission-response" data-mission-agent="${key}">${escapeHtml(response)}</p></article>`;
  }).join('');
  const repoFileOptions = repoFiles.map(path => `<label class="repo-file-option"><input type="checkbox" data-repo-file="${escapeHtml(path)}" ${repoSelectedFiles.includes(path) ? 'checked' : ''} ${repoBusy || repoChanges ? 'disabled' : ''}><span>${escapeHtml(path)}</span></label>`).join('');
  const repoProposal = repoChanges ? `<div class="repo-proposal"><div class="mission-current"><strong>Proponowany diff · ${repoChanges.changes.length} plików</strong><p>Sprawdź każdą zmianę poniżej. Zapis dotyczy wyłącznie pokazanych plików.</p></div>${repoChanges.changes.map(change => `<article class="repo-diff"><strong>${escapeHtml(change.path)}</strong><pre>${escapeHtml(change.diff)}</pre></article>`).join('')}<label class="repo-consent"><input type="checkbox" id="repo-change-consent" ${repoBusy ? 'disabled' : ''}> Sprawdziłem diff i zatwierdzam zapis tych zmian w repozytorium.</label><div class="heading-actions"><button class="button primary" id="repo-change-approve" disabled>Zapisz zatwierdzone zmiany</button><button class="button" id="repo-change-reject" ${repoBusy ? 'disabled' : ''}>Odrzuć propozycję</button></div></div>` : '';
  return `<div class="page-heading"><div><div class="eyebrow">GEMINI 3.1 FLASH-LITE · OLLAMA FALLBACK</div><h1>Rozmowa z agentami</h1><p class="page-subtitle">Cztery role jednego modelu, każda z własną historią rozmowy.</p></div></div>
    <div class="agent-cards">${agents.map(agent => `<article class="agent-card"><span class="agent-avatar">${agent.initials}</span><div class="agent-name">${agent.name}</div><div class="agent-role">${agent.role}</div></article>`).join('')}</div>
    <section class="panel mission-panel"><div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('send')}</span>Misja dla wszystkich agentów</div><span class="panel-meta">4 NIEZALEŻNE ODPOWIEDZI</span></div><div class="panel-body"><form id="mission-form" class="mission-form"><label class="sr-only" for="mission-input">Treść wspólnej misji</label><textarea id="mission-input" maxlength="2000" placeholder="Opisz misję lub problem do przeanalizowania…" ${missionBusy ? 'disabled' : ''}></textarea><button class="button primary" type="submit" ${missionBusy ? 'disabled' : ''}>${icon('send', 14)} ${missionBusy ? 'Wysyłanie…' : 'Wyślij misję'}</button></form><p class="mission-note">Każdy agent odpowiada osobno przez skonfigurowany model. To analiza i rekomendacje — nic nie jest wykonywane automatycznie.</p>${missionState ? `<div class="mission-current"><strong>Ostatnia misja</strong><p>${escapeHtml(missionState.text)}</p><div class="mission-results" aria-live="polite">${missionAgents}</div></div>` : ''}</div></section>
    <section class="panel mission-panel"><div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('file')}</span>Propozycja zmian w repozytorium</div><span class="panel-meta">ZAPIS PO TWOJEJ ZGODZIE</span></div><div class="panel-body"><p class="mission-note repo-disclosure">Wybrane pliki zostaną wysłane do Gemini razem z opisem zadania. Agent może zaproponować edycję wybranych plików lub utworzenie nowych w istniejących katalogach. Nie ma dostępu do powłoki ani plików sekretów.</p><form id="repo-change-form"><div class="repo-controls"><label>Agent<select id="repo-agent" ${repoBusy || repoChanges ? 'disabled' : ''}>${Object.entries(AGENT_ROLES).map(([key, role]) => `<option value="${key}" ${key === repoSelectedAgent ? 'selected' : ''}>${role[0]}</option>`).join('')}</select></label><textarea id="repo-mission" aria-label="Zadanie dla agenta" maxlength="2000" placeholder="Opisz dokładnie, co ma zostać utworzone lub zmienione…" ${repoBusy || repoChanges ? 'disabled' : ''}></textarea></div><div class="repo-files-head"><strong>Pliki do udostępnienia agentowi</strong><button class="button small subtle" type="button" id="repo-files-refresh" ${repoBusy || repoChanges || repoFilesLoading ? 'disabled' : ''}>${repoFilesLoading ? 'Odświeżanie…' : 'Odśwież listę'}</button></div><div class="repo-file-list" id="repo-file-list">${repoFilesLoaded ? repoFileOptions || '<p class="mission-note">Nie znaleziono dostępnych plików tekstowych.</p>' : repoFilesLoading ? '<p class="mission-note">Wczytuję listę plików…</p>' : '<p class="mission-note">Lista plików jest niedostępna.</p>'}</div>${repoFilesError ? `<p class="repo-error" role="alert">${escapeHtml(repoFilesError)}</p>` : ''}${repoChangeError ? `<p class="repo-error" role="alert">${escapeHtml(repoChangeError)}</p>` : ''}<button class="button primary repo-propose-button" type="submit" ${repoBusy || repoChanges || !repoFilesLoaded ? 'disabled' : ''}>${repoBusy ? 'Przygotowuję / zapisuję…' : 'Przygotuj diff do sprawdzenia'}</button></form>${repoProposal}</div></section>
    <div class="agent-chat-picker"><label for="chat-agent">Rozmówca</label><select id="chat-agent" aria-label="Agent rozmowy" ${chatBusy ? 'disabled' : ''}>${Object.entries(AGENT_ROLES).map(([key, role]) => `<option value="${key}" ${key === selectedChatAgent ? 'selected' : ''}>${role[0]}</option>`).join('')}</select></div>
    ${terminalPanel()}`;
}

function terminalPage() {
  return `<div class="page-heading"><div><div class="eyebrow"><i class="live-dot"></i> SHELL · OLLAMA</div><h1>Konsola HyperOS</h1><p class="page-subtitle">Rozmowa z lokalnym modelem lub propozycja polecenia systemowego.</p></div></div>${terminalPanel()}<section class="panel" style="margin-top:14px"><div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('shield')}</span>Bezpieczne wykonanie</div><span class="panel-meta">USER · APPROVAL REQUIRED</span></div><div class="panel-body"><p class="runtime-detail" style="margin:0">Polecenia są wykonywane jako użytkownik serwera dopiero po pokazaniu pełnej treści i jej zatwierdzeniu. Do uprawnień root służy osobny Hiperterminal.</p></div></section>`;
}

function chronologPage() {
  return `<div class="page-heading"><div><div class="eyebrow"><i class="live-dot"></i> CHRONOLOG · 64 SLOTY</div><h1>Strumień zdarzeń</h1><p class="page-subtitle">Ostatnie wpisy diagnostyczne i systemowe w lokalnym podglądzie.</p></div><div class="heading-actions"><button class="button subtle" data-action="clear-log">${icon('refresh', 14)} Wyczyść podgląd</button></div></div><section class="panel"><div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('activity')}</span>Rejestr systemowy</div><span class="panel-meta" id="log-count">${events.length} WPISY</span></div><div class="log-list" id="full-log">${events.map((event) => `<div class="log-item"><span class="log-time">${event.time}</span><span class="log-message">${escapeHtml(event.message)}</span><span class="log-level ${event.type === 'GUARD' ? 'warn' : ''}">${event.type} · ${event.code}</span></div>`).join('')}</div></section>`;
}

function modelsPage() {
  return `<div class="page-heading"><div><div class="eyebrow"><i class="live-dot"></i> MAGAZYN LOKALNY</div><h1>Modele i runtime</h1><p class="page-subtitle">Oddzielamy stan zapisanych wag od możliwości ich uruchomienia.</p></div></div><div class="content-grid"><section class="panel"><div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('layers')}</span>Biblioteka modeli</div><span class="panel-meta">1 Wpis</span></div><div class="panel-body"><div class="runtime-row"><span class="runtime-logo">${icon('image', 17)}</span><span><span class="runtime-name">llama3.2-vision:latest</span><span class="runtime-detail">10.7B parametrów · Q4_K_M · obraz multimodalny</span><span class="runtime-tag">7.8 GB · WAGI ZAPISANE</span></span></div><div class="setting-row" style="margin-top:12px"><div><div class="setting-name">Model w cache</div><div class="setting-note">Manifest i warstwy lokalne zweryfikowane</div></div><span class="log-level">READY</span></div><div class="setting-row"><div><div class="setting-name">Inferencja</div><div class="setting-note">Ollama 0.6.8 z natywnym silnikiem Go; starsze wersje nie obsługują mllama</div></div><span class="log-level">READY</span></div></div></section><section class="panel"><div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('cpu')}</span>Backend</div><span class="panel-meta">CPU ONLY</span></div><div class="panel-body"><p class="runtime-detail" style="margin:0 0 13px">Terminal wysyła żądania do lokalnej Ollamy. Model wymaga około 10 GiB pamięci (z pliku wymiany) i ładuje się przy pierwszym zapytaniu około minuty.</p><span class="runtime-tag">OLLAMA · 127.0.0.1:11434</span></div></section></div>`;
}

function settingsPage() {
  return `<div class="page-heading"><div><div class="eyebrow"><i class="live-dot"></i> PREFERENCJE</div><h1>Ustawienia panelu</h1><p class="page-subtitle">Lokalne preferencje interfejsu, bez modyfikacji konfiguracji kernela.</p></div></div><div class="content-grid"><section class="panel"><div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('sliders')}</span>Interfejs</div><span class="panel-meta">LOCAL</span></div><div class="panel-body"><div class="setting-row"><div><div class="setting-name">Animacje interfejsu</div><div class="setting-note">Przejścia i delikatne wejście paneli</div></div><button class="toggle on" data-toggle aria-label="Przełącz animacje"></button></div><div class="setting-row"><div><div class="setting-name">Odświeżanie zegara</div><div class="setting-note">Lokalny czas przeglądarki</div></div><button class="toggle on" data-toggle aria-label="Przełącz zegar"></button></div><div class="setting-row"><div><div class="setting-name">Zapis historii demo</div><div class="setting-note">Historia pozostaje tylko w pamięci strony</div></div><button class="toggle" data-toggle aria-label="Przełącz zapis historii"></button></div></div></section><section class="panel"><div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('shield')}</span>O systemie</div></div><div class="panel-body"><div class="setting-row"><span class="setting-name">Build UI</span><span class="log-code">CACTUS-0.1.0</span></div><div class="setting-row"><span class="setting-name">Motyw</span><span class="log-code">BASTILLA / DARK</span></div><div class="setting-row"><span class="setting-name">Połączenie kernel</span><span class="log-level warn">NIEPODŁĄCZONE</span></div></div></section></div>`;
}

const renderers = { hyperspace: rootTerminalPage, dashboard: dashboardPage, agents: agentsPage, terminal: terminalPage, chronolog: chronologPage, models: modelsPage, settings: settingsPage };

function renderPage(name = currentPage) {
  currentPage = name in renderers ? name : 'dashboard';
  const [label] = pageTitles[currentPage];
  breadcrumb.textContent = label;
  document.title = `CactusOS · ${pageTitles[currentPage][1]}`;
  navButtons.forEach((button) => button.classList.toggle('active', button.dataset.page === currentPage));
  pageContent.innerHTML = renderers[currentPage]();
  bindPageActions();
  syncApprovalDialog();
}

function bindPageActions() {
  bindRootTerminal();
  bindUserTerminal();
  pageContent.querySelector('#mission-form')?.addEventListener('submit', onMissionSubmit);
  pageContent.querySelector('#repo-change-form')?.addEventListener('submit', onRepositoryChangeSubmit);
  pageContent.querySelector('#repo-files-refresh')?.addEventListener('click', loadRepositoryFiles);
  pageContent.querySelector('#repo-agent')?.addEventListener('change', event => {
    repoSelectedAgent = event.target.value;
  });
  pageContent.querySelectorAll('[data-repo-file]').forEach(input => input.addEventListener('change', () => {
    repoSelectedFiles = [...pageContent.querySelectorAll('[data-repo-file]:checked')].map(file => file.dataset.repoFile);
  }));
  pageContent.querySelector('#repo-change-consent')?.addEventListener('change', event => {
    const approve = pageContent.querySelector('#repo-change-approve');
    if (approve) approve.disabled = !event.target.checked || repoBusy;
  });
  pageContent.querySelector('#repo-change-approve')?.addEventListener('click', () => resolveRepositoryChange(true));
  pageContent.querySelector('#repo-change-reject')?.addEventListener('click', () => resolveRepositoryChange(false));
  pageContent.querySelector('#chat-agent')?.addEventListener('change', event => {
    selectedChatAgent = event.target.value;
  });
  pageContent.querySelectorAll('[data-page-link]').forEach((button) => button.addEventListener('click', () => renderPage(button.dataset.pageLink)));
  pageContent.querySelectorAll('[data-action="refresh"]').forEach((button) => button.addEventListener('click', () => { updateClock(); showToast('Widok odświeżony · dane demonstracyjne'); }));
  pageContent.querySelectorAll('[data-action="clear-log"]').forEach((button) => button.addEventListener('click', () => { events = []; renderPage('chronolog'); showToast('Wyczyszczono lokalny podgląd zdarzeń'); }));
  pageContent.querySelectorAll('[data-toggle]').forEach((button) => button.addEventListener('click', () => button.classList.toggle('on')));
  pageContent.querySelectorAll('.module-card').forEach((button) => button.addEventListener('click', () => renderPage(button.dataset.pageLink || 'dashboard')));
  const form = pageContent.querySelector('#command-form');
  if (form) form.addEventListener('submit', onCommandSubmit);
  const attachmentInput = pageContent.querySelector('#attachment-input');
  pageContent.querySelector('#attachment-trigger')?.addEventListener('click', () => attachmentInput?.click());
  attachmentInput?.addEventListener('change', () => {
    const newFiles = [...attachmentInput.files];
    if (newFiles.some(file => file.size > MAX_MEDIA_FILE_BYTES)) {
      showToast('Każdy załącznik musi mieć maksymalnie 20 MiB.');
      attachmentInput.value = '';
      return;
    }
    if (pendingMediaFiles.length + newFiles.length > 4) {
      showToast('Możesz dodać maksymalnie 4 pliki naraz.');
      attachmentInput.value = '';
      return;
    }
    pendingMediaFiles.push(...newFiles);
    renderPage(currentPage);
  });
  pageContent.querySelectorAll('[data-remove-attachment]').forEach(button => button.addEventListener('click', () => {
    pendingMediaFiles.splice(Number(button.dataset.removeAttachment), 1);
    renderPage(currentPage);
  }));
  const output = pageContent.querySelector('#terminal-output');
  if (output) output.scrollTop = output.scrollHeight;
  if (currentPage === 'agents' && !repoFilesAttempted && !repoFilesLoading) loadRepositoryFiles();
}

function updateTerminalOutput(entry) {
  const output = pageContent.querySelector('#terminal-output');
  if (!output) return;
  const line = output.lastElementChild?.querySelector('.line-text');
  if (line && entry) line.textContent = entry.text;
  if (output.scrollHeight - output.scrollTop - output.clientHeight < 48) {
    output.scrollTop = output.scrollHeight;
  }
}

function imageByteSize(dataUrl) {
  const encoded = dataUrl.slice(dataUrl.indexOf(',') + 1);
  const padding = encoded.endsWith('==') ? 2 : encoded.endsWith('=') ? 1 : 0;
  return Math.floor(encoded.length * 3 / 4) - padding;
}

async function drawJpeg(source, width, height) {
  const scale = Math.min(1, 1280 / Math.max(width, height));
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.round(width * scale));
  canvas.height = Math.max(1, Math.round(height * scale));
  const context = canvas.getContext('2d');
  if (!context) throw new Error('Przeglądarka nie może przygotować obrazu.');
  context.drawImage(source, 0, 0, canvas.width, canvas.height);
  const blob = await new Promise((resolve, reject) => {
    canvas.toBlob(result => {
      canvas.width = 0;
      canvas.height = 0;
      if (result) resolve(result);
      else reject(new Error('Nie udało się zakodować obrazu.'));
    }, 'image/jpeg', 0.72);
  });
  if (blob.size > MAX_MEDIA_IMAGE_BYTES) {
    throw new Error('Przygotowany obraz przekracza limit 1 MiB.');
  }
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result);
    reader.onerror = () => reject(new Error('Nie udało się odczytać przygotowanego obrazu.'));
    reader.readAsDataURL(blob);
  });
}

function waitForVideoEvent(video, eventName, errorMessage, action) {
  return new Promise((resolve, reject) => {
    const finish = (error) => {
      clearTimeout(timeout);
      video.removeEventListener(eventName, succeeded);
      video.removeEventListener('error', failed);
      if (error) reject(error);
      else resolve();
    };
    const succeeded = () => finish();
    const failed = () => finish(new Error(errorMessage));
    const timeout = setTimeout(() => finish(new Error(errorMessage)), 15000);
    video.addEventListener(eventName, succeeded, { once: true });
    video.addEventListener('error', failed, { once: true });
    try {
      action();
    } catch (error) {
      finish(error);
    }
  });
}

async function prepareMediaFiles(files) {
  const images = [];
  const labels = [];
  for (const file of files) {
    if (file.size > MAX_MEDIA_FILE_BYTES) throw new Error(`Plik ${file.name} przekracza limit 20 MiB.`);
    if (file.type.startsWith('image/')) {
      let bitmap;
      try {
        bitmap = await createImageBitmap(file, { resizeWidth: 1280, resizeQuality: 'low' });
        images.push(await drawJpeg(bitmap, bitmap.width, bitmap.height));
      } finally {
        bitmap?.close();
      }
      labels.push(file.name);
    } else if (file.type.startsWith('video/')) {
      const url = URL.createObjectURL(file);
      const video = document.createElement('video');
      video.preload = 'metadata';
      video.muted = true;
      video.src = url;
      try {
        await waitForVideoEvent(video, 'loadedmetadata',
          `Nie można odczytać wideo ${file.name}.`, () => video.load());
        if (!Number.isFinite(video.duration) || video.duration <= 0) {
          throw new Error(`Nieprawidłowy czas trwania wideo ${file.name}.`);
        }
        const frames = Math.min(2, 4 - images.length);
        if (frames < 1) throw new Error('Limit 4 obrazów lub klatek został osiągnięty.');
        for (let frame = 0; frame < frames; frame++) {
          const time = video.duration * (frame + 1) / (frames + 1);
          await waitForVideoEvent(video, 'seeked',
            `Nie można odczytać klatki z ${file.name}.`, () => { video.currentTime = time; });
          images.push(await drawJpeg(video, video.videoWidth, video.videoHeight));
        }
        labels.push(`${file.name} (${frames} ${frames === 1 ? 'klatka' : 'klatki'})`);
      } finally {
        video.pause();
        video.removeAttribute('src');
        video.load();
        URL.revokeObjectURL(url);
      }
    } else {
      throw new Error(`Nieobsługiwany format pliku: ${file.name}.`);
    }
    if (images.length > 4) throw new Error('Możesz wysłać maksymalnie 4 obrazy lub klatki wideo.');
  }
  if (images.reduce((total, image) => total + imageByteSize(image), 0) > MAX_MEDIA_TOTAL_BYTES) {
    throw new Error('Łączny rozmiar obrazów przekracza limit 3 MiB.');
  }
  return { images, labels };
}

function setMissionAgentResult(agentKey, status, reply) {
  missionState.agents[agentKey] = { status, reply };
  if (currentPage !== 'agents') return;
  const card = pageContent.querySelector(`[data-mission-agent="${agentKey}"]`);
  const statusNode = card?.closest('.mission-card')?.querySelector('.mission-agent-status');
  if (card) card.textContent = reply;
  if (statusNode) {
    statusNode.textContent = status;
    statusNode.classList.toggle('error', status === 'Błąd');
  }
}

async function loadRepositoryFiles() {
  if (repoFilesLoading) return;
  repoFilesAttempted = true;
  repoFilesLoading = true;
  repoFilesError = '';
  if (currentPage === 'agents') renderPage('agents');
  try {
    const response = await fetch('/api/repo/files', { cache: 'no-store' });
    const result = await response.json();
    if (!response.ok) throw new Error(result.error || `HTTP ${response.status}`);
    repoFiles = Array.isArray(result.files) ? result.files : [];
    repoFilesLoaded = true;
    repoSelectedFiles = repoSelectedFiles.filter(path => repoFiles.includes(path));
  } catch (error) {
    repoFilesError = `Nie udało się wczytać plików repozytorium: ${error.message}`;
  } finally {
    repoFilesLoading = false;
    if (currentPage === 'agents') renderPage('agents');
  }
}

async function onRepositoryChangeSubmit(event) {
  event.preventDefault();
  if (repoBusy || chatBusy) return;
  const mission = pageContent.querySelector('#repo-mission').value.trim();
  if (!mission) {
    showToast('Opisz zadanie dla agenta.');
    return;
  }
  if (!repoSelectedFiles.length) {
    showToast('Wybierz przynajmniej jeden plik jako kontekst dla agenta.');
    return;
  }
  repoBusy = true;
  chatBusy = true;
  repoChangeError = '';
  repoChanges = null;
  renderPage('agents');
  try {
    const response = await terminalFetch('/api/repo/propose', {
      agent: repoSelectedAgent,
      mission,
      paths: repoSelectedFiles,
    });
    const result = await response.json();
    if (!response.ok) throw new Error(`${result.error || 'Błąd serwera'} (HTTP ${response.status})`);
    repoChanges = result;
  } catch (error) {
    repoChangeError = `Nie udało się przygotować propozycji: ${error.message}`;
  } finally {
    repoBusy = false;
    chatBusy = false;
    if (currentPage === 'agents') renderPage('agents');
  }
}

async function resolveRepositoryChange(approved) {
  if (repoBusy || !repoChanges) return;
  if (approved && !pageContent.querySelector('#repo-change-consent')?.checked) return;
  repoBusy = true;
  chatBusy = true;
  repoChangeError = '';
  renderPage('agents');
  try {
    const response = await terminalFetch('/api/repo/resolve', {
      proposal_id: repoChanges.proposal_id,
      approved,
    });
    const result = await response.json();
    if (!response.ok) throw new Error(`${result.error || 'Błąd serwera'} (HTTP ${response.status})`);
    if (approved) {
      repoChangeError = `Zapisano pliki: ${result.applied.join(', ')}`;
    } else {
      repoChangeError = 'Propozycja została odrzucona; pliki nie zostały zmienione.';
    }
    repoChanges = null;
  } catch (error) {
    repoChangeError = `Nie udało się zatwierdzić propozycji: ${error.message}`;
    if (responseInvalidatesProposal(error)) repoChanges = null;
  } finally {
    repoBusy = false;
    chatBusy = false;
    if (currentPage === 'agents') renderPage('agents');
    if (approved) loadRepositoryFiles();
  }
}

function responseInvalidatesProposal(error) {
  return /HTTP (400|409)/.test(error.message);
}

function syncApprovalDialog() {
  const pending = userTerminalState.proposal
    ? { type: 'user', ...userTerminalState }
    : rootTerminalState.proposal
      ? { type: 'root', ...rootTerminalState }
      : null;
  const key = pending ? `${pending.type}:${pending.proposal.proposal_id}` : '';
  if (key !== syncApprovalDialog.currentKey) {
    approvalConsent.checked = false;
    syncApprovalDialog.currentKey = key;
  }
  if (!pending) {
    if (approvalDialog.open) approvalDialog.close();
    return;
  }

  document.querySelector('#approval-title').textContent = pending.type === 'root'
    ? 'Zatwierdzenie wykonania jako root'
    : 'Zatwierdzenie polecenia';
  document.querySelector('#approval-scope').textContent = pending.type === 'root'
    ? 'UID 0 · ROOT' : 'ZWYKŁY UŻYTKOWNIK';
  document.querySelector('#approval-command').textContent = pending.proposal.command || '';
  document.querySelector('#approval-error').textContent = pending.error
    || (pending.busy ? 'Przetwarzanie decyzji…' : '');
  const allowed = pending.executionAllowed;
  document.querySelector('#approval-approve').textContent = pending.busy
    ? 'Przetwarzanie…' : pending.type === 'root' ? 'Wykonaj jako root' : 'Wykonaj';
  document.querySelector('#approval-approve').disabled = pending.busy
    || !allowed || !approvalConsent.checked;
  document.querySelector('#approval-reject').disabled = pending.busy;
  approvalConsent.disabled = pending.busy;
  if (!approvalDialog.open) approvalDialog.showModal();
}

approvalConsent.addEventListener('change', () => {
  syncApprovalDialog();
});
document.querySelector('#approval-approve').addEventListener('click', () => {
  if (userTerminalState.proposal) resolveUserProposal(true);
  else if (rootTerminalState.proposal) resolveRootProposal(true);
});
document.querySelector('#approval-reject').addEventListener('click', () => {
  if (userTerminalState.proposal) resolveUserProposal(false);
  else if (rootTerminalState.proposal) resolveRootProposal(false);
});
approvalDialog.addEventListener('cancel', event => event.preventDefault());

async function onMissionSubmit(event) {
  event.preventDefault();
  if (missionBusy || chatBusy) return;
  const input = pageContent.querySelector('#mission-input');
  const mission = input.value.trim();
  if (!mission) return;

  missionBusy = true;
  chatBusy = true;
  missionState = {
    text: mission,
    agents: Object.fromEntries(Object.keys(AGENT_ROLES).map(key => [key, { status: 'W toku', reply: 'Agent analizuje misję…' }])),
  };
  const time = new Date().toLocaleTimeString('pl-PL', { hour12: false });
  events.unshift({ time, type: 'MISSION', message: `Wysłano misję do 4 agentów · ${mission.slice(0, 60)}`, code: 'AI' });
  events = events.slice(0, 16);
  renderPage('agents');

  const tasks = Object.entries(AGENT_ROLES).map(async ([agentKey]) => {
    const history = chatHistories[agentKey] || (chatHistories[agentKey] = []);
    const userMessage = { role: 'user', content: mission };
    history.push(userMessage);
    try {
      const reply = await streamChat(buildMissionMessages(history.slice(0, -1), agentKey, mission), partial => {
        setMissionAgentResult(agentKey, 'W toku', partial);
      }, undefined, provider => {
        if (provider) lastChatProvider = provider;
      });
      history.push({ role: 'assistant', content: reply });
      if (history.length > MAX_AGENT_HISTORY_MESSAGES) {
        history.splice(0, history.length - MAX_AGENT_HISTORY_MESSAGES);
      }
      setMissionAgentResult(agentKey, 'Gotowe', reply || '(model nie zwrócił odpowiedzi)');
    } catch (error) {
      if (history.at(-1) === userMessage) history.pop();
      const unreachable = error instanceof TypeError;
      const message = unreachable
        ? 'Nie można połączyć się z serwerem CactusOS. Otwórz stronę na porcie 4180 i sprawdź, czy backend działa.'
        : `Błąd modelu: ${error.message}`;
      setMissionAgentResult(agentKey, 'Błąd', message);
    }
  });
  await Promise.all(tasks);
  missionBusy = false;
  chatBusy = false;
  if (currentPage === 'agents') renderPage('agents');
  refreshOllamaStatus();
}

async function onCommandSubmit(event) {
  event.preventDefault();
  if (chatBusy) return;
  const input = pageContent.querySelector('#command-input');
  const command = input.value.trim();
  const files = pendingMediaFiles.slice();
  if (!command && !files.length) return;
  input.value = '';

  if (command.toLowerCase() === 'clear' && !files.length) {
    commandHistory = [];
    chatHistories = {};
    renderPage(currentPage);
    return;
  }

  const parsed = parseCommand(command);
  const selectedRole = currentPage === 'agents' && !parsed.agent ? AGENT_ROLES[selectedChatAgent] : null;
  const agent = parsed.agent || (selectedRole && selectedRole[0]);
  const text = parsed.text;
  const system = selectedRole
    ? `${BASE_PROMPT} Odpowiadasz jako ${selectedRole[0]}, rola: ${selectedRole[1]}.`
    : parsed.system;
  if (!files.length && isCommandExecutionRequest(text)) {
    if (rootTerminalState.busy || rootTerminalState.proposal
        || userTerminalState.busy || userTerminalState.proposal) {
      input.value = command;
      showToast('Najpierw zakończ lub odrzuć oczekującą propozycję terminala.');
      return;
    }
    const time = new Date().toLocaleTimeString('pl-PL', { hour12: false });
    const entry = { prompt: command, text: 'Przygotowuję propozycję…', kind: 'response', time };
    commandHistory.push(entry);
    if (commandHistory.length > 18) commandHistory = commandHistory.slice(-18);
    events.unshift({ time, type: 'CLI', message: `Propozycja polecenia · ${command.slice(0, 60)}`, code: 'USER' });
    events = events.slice(0, 16);
    renderPage(currentPage);
    submitUserCommand(text, entry);
    return;
  }
  const channel = agent || 'default';
  const chatMessages = chatHistories[channel] || (chatHistories[channel] = []);
  const time = new Date().toLocaleTimeString('pl-PL', { hour12: false });
  const entry = { prompt: agent ? `@${agent} ${text}` : command || 'Załączniki', text: 'Przygotowuję załączniki…', kind: 'response', time };
  commandHistory.push(entry);
  if (commandHistory.length > 18) commandHistory = commandHistory.slice(-18);
  events.unshift({ time, type: 'CLI', message: `Wpis terminala${agent ? ' · ' + agent : ''} · ${(command || 'załączniki').slice(0, 60)}`, code: 'AI' });
  events = events.slice(0, 16);

  pendingMediaFiles = [];
  chatBusy = true;
  renderPage(currentPage);
  let preparingMedia = true;
  let userMessageAdded = false;
  try {
    const media = await prepareMediaFiles(files);
    const content = [text || 'Opisz załączone obrazy.', media.labels.length
      ? `Załączniki: ${media.labels.join(', ')}` : ''].filter(Boolean).join('\n\n');
    entry.prompt = `${agent ? `@${agent} ` : ''}${text || 'Załączniki'}${media.labels.length ? ` [${media.labels.join(', ')}]` : ''}`;
    chatMessages.push({ role: 'user', content, ...(media.images.length ? { images: media.images } : {}) });
    userMessageAdded = true;
    let excessImages = chatMessages.reduce((count, message) => count + (message.images?.length || 0), 0) - 4;
    let excessBytes = chatMessages.reduce(
      (total, message) => total + (message.images || []).reduce((size, image) => size + imageByteSize(image), 0),
      0,
    ) - MAX_MEDIA_TOTAL_BYTES;
    for (const message of chatMessages) {
      while ((excessImages > 0 || excessBytes > 0) && message.images?.length) {
        const removed = message.images.shift();
        excessImages--;
        excessBytes -= imageByteSize(removed);
      }
      if (message.images && !message.images.length) delete message.images;
    }
    updateTerminalOutput(entry);
    preparingMedia = false;
    const reply = await streamChat(buildMessages(chatMessages, system), (partial) => {
      entry.text = partial;
      pendingTerminalEntry = entry;
      if (!terminalUpdateFrame) {
        terminalUpdateFrame = requestAnimationFrame(() => {
          terminalUpdateFrame = 0;
          const latestEntry = pendingTerminalEntry;
          pendingTerminalEntry = null;
          updateTerminalOutput(latestEntry);
        });
      }
    }, undefined, provider => {
      const pill = document.querySelector('#ollama-pill');
      if (pill && provider) {
        lastChatProvider = provider;
        pill.classList.add('online');
        const label = provider === 'Gemini' ? 'GEMINI 3.1 FLASH-LITE'
          : provider.startsWith('Cactus SI') ? 'CACTUS SI'
            : provider.toUpperCase();
        pill.innerHTML = `<i></i> ${label}`;
      }
    });
    entry.text = reply || '(model nie zwr\u00f3ci\u0142 odpowiedzi)';
    chatMessages.push({ role: 'assistant', content: reply });
    trimChatHistory(chatMessages);
  } catch (error) {
    if (userMessageAdded && chatMessages.at(-1)?.role === 'user') chatMessages.pop();
    if (preparingMedia) pendingMediaFiles = files;
    entry.kind = 'notice';
    const unreachable = error instanceof TypeError;
    entry.text = preparingMedia
      ? error.message
      : unreachable
      ? 'Nie można połączyć się z serwerem CactusOS. Otwórz stronę na porcie 4180 i sprawdź, czy backend działa.'
      : `Błąd modelu: ${error.message}`;
  } finally {
    chatBusy = false;
    renderPage(currentPage);
    pageContent.querySelector('#command-input')?.focus();
    refreshOllamaStatus();
  }
}

async function refreshOllamaStatus() {
  if (ollamaStatusBusy || document.hidden) return;
  ollamaStatusBusy = true;
  try {
    const status = await modelStatus();
    const pill = document.querySelector('#ollama-pill');
    if (pill) {
      const online = status?.available === true;
      const activeProvider = lastChatProvider || status?.primary_provider;
      pill.classList.toggle('online', online);
      pill.innerHTML = `<i></i> ${online
        ? activeProvider?.startsWith('Ollama') ? activeProvider.toUpperCase()
          : activeProvider?.startsWith('Cactus SI') ? 'CACTUS SI · KLUCZ USTAWIONY'
            : activeProvider === 'Gemini' ? 'GEMINI · KLUCZ USTAWIONY' : 'OLLAMA'
        : 'MODELE · BRAK POŁĄCZENIA'}`;
    }
    const node = document.querySelector('#node-ollama');
    if (node) node.textContent = status?.available
      ? lastChatProvider?.startsWith('Ollama') ? lastChatProvider : status.primary_provider
      : 'offline';
  } finally {
    ollamaStatusBusy = false;
  }
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[char]);
}

function updateClock() {
  const clock = document.querySelector('#clock');
  if (clock) clock.textContent = new Date().toLocaleTimeString('pl-PL', { hour12: false });
}

function syncClockTimer() {
  clearInterval(clockTimer);
  clockTimer = null;
  if (!document.hidden) {
    updateClock();
    clockTimer = setInterval(updateClock, 1000);
  }
}

function showToast(message) {
  document.querySelector('.toast')?.remove();
  const toast = document.createElement('div');
  toast.className = 'toast';
  toast.setAttribute('role', 'status');
  toast.textContent = message;
  document.body.append(toast);
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toast.remove(), 2400);
}

navButtons.forEach((button) => button.addEventListener('click', () => renderPage(button.dataset.page)));
document.querySelector('#refresh-button').addEventListener('click', () => { updateClock(); showToast('Zegar zsynchronizowany lokalnie'); });
document.querySelectorAll('[data-icon]').forEach((node) => { node.innerHTML = icon(node.dataset.icon, 16); });
syncClockTimer();
renderPage('dashboard');
refreshOllamaStatus();
setInterval(refreshOllamaStatus, 30000);
document.addEventListener('visibilitychange', () => {
  syncClockTimer();
  refreshOllamaStatus();
});