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

const initialEvents = [
  { time: '18:42:07', type: 'CORE', message: 'POST · walidacja siatki PI=4 zakończona', code: '0x00' },
  { time: '18:41:53', type: 'MEM', message: 'HyperAllocator · pule statyczne gotowe', code: '64 × 8 MB' },
  { time: '18:40:21', type: 'MODEL', message: 'Gemini 3.1 Flash-Lite · model główny', code: 'GEMINI' },
  { time: '18:39:04', type: 'GUARD', message: 'HyperShield · pierścień 0 zablokowany', code: 'RING-0' },
];

const pageTitles = {
  hyperspace: ['HIPERTERMINAL', 'Terminal binarnej hiperprzestrzeni', 'Kontener Codespaces'],
  dashboard: ['PULPIT', 'Pulpit systemu', 'Stan rdzenia i lokalnych usług w jednym miejscu.'],
  repo: ['REPOZYTORIUM', 'Zmiany w repozytorium', 'Propozycje zmian zapisywane wyłącznie po Twojej zgodzie.'],
  terminal: ['TERMINAL', 'Konsola HyperOS', 'Podgląd poleceń i status kolejki terminala.'],
  chronolog: ['CHRONOLOG', 'Strumień zdarzeń', 'Ostatnie wpisy rejestru systemowego.'],
  models: ['MODELE', 'Magazyn modeli', 'Lokalne wagi multimodalne i status runtime’u.'],
  settings: ['USTAWIENIA', 'Konfiguracja panelu', 'Preferencje lokalnego podglądu CactusOS.'],
};

const pageContent = document.querySelector('#page-content');
const navButtons = [...document.querySelectorAll('.nav-item')];
const breadcrumb = document.querySelector('#breadcrumb-current');
let currentPage = 'dashboard';
let events = [...initialEvents];
let commandHistory = [
  { text: 'Pisz swobodnie: odpowiada skonfigurowany model (Cactus SI, Gemini lub Ollama). clear czyści terminal.', kind: 'notice' },
];
let chatMessages = [];
let repoFiles = [];
let repoFilesLoaded = false;
let repoFilesAttempted = false;
let repoFilesLoading = false;
let repoFilesError = '';
let repoSelectedFiles = [];
let repoQueue = [];
let repoQueueLoading = false;
let repoQueueError = '';
let repoQueueTimer = null;
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

function metricCard(label, value, unit, note, iconName, color) {
  return `<article class="metric" style="--metric-color:${color}"><div class="metric-head"><span>${label}</span><span class="mini-icon">${icon(iconName, 14)}</span></div><div class="metric-value">${value}<small>${unit}</small></div><div class="metric-foot">${note}</div></article>`;
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
        <input class="terminal-input" id="command-input" maxlength="2000" placeholder="Rozmowa lub polecenie…" autocomplete="off" aria-label="Polecenie terminala" ${commandDisabled ? 'disabled' : ''}>
        <button class="icon-button attach-button" type="button" id="attachment-trigger" aria-label="Dodaj obrazy lub wideo" title="Dodaj obrazy lub wideo" ${commandDisabled ? 'disabled' : ''}>${icon('file', 15)}</button>
        <input id="attachment-input" type="file" accept="image/*,video/*" multiple hidden>
        <button class="send-button" type="submit" aria-label="Wyślij polecenie" ${commandDisabled ? 'disabled' : ''}>${icon('send', 15)}</button>
      </form>
      <section id="user-approval" class="approval-inline" aria-live="polite" hidden>
        <strong>Wymagana zgoda · zwykły użytkownik</strong>
        <p>Sprawdź dokładne polecenie przed uruchomieniem:</p>
        <pre id="user-approval-command" class="root-text"></pre>
        <p id="user-approval-error" class="root-error" role="status"></p>
        <label class="root-consent"><input type="checkbox" id="user-approval-consent"> Zatwierdzam dokładnie to polecenie.</label>
        <div class="heading-actions">
          <button id="user-approval-approve" class="button primary" type="button" disabled>Wykonaj</button>
          <button id="user-approval-reject" class="button" type="button">Odrzuć</button>
        </div>
      </section>
      <div class="attachment-preview" id="attachment-preview">${pendingMediaFiles.map((file, index) => `<span class="attachment-chip">${icon(file.type.startsWith('video/') ? 'image' : 'file', 13)} ${escapeHtml(file.name)}<button type="button" data-remove-attachment="${index}" aria-label="Usuń załącznik ${escapeHtml(file.name)}">×</button></span>`).join('')}</div>
      <div class="terminal-caption">Zwykły tekst obsługuje aktywny dostawca (Gemini/Ollama), a obrazy trafiają lokalnie do Llama Vision przez Ollama. Z wideo analizowane są maksymalnie dwie klatki. Nie wysyłaj sekretów. Polecenia systemowe wymagają zgody i działają jako użytkownik serwera.</div>
      <p id="user-command-error" class="root-error" role="alert"></p>
      <pre id="user-command-result" class="root-text"></pre>
    </div>
  </section>`;
}

function dashboardPage() {
  return `<section class="hero"><div class="hero-glow" aria-hidden="true"></div><div class="hero-main"><div class="eyebrow"><i class="live-dot"></i> SYSTEM ONLINE · HYPERSPACE DESKTOP</div><h1>Enter the <span class="gradient-text">Hyperspace</span></h1><p class="hero-copy">Jeden pulpit dla poleceń jądra, i lokalnego modelu. Wybierz moduł albo napisz do terminala swobodnie.</p><div class="heading-actions"><button class="button primary" data-page-link="terminal">${icon('terminal', 14)} Pełny terminal ${icon('arrow', 12)}</button><button class="button" data-page-link="hyperspace">${icon('shield', 14)} Hiperterminal root ${icon('arrow', 12)}</button><button class="button" data-page-link="repo">${icon('file', 14)} Zmiany w repozytorium ${icon('arrow', 12)}</button><button class="button subtle" data-action="refresh">${icon('refresh', 14)} Odśwież widok</button></div></div><div class="hero-chips"><span class="chip ok">JĄDRO: PODGLĄD</span><span class="chip">GEMINI 3.1 FLASH-LITE</span><span class="chip">OLLAMA · FALLBACK</span><span class="chip accent">SPSC</span></div></section>
    <div class="metric-grid">${metricCard('Pamięć systemu', '30', 'GiB pool', 'STATIC · PODGLĄD', 'memory', '#a9dc72')}${metricCard('ChronoLog', '64', 'sloty', 'RING BUFFER · IDLE', 'activity', '#d6b56f')}${metricCard('Model lokalny', '7.8', 'GB', 'WAGI W CACHE', 'layers', '#95be79')}</div>
  <div class="dashboard-grid"><div class="stack">${terminalPanel()}<section class="panel"><div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('activity')}</span>Ostatnia aktywność</div><button class="button small subtle" data-page-link="chronolog">Pełny rejestr ${icon('arrow', 12)}</button></div><div class="panel-body" style="padding:0"><table class="activity-table"><thead><tr><th>Czas</th><th>Źródło</th><th>Zdarzenie</th><th>Wynik</th></tr></thead><tbody id="activity-rows">${eventRows(events.slice(0, 4))}</tbody></table></div></section></div><aside class="stack"><section class="panel"><div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('layers')}</span>Runtime modelu</div><span class="panel-meta">OLLAMA</span></div><div class="panel-body"><div class="runtime-row"><span class="runtime-logo">${icon('image', 17)}</span><span><span class="runtime-name">llama3.2-vision</span><span class="runtime-detail">Wagi Q4_K_M zapisane lokalnie. Działa na Ollama 0.6.8 (silnik Go); starsze wersje nie znają architektury mllama.</span><span class="runtime-tag">CACHE READY · INFERENCE OK</span></span></div></div></section><section class="panel"><div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('shield')}</span>Granica bezpieczeństwa</div><span class="panel-meta">RING 0</span></div><div class="panel-body"><div class="status-row" style="padding-top:0;border:0"><span class="status-avatar">${icon('shield', 16)}</span><span class="status-details"><span class="status-name">HyperShield</span><span class="status-role">Prywatny stan jądra · blokada aktywna</span></span><span class="status-status ready"><i></i>LOCKED</span></div></div></section></aside></div>
  <section class="modules"><div class="modules-head"><h2><i class="live-dot"></i> Moduły systemowe</h2><span class="panel-meta">CACTUSOS · NARZĘDZIA HYPEROS</span></div><div class="module-grid">${[
    ['terminal', 'terminal', 'Terminal', 'Rozmowa z modelem lokalnym', '03'],
    ['hyperspace', 'shield', 'Hiperterminal root', 'Polecenia root po lokalnym zatwierdzeniu', 'ROOT'],
    ['repo', 'file', 'Repozytorium', 'Propozycje zmian z zatwierdzeniem diffu', '02'],
    ['chronolog', 'activity', 'ChronoLog', 'Rejestr zdarzeń systemowych', '04'],
    ['models', 'layers', 'Modele', 'Wagi lokalne i runtime Ollama', '05'],
    ['settings', 'sliders', 'Ustawienia', 'Preferencje interfejsu', '06'],
  ].map(([page, symbol, name, description, code]) => `<button class="module-card" data-page-link="${page}"><span class="module-top"><span class="module-symbol">${icon(symbol, 16)}</span><span class="module-code">${code}</span></span><span class="module-name">${name}</span><span class="module-description">${description}</span></button>`).join('')}</div></section>`;
}

function repoPage() {
  const repoFileOptions = repoFiles.map(path => `<label class="repo-file-option"><input type="checkbox" data-repo-file="${escapeHtml(path)}" ${repoSelectedFiles.includes(path) ? 'checked' : ''} ${repoBusy ? 'disabled' : ''}><span>${escapeHtml(path)}</span></label>`).join('');
  const repoQueueItems = repoQueue.map(proposal => `<article class="repo-diff repo-queue-item"><div class="repo-queue-head"><strong>Propozycja zmian · ${proposal.changes.length} plików</strong><span>${proposal.expires_in}s</span></div>${proposal.changes.map(change => `<div class="repo-queue-change"><strong>${escapeHtml(change.path)}</strong><pre>${escapeHtml(change.diff)}</pre></div>`).join('')}<label class="repo-consent"><input type="checkbox" data-repo-consent="${escapeHtml(proposal.proposal_id)}" ${repoBusy ? 'disabled' : ''}> Sprawdziłem diff i zatwierdzam zapis tych zmian.</label><div class="heading-actions"><button class="button primary" data-repo-approve="${escapeHtml(proposal.proposal_id)}" type="button" disabled ${repoBusy ? 'disabled' : ''}>Zapisz zatwierdzone zmiany</button><button class="button" data-repo-reject="${escapeHtml(proposal.proposal_id)}" type="button" ${repoBusy ? 'disabled' : ''}>Odrzuć</button></div></article>`).join('');
  const repoQueueContent = repoQueueError
    ? `<p class="repo-error" role="alert">${escapeHtml(repoQueueError)}</p>`
    : repoQueueLoading && !repoQueue.length
      ? '<p class="mission-note">Sprawdzam kolejkę oczekujących propozycji…</p>'
      : repoQueueItems || '<p class="mission-note">Brak oczekujących próśb o utworzenie lub zmianę plików.</p>';
  return `<div class="page-heading"><div><div class="eyebrow">GEMINI 3.1 FLASH-LITE · OLLAMA FALLBACK</div><h1>Zmiany w repozytorium</h1><p class="page-subtitle">Model proponuje diff; zapis następuje dopiero po Twojej zgodzie.</p></div></div>
    <section class="panel mission-panel"><div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('file')}</span>Kolejka próśb o akceptację</div><span class="panel-meta" id="repo-queue-count">${repoQueue.length} OCZEKUJE</span><button class="button small subtle" type="button" id="repo-queue-refresh" ${repoQueueLoading ? 'disabled' : ''}>${repoQueueLoading ? 'Sprawdzam…' : 'Odśwież kolejkę'}</button></div><div class="panel-body"><div id="repo-approval-queue">${repoQueueContent}</div></div></section>
    <section class="panel mission-panel"><div class="panel-head"><div class="panel-title"><span class="section-icon">${icon('file')}</span>Przygotuj nową prośbę o zmiany</div><span class="panel-meta">ZAPIS PO TWOJEJ ZGODZIE</span></div><div class="panel-body"><p class="mission-note repo-disclosure">Wybrane pliki zostaną wysłane do Gemini razem z opisem zadania. Model może zaproponować edycję wybranych plików lub utworzenie nowych w istniejących katalogach. Nie ma dostępu do powłoki ani plików sekretów.</p><form id="repo-change-form"><div class="repo-controls"><textarea id="repo-mission" aria-label="Zadanie" maxlength="2000" placeholder="Opisz dokładnie, co ma zostać utworzone lub zmienione…" ${repoBusy ? 'disabled' : ''}></textarea></div><div class="repo-files-head"><strong>Pliki do udostępnienia modelowi</strong><button class="button small subtle" type="button" id="repo-files-refresh" ${repoBusy || repoFilesLoading ? 'disabled' : ''}>${repoFilesLoading ? 'Odświeżanie…' : 'Odśwież listę'}</button></div><div class="repo-file-list" id="repo-file-list">${repoFilesLoaded ? repoFileOptions || '<p class="mission-note">Nie znaleziono dostępnych plików tekstowych.</p>' : repoFilesLoading ? '<p class="mission-note">Wczytuję listę plików…</p>' : '<p class="mission-note">Lista plików jest niedostępna.</p>'}</div>${repoFilesError ? `<p class="repo-error" role="alert">${escapeHtml(repoFilesError)}</p>` : ''}${repoChangeError ? `<p class="repo-error" role="alert">${escapeHtml(repoChangeError)}</p>` : ''}<button class="button primary repo-propose-button" type="submit" ${repoBusy || !repoFilesLoaded ? 'disabled' : ''}>${repoBusy ? 'Przygotowuję…' : 'Przygotuj diff do sprawdzenia'}</button></form></div></section>
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

const renderers = { hyperspace: rootTerminalPage, dashboard: dashboardPage, repo: repoPage, terminal: terminalPage, chronolog: chronologPage, models: modelsPage, settings: settingsPage };

function renderPage(name = currentPage) {
  currentPage = name in renderers ? name : 'dashboard';
  const [label] = pageTitles[currentPage];
  breadcrumb.textContent = label;
  document.title = `CactusOS · ${pageTitles[currentPage][1]}`;
  navButtons.forEach((button) => button.classList.toggle('active', button.dataset.page === currentPage));
  pageContent.innerHTML = renderers[currentPage]();
  bindPageActions();
}

function bindPageActions() {
  bindRootTerminal();
  bindUserTerminal();
  pageContent.querySelector('#repo-change-form')?.addEventListener('submit', onRepositoryChangeSubmit);
  pageContent.querySelector('#repo-files-refresh')?.addEventListener('click', loadRepositoryFiles);
  pageContent.querySelectorAll('[data-repo-file]').forEach(input => input.addEventListener('change', () => {
    repoSelectedFiles = [...pageContent.querySelectorAll('[data-repo-file]:checked')].map(file => file.dataset.repoFile);
  }));
  pageContent.querySelector('#repo-queue-refresh')?.addEventListener('click', loadRepositoryQueue);
  pageContent.querySelector('#repo-approval-queue')?.addEventListener('change', event => {
    if (event.target.matches('[data-repo-consent]')) syncRepoApprovalButtons();
  });
  pageContent.querySelector('#repo-approval-queue')?.addEventListener('click', event => {
    const approve = event.target.closest('[data-repo-approve]');
    const reject = event.target.closest('[data-repo-reject]');
    if (approve) resolveRepositoryChange(true, approve.dataset.repoApprove);
    if (reject) resolveRepositoryChange(false, reject.dataset.repoReject);
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
  if (currentPage === 'repo') {
    if (!repoFilesAttempted && !repoFilesLoading) loadRepositoryFiles();
    if (repoQueueTimer) clearInterval(repoQueueTimer);
    repoQueueTimer = setInterval(() => {
      if (!repoBusy) loadRepositoryQueue();
    }, 4000);
    loadRepositoryQueue();
  } else if (repoQueueTimer) {
    clearInterval(repoQueueTimer);
    repoQueueTimer = null;
  }
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

async function loadRepositoryFiles() {
  if (repoFilesLoading) return;
  repoFilesAttempted = true;
  repoFilesLoading = true;
  repoFilesError = '';
  if (currentPage === 'repo') renderPage('repo');
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
    if (currentPage === 'repo') renderPage('repo');
  }
}

async function loadRepositoryQueue() {
  if (repoQueueLoading) return;
  repoQueueLoading = true;
  repoQueueError = '';
  renderRepositoryQueue();
  try {
    const response = await fetch('/api/repo/pending', { cache: 'no-store' });
    const result = await response.json();
    if (!response.ok) throw new Error(result.error || `HTTP ${response.status}`);
    if (!Array.isArray(result.proposals)) throw new Error('Nieprawidłowa odpowiedź kolejki');
    repoQueue = result.proposals;
  } catch (error) {
    repoQueueError = `Nie udało się wczytać kolejki akceptacji: ${error.message}`;
  } finally {
    repoQueueLoading = false;
    renderRepositoryQueue();
  }
}

function renderRepositoryQueue() {
  if (currentPage !== 'repo') return;
  const queue = pageContent.querySelector('#repo-approval-queue');
  if (!queue) return;
  const count = pageContent.querySelector('#repo-queue-count');
  if (count) count.textContent = `${repoQueue.length} OCZEKUJE`;
  const refresh = pageContent.querySelector('#repo-queue-refresh');
  if (refresh) {
    refresh.disabled = repoQueueLoading;
    refresh.textContent = repoQueueLoading ? 'Sprawdzam…' : 'Odśwież kolejkę';
  }
  if (repoQueueError) {
    queue.innerHTML = `<p class="repo-error" role="alert">${escapeHtml(repoQueueError)}</p>`;
    return;
  }
  if (!repoQueue.length) {
    queue.innerHTML = `<p class="mission-note">${repoQueueLoading
      ? 'Sprawdzam kolejkę oczekujących propozycji…'
      : 'Brak oczekujących próśb o utworzenie lub zmianę plików.'}</p>`;
    return;
  }
  queue.innerHTML = repoQueue.map(proposal => `<article class="repo-diff repo-queue-item"><div class="repo-queue-head"><strong>Prośba do akceptacji · ${proposal.changes.length} plików</strong><span>Wygasa za ${proposal.expires_in}s</span></div>${proposal.changes.map(change => `<div class="repo-queue-change"><strong>${escapeHtml(change.path)}</strong><pre>${escapeHtml(change.diff)}</pre></div>`).join('')}<label class="repo-consent"><input type="checkbox" data-repo-consent="${escapeHtml(proposal.proposal_id)}" ${repoBusy ? 'disabled' : ''}> Sprawdziłem diff i zatwierdzam zapis tych zmian.</label><div class="heading-actions"><button class="button primary" data-repo-approve="${escapeHtml(proposal.proposal_id)}" type="button" disabled ${repoBusy ? 'disabled' : ''}>Zapisz zatwierdzone zmiany</button><button class="button" data-repo-reject="${escapeHtml(proposal.proposal_id)}" type="button" ${repoBusy ? 'disabled' : ''}>Odrzuć</button></div></article>`).join('');
}

function syncRepoApprovalButtons() {
  pageContent.querySelectorAll('[data-repo-approve]').forEach(button => {
    const proposalId = button.dataset.repoApprove;
    const consent = [...pageContent.querySelectorAll('[data-repo-consent]')]
      .find(input => input.dataset.repoConsent === proposalId);
    button.disabled = repoBusy || !consent?.checked;
  });
}

async function onRepositoryChangeSubmit(event) {
  event.preventDefault();
  if (repoBusy || chatBusy) return;
  const mission = pageContent.querySelector('#repo-mission').value.trim();
  if (!mission) {
    showToast('Opisz zadanie.');
    return;
  }
  if (!repoSelectedFiles.length) {
    showToast('Wybierz przynajmniej jeden plik jako kontekst.');
    return;
  }
  repoBusy = true;
  chatBusy = true;
  repoChangeError = '';
  renderPage('repo');
  try {
    const response = await terminalFetch('/api/repo/propose', {
      mission,
      paths: repoSelectedFiles,
    });
    const result = await response.json();
    if (!response.ok) throw new Error(`${result.error || 'Błąd serwera'} (HTTP ${response.status})`);
    repoChangeError = `Dodano do kolejki akceptacji (${result.changes.length} plików).`;
  } catch (error) {
    repoChangeError = `Nie udało się przygotować propozycji: ${error.message}`;
  } finally {
    repoBusy = false;
    chatBusy = false;
    if (currentPage === 'repo') renderPage('repo');
  }
}

async function resolveRepositoryChange(approved, proposalId) {
  const proposal = repoQueue.find(item => item.proposal_id === proposalId);
  if (repoBusy || !proposal) return;
  if (approved) {
    const consent = [...pageContent.querySelectorAll('[data-repo-consent]')]
      .find(input => input.dataset.repoConsent === proposalId);
    if (!consent?.checked) return;
  }
  repoBusy = true;
  chatBusy = true;
  repoChangeError = '';
  renderPage('repo');
  try {
    const response = await terminalFetch('/api/repo/resolve', {
      proposal_id: proposal.proposal_id,
      approved,
    });
    const result = await response.json();
    if (!response.ok) throw new Error(`${result.error || 'Błąd serwera'} (HTTP ${response.status})`);
    if (approved) {
      repoChangeError = `Zapisano pliki: ${result.applied.join(', ')}`;
    } else {
      repoChangeError = 'Propozycja została odrzucona; pliki nie zostały zmienione.';
    }
    repoQueue = repoQueue.filter(item => item.proposal_id !== proposalId);
  } catch (error) {
    repoChangeError = `Nie udało się zatwierdzić propozycji: ${error.message}`;
    if (responseInvalidatesProposal(error)) {
      repoQueue = repoQueue.filter(item => item.proposal_id !== proposalId);
    }
  } finally {
    repoBusy = false;
    chatBusy = false;
    if (currentPage === 'repo') renderPage('repo');
    if (approved) loadRepositoryFiles();
  }
}

function responseInvalidatesProposal(error) {
  return /HTTP (400|409)/.test(error.message);
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
    chatMessages = [];
    renderPage(currentPage);
    return;
  }

  const text = command;
  const system = BASE_PROMPT;
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
  const time = new Date().toLocaleTimeString('pl-PL', { hour12: false });
  const entry = { prompt: command || 'Załączniki', text: 'Przygotowuję załączniki…', kind: 'response', time };
  commandHistory.push(entry);
  if (commandHistory.length > 18) commandHistory = commandHistory.slice(-18);
  events.unshift({ time, type: 'CLI', message: `Wpis terminala · ${(command || 'załączniki').slice(0, 60)}`, code: 'AI' });
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
    entry.prompt = `${text || 'Załączniki'}${media.labels.length ? ` [${media.labels.join(', ')}]` : ''}`;
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