const rootTerminalState = {
  busy: false, proposal: null, vector: '', reply: '', result: '', error: '',
  executionAllowed: false, rootAvailable: false,
};

const userTerminalState = {
  busy: false, proposal: null, result: '', error: '', executionAllowed: false,
};

const pythonScriptState = {
  busy: false, scripts: [], selectedPath: '', proposal: null, result: '', error: '',
};

function rootTerminalPage() {
  return `<div class="page-heading"><div><div class="eyebrow">BINARY HYPERSPACE · 256 BIT</div><h1>Terminal hiperprzestrzeni</h1><p class="page-subtitle">Kontener Codespaces · katalog roboczy /workspaces/CactusOS</p></div><span class="runtime-tag" id="root-status">Sprawdzanie wykonawcy root</span></div>
    <form id="root-command-form" class="terminal-composer">
      <label for="root-command-input" class="composer-prompt">$</label>
      <input id="root-command-input" class="terminal-input" maxlength="2000" required autocomplete="off" placeholder="Polecenie lub opis zmiany w swobodnym języku" aria-label="Polecenie hiperterminala">
      <button class="button primary" type="submit">${icon('send')} Wyślij do modelu</button>
    </form>
    ${agentActivityPanel()}
    <section class="panel python-script-panel">
      <div class="panel-head"><span class="panel-title">Uruchom skrypt Python</span><span id="python-script-status" class="panel-meta" role="status">Ładowanie skryptów…</span></div>
      <div class="panel-body">
        <p class="python-script-disclosure">Po zgodzie kod działa jako administrator kontenera Codespaces, nie systemu hosta.</p>
        <div class="python-script-controls">
          <label for="python-script-select">Plik</label>
          <select id="python-script-select" aria-label="Wybierz skrypt Python"><option value="">Wybierz skrypt</option></select>
          <button id="python-script-refresh" class="button subtle" type="button" title="Odśwież listę skryptów" aria-label="Odśwież listę skryptów">${icon('refresh')}</button>
          <button id="python-script-propose" class="button primary" type="button" disabled>Przygotuj uruchomienie</button>
        </div>
        <div class="python-script-paste">
          <label for="python-script-paste-input">Albo wklej skrypt Python</label>
          <textarea id="python-script-paste-input" maxlength="65536" rows="8" spellcheck="false" placeholder="Wklej kod Python do sprawdzenia i zatwierdzenia…"></textarea>
          <button id="python-script-paste-propose" class="button" type="button" disabled>${icon('file')} Przygotuj podgląd</button>
        </div>
        <section id="python-script-approval" class="approval-inline" aria-live="polite" hidden>
          <strong>Uruchomienie z uprawnieniami administratora</strong>
          <p id="python-script-approval-path"></p>
          <pre id="python-script-source" class="root-text python-script-source"></pre>
          <p id="python-script-approval-error" class="root-error" role="status"></p>
          <div class="heading-actions">
            <button id="python-script-approve" class="button primary" type="button" disabled>Uruchom jako administrator</button>
            <button id="python-script-reject" class="button" type="button">Odrzuć</button>
          </div>
        </section>
        <p id="python-script-error" class="root-error" role="alert"></p>
        <pre id="python-script-result" class="root-text"></pre>
      </div>
    </section>
    <p id="root-progress" role="status" class="runtime-detail"></p>
    <p id="root-error" role="alert" class="root-error"></p>
    <div class="content-grid">
      <section class="panel"><div class="panel-head"><span class="panel-title">Wektor binarny · 16 × 16</span><span class="panel-meta">GF(2)</span></div><div class="panel-body"><pre id="root-vector" class="binary-matrix"></pre></div></section>
      <section class="panel"><div class="panel-head"><span class="panel-title">Interpretacja modelu</span><span class="panel-meta">GEMINI · OLLAMA FALLBACK</span></div><div class="panel-body"><p id="root-reply" class="root-text"></p></div></section>
    </div>
    <section class="panel"><div class="panel-head"><span class="panel-title">Wynik wykonania</span><span class="panel-meta">LIMIT 60 S · 64 KiB WYJŚCIA</span></div><div class="panel-body"><pre id="root-result" class="root-text"></pre></div></section>`;
}

async function rootRequest(path, data) {
  const response = await terminalFetch(path, data);
  const result = await response.json();
  if (!response.ok) throw new Error(`${result.error || 'Błąd serwera'} (HTTP ${response.status})`);
  return result;
}

function updateUserTerminal() {
  const state = userTerminalState;
  updateApprovalNotice();
  const error = document.querySelector('#user-command-error');
  const result = document.querySelector('#user-command-result');
  if (error) error.textContent = state.error;
  if (result) result.textContent = state.result;
  const form = document.querySelector('#command-form');
  form?.querySelectorAll('button, input').forEach(control => {
    control.disabled = state.busy || chatBusy
      || rootTerminalState.busy
      || pythonScriptState.busy || Boolean(pythonScriptState.proposal);
  });
}

function bindUserTerminal() {
  const form = document.querySelector('#command-form');
  if (!form) return;
  userTerminalState.executionAllowed = Boolean(document.querySelector('meta[name="terminal-csrf"]'));
  updateUserTerminal();
}

async function submitUserCommand(prompt, entry, activityId) {
  const state = userTerminalState;
  state.busy = true;
  state.error = '';
  state.result = '';
  updateAgentActivity(activityId, 'running', 'Model przygotowuje polecenie do zatwierdzenia');
  updateUserTerminal();
  try {
    const proposal = await rootRequest('/api/terminal/propose', { prompt });
    entry.text = proposal.reply;
    state.proposal = proposal.proposal_id ? proposal : null;
    updateAgentActivity(activityId, state.proposal ? 'waiting' : 'done',
      state.proposal ? proposal.command : 'Nie zaproponowano polecenia do wykonania',
      state.proposal ? 'Oczekuje na zgodę' : 'Analiza zakończona');
  } catch (error) {
    entry.kind = 'notice';
    entry.text = error.message;
    updateAgentActivity(activityId, 'error', error.message, 'Błąd propozycji');
  } finally {
    state.busy = false;
    persistTerminalHistory();
    updateTerminalOutput(entry);
    updateUserTerminal();
  }
}

async function resolveUserProposal(approved) {
  const state = userTerminalState;
  if (state.busy || !state.proposal) return;
  const proposal = state.proposal;
  const activityId = addAgentActivity(
    approved ? 'Running' : 'Cancelled',
    approved ? 'Uruchamianie zatwierdzonego polecenia' : 'Polecenie odrzucone · nic nie uruchomiono',
    approved ? 'running' : 'cancelled',
  );
  state.busy = true;
  state.error = '';
  updateUserTerminal();
  try {
    const result = await rootRequest('/api/terminal/resolve', {
      proposal_id: proposal.proposal_id, command: proposal.command, approved,
    });
    state.proposal = null;
    state.result = result.cancelled ? 'Odrzucono. Niczego nie wykonano.'
      : `${result.output}\nKod wyjścia: ${result.exit_code}${result.truncated ? '\nWyjście ucięte do 64 KiB.' : ''}`;
    updateAgentActivity(activityId, result.cancelled ? 'cancelled' : 'done',
      result.cancelled ? 'Nie wykonano polecenia' : `Kod wyjścia: ${result.exit_code}`,
      result.cancelled ? 'Cancelled' : 'Running zakończone');
    recordEvent({time: new Date().toLocaleTimeString('pl-PL'), type: 'CLI',
      message: approved ? 'Zatwierdzone polecenie użytkownika zakończone' : 'Propozycja polecenia odrzucona',
      code: result.cancelled ? 'CANCEL' : String(result.exit_code)});
  } catch (error) {
    state.error = error.message;
    updateAgentActivity(activityId, 'error', error.message, 'Błąd wykonania');
    if (responseInvalidatesProposal(error)) state.proposal = null;
  } finally {
    state.busy = false;
    updateUserTerminal();
  }
}

function updateRootTerminal() {
  updateApprovalNotice();
  updatePythonScriptUI();
  const form = document.querySelector('#root-command-form');
  if (!form) return;
  const state = rootTerminalState;
  document.querySelector('#root-vector').textContent = state.vector
    ? state.vector.match(/.{16}/g).map(row => row.split('').join(' ')).join('\n') : 'Oczekiwanie na wektor';
  document.querySelector('#root-reply').textContent = state.reply;
  document.querySelector('#root-result').textContent = state.result;
  document.querySelector('#root-error').textContent = state.error;
  document.querySelector('#root-progress').textContent = state.busy ? 'Przetwarzanie…' : '';
  form.querySelectorAll('button, input').forEach(control => {
    control.disabled = state.busy
      || userTerminalState.busy
      || pythonScriptState.busy || Boolean(pythonScriptState.proposal);
  });
}

function updatePythonScriptUI() {
  const state = pythonScriptState;
  const select = document.querySelector('#python-script-select');
  if (!select) return;
  const status = document.querySelector('#python-script-status');
  const approval = document.querySelector('#python-script-approval');
  const proposalActive = Boolean(state.proposal);
  const otherWorkActive = rootTerminalState.busy || Boolean(rootTerminalState.proposal)
    || userTerminalState.busy || Boolean(userTerminalState.proposal);
  const disabled = state.busy || proposalActive || otherWorkActive;

  status.textContent = state.busy ? 'Przetwarzanie…'
    : `${state.scripts.length} skryptów · ${rootTerminalState.rootAvailable ? 'administrator dostępny' : 'administrator niedostępny'}`;
  select.disabled = disabled;
  document.querySelector('#python-script-paste-input').disabled = disabled;
  document.querySelector('#python-script-refresh').disabled = disabled;
  document.querySelector('#python-script-propose').disabled = disabled
    || !rootTerminalState.rootAvailable || !state.selectedPath;
  document.querySelector('#python-script-paste-propose').disabled = disabled
    || !rootTerminalState.rootAvailable
    || !document.querySelector('#python-script-paste-input').value.trim();
  approval.hidden = !proposalActive;
  document.querySelector('#python-script-approval-path').textContent = proposalActive
    ? `${state.proposal.path} · SHA-256 ${state.proposal.sha256}` : '';
  document.querySelector('#python-script-source').textContent = proposalActive ? state.proposal.source : '';
  document.querySelector('#python-script-approval-error').textContent = state.busy ? 'Przetwarzanie decyzji…' : '';
  document.querySelector('#python-script-approve').disabled = state.busy;
  document.querySelector('#python-script-reject').disabled = state.busy;
  document.querySelector('#python-script-error').textContent = state.error;
  document.querySelector('#python-script-result').textContent = state.result;
}

async function loadPythonScripts() {
  const state = pythonScriptState;
  const status = document.querySelector('#python-script-status');
  if (!status || state.busy || state.proposal) return;
  state.busy = true;
  state.error = '';
  const activityId = addAgentActivity('Loading', 'Ładowanie listy skryptów Python');
  updatePythonScriptUI();
  try {
    const response = await fetch('/api/root/scripts', { cache: 'no-store' });
    const result = await response.json();
    if (!response.ok) throw new Error(`${result.error || 'Błąd serwera'} (HTTP ${response.status})`);
    state.scripts = result.scripts;
    updateAgentActivity(activityId, 'done', `${state.scripts.length} skryptów dostępnych`, 'Loaded');
    if (!state.scripts.includes(state.selectedPath)) state.selectedPath = state.scripts[0] || '';
    const select = document.querySelector('#python-script-select');
    select.replaceChildren(new Option('Wybierz skrypt', ''));
    for (const path of state.scripts) select.add(new Option(path, path));
    select.value = state.selectedPath;
  } catch (error) {
    state.error = error.message;
    updateAgentActivity(activityId, 'error', error.message, 'Błąd ładowania skryptów');
  } finally {
    state.busy = false;
    updatePythonScriptUI();
  }
}

async function preparePythonScript(payload) {
  const state = pythonScriptState;
  if (state.busy || state.proposal || !rootTerminalState.rootAvailable) return;
  state.busy = true;
  state.error = '';
  state.result = '';
  const activityId = addAgentActivity('Loading', 'Przygotowanie podglądu skryptu');
  updatePythonScriptUI();
  try {
    state.proposal = await rootRequest('/api/root/script/propose', payload);
    updateAgentActivity(activityId, 'waiting', `${state.proposal.path} · SHA-256 ${state.proposal.sha256}`, 'Oczekuje na zgodę');
  } catch (error) {
    state.error = error.message;
    updateAgentActivity(activityId, 'error', error.message, 'Błąd przygotowania skryptu');
  } finally {
    state.busy = false;
    updateRootTerminal();
  }
}

async function proposePythonScript() {
  if (!pythonScriptState.selectedPath) return;
  await preparePythonScript({ path: pythonScriptState.selectedPath });
}

async function proposePastedPythonScript() {
  const source = document.querySelector('#python-script-paste-input').value;
  if (!source.trim()) return;
  await preparePythonScript({ source });
}

async function resolvePythonScript(approved) {
  const state = pythonScriptState;
  if (state.busy || !state.proposal) return;
  const proposal = state.proposal;
  const activityId = addAgentActivity(
    approved ? 'Running' : 'Cancelled',
    approved ? `Uruchamianie ${proposal.path}` : 'Skrypt odrzucony · nie został uruchomiony',
    approved ? 'running' : 'cancelled',
  );
  state.busy = true;
  state.error = '';
  updatePythonScriptUI();
  try {
    const result = await rootRequest('/api/root/script/resolve', {
      proposal_id: proposal.proposal_id, approved, sha256: proposal.sha256,
    });
    state.proposal = null;
    state.result = result.cancelled ? 'Odrzucono. Skrypt nie został uruchomiony.'
      : `${result.output}\nKod wyjścia: ${result.exit_code}${result.truncated ? '\nWyjście ucięte do 64 KiB.' : ''}`;
    updateAgentActivity(activityId, result.cancelled ? 'cancelled' : 'done',
      result.cancelled ? 'Skrypt nie został uruchomiony' : `Kod wyjścia: ${result.exit_code}`,
      result.cancelled ? 'Cancelled' : 'Running zakończone');
    if (!result.cancelled) {
      recordEvent({time: new Date().toLocaleTimeString('pl-PL'), type: 'ROOT',
        message: `Skrypt Python · ${proposal.path} · SHA ${proposal.sha256.slice(0, 12)}`,
        code: String(result.exit_code)});
    }
  } catch (error) {
    state.error = error.message;
    updateAgentActivity(activityId, 'error', error.message, 'Błąd uruchomienia skryptu');
    if (responseInvalidatesProposal(error)) {
      state.proposal = null;
    }
  } finally {
    state.busy = false;
    updateRootTerminal();
    updateUserTerminal();
  }
}

function bindRootTerminal() {
  const form = document.querySelector('#root-command-form');
  if (!form) return;
  pageContent.querySelector('#python-script-select')?.addEventListener('change', event => {
    pythonScriptState.selectedPath = event.target.value;
    updatePythonScriptUI();
  });
  pageContent.querySelector('#python-script-refresh')?.addEventListener('click', loadPythonScripts);
  pageContent.querySelector('#python-script-propose')?.addEventListener('click', proposePythonScript);
  pageContent.querySelector('#python-script-paste-propose')?.addEventListener('click', proposePastedPythonScript);
  pageContent.querySelector('#python-script-paste-input')?.addEventListener('input', updatePythonScriptUI);
  pageContent.querySelector('#python-script-approve')?.addEventListener('click', () => resolvePythonScript(true));
  pageContent.querySelector('#python-script-reject')?.addEventListener('click', () => resolvePythonScript(false));
  updateRootTerminal();
  loadPythonScripts();
  const status = document.querySelector('#root-status');
  if (!document.querySelector('meta[name="terminal-csrf"]')) {
    status.textContent = 'Backend root niepodłączony · port 4180';
  } else {
    fetch('/api/root/status', { cache: 'no-store' }).then(async response => {
      const result = await response.json();
      if (!response.ok) throw new Error(`${result.error || 'Błąd serwera'} (HTTP ${response.status})`);
      return result;
    }).then(result => {
      rootTerminalState.rootAvailable = result.root_available === true;
      rootTerminalState.executionAllowed = result.execution_allowed === true;
      status.textContent = rootTerminalState.rootAvailable
        ? 'ROOT DOSTĘPNY · WYMAGA ZGODY'
        : 'ROOT NIEDOSTĘPNY (sudo -n) · ZGODA NADAL WYMAGANA';
      updateRootTerminal();
    }).catch(error => {
      rootTerminalState.executionAllowed = false;
      rootTerminalState.rootAvailable = false;
      rootTerminalState.error = `Nie można sprawdzić uprawnień root: ${error.message}`;
      status.textContent = 'Backend niedostępny';
      updateRootTerminal();
    });
  }

  form.addEventListener('submit', async event => {
    event.preventDefault();
    const state = rootTerminalState;
    if (state.busy || userTerminalState.busy
      || pythonScriptState.busy || pythonScriptState.proposal) return;
    const input = document.querySelector('#root-command-input');
    const prompt = input.value.trim();
    if (!prompt) return;
    const approval = terminalApprovalCommand(prompt);
    if (approval !== null) {
      input.value = '';
      await resolvePendingTerminalProposal(approval);
      return;
    }
    if (state.proposal || userTerminalState.proposal) return;
    const activityId = addAgentActivity('Loading', `Analiza polecenia · ${prompt}`);
    state.busy = true;
    state.error = '';
    state.reply = '';
    state.result = '';
    state.vector = '';
    updateRootTerminal();
    try {
      const proposal = await rootRequest('/api/root/propose', { prompt });
      state.vector = proposal.vector;
      state.reply = proposal.reply;
      state.proposal = proposal.proposal_id ? proposal : null;
      updateAgentActivity(activityId, state.proposal ? 'waiting' : 'done',
        state.proposal ? proposal.command : proposal.reply,
        state.proposal ? 'Oczekuje na zgodę root' : 'Analiza zakończona');
    } catch (error) {
      state.error = error.message;
      updateAgentActivity(activityId, 'error', error.message, 'Błąd analizy');
    } finally {
      state.busy = false;
      updateRootTerminal();
    }
  });
}

function updateApprovalNotice() {
  const pendingApprovals = document.querySelector('#pending-approvals');
  if (pendingApprovals) {
    pendingApprovals.hidden = !userTerminalState.proposal && !rootTerminalState.proposal;
  }
  for (const [prefix, state, scope] of [
    ['user', userTerminalState, 'Wykonaj'],
    ['root', rootTerminalState, 'Wykonaj jako root'],
  ]) {
    const approval = document.querySelector(`#${prefix}-approval`);
    if (!approval) continue;
    approval.hidden = !state.proposal;
    if (!state.proposal) continue;
    document.querySelector(`#${prefix}-approval-command`).textContent = state.proposal.command;
    document.querySelector(`#${prefix}-approval-error`).textContent = state.error
      || (state.busy ? 'Przetwarzanie decyzji…' : '');
    const approve = document.querySelector(`#${prefix}-approval-approve`);
    approve.textContent = state.busy ? 'Przetwarzanie…' : scope;
    approve.disabled = state.busy;
    document.querySelector(`#${prefix}-approval-reject`).disabled = state.busy;
  }
}

function bindApprovalActions() {
  const userApprove = document.querySelector('#user-approval-approve');
  const userReject = document.querySelector('#user-approval-reject');
  const rootApprove = document.querySelector('#root-approval-approve');
  const rootReject = document.querySelector('#root-approval-reject');
  if (userApprove) userApprove.onclick = () => resolveUserProposal(true);
  if (userReject) userReject.onclick = () => resolveUserProposal(false);
  if (rootApprove) rootApprove.onclick = () => resolveRootProposal(true);
  if (rootReject) rootReject.onclick = () => resolveRootProposal(false);
}

async function resolveRootProposal(approved) {
  const state = rootTerminalState;
  if (state.busy || !state.proposal) return;
  const proposal = state.proposal;
  const activityId = addAgentActivity(
    approved ? 'Running' : 'Cancelled',
    approved ? 'Uruchamianie zatwierdzonego polecenia root' : 'Polecenie root odrzucone',
    approved ? 'running' : 'cancelled',
  );
  state.busy = true;
  state.error = '';
  updateRootTerminal();
  try {
    const result = await rootRequest('/api/root/resolve', {
      proposal_id: proposal.proposal_id, command: proposal.command, approved,
    });
    state.proposal = null;
    state.result = result.cancelled ? 'Odrzucono. Niczego nie wykonano.'
      : `${result.output}\nKod wyjścia: ${result.exit_code}${result.truncated ? '\nWyjście ucięte do 64 KiB.' : ''}`;
    updateAgentActivity(activityId, result.cancelled ? 'cancelled' : 'done',
      result.cancelled ? 'Nic nie wykonano' : `Kod wyjścia: ${result.exit_code}`,
      result.cancelled ? 'Cancelled' : 'Running zakończone');
    if (!result.cancelled && result.exit_code !== 0 && !state.rootAvailable) {
      state.error = 'Polecenie wykonane bez pełnych uprawnień root (sudo -n niedostępne). Skonfiguruj NOPASSWD jeśli potrzebujesz root.';
    }
    recordEvent({time: new Date().toLocaleTimeString('pl-PL'), type: 'ROOT',
      message: approved ? 'Zatwierdzone polecenie zakończone' : 'Propozycja odrzucona',
      code: result.cancelled ? 'CANCEL' : String(result.exit_code)});
  } catch (error) {
    state.error = error.message;
    updateAgentActivity(activityId, 'error', error.message, 'Błąd wykonania root');
    if (responseInvalidatesProposal(error)) state.proposal = null;
  } finally {
    state.busy = false;
    updateRootTerminal();
  }
}

function terminalApprovalCommand(text) {
  const command = text.trim().toLowerCase();
  if (['approve', 'accept', 'yes'].includes(command)) return true;
  if (['reject', 'deny', 'no'].includes(command)) return false;
  return null;
}

async function resolvePendingTerminalProposal(approved, entry = null) {
  const pending = [
    ['user', userTerminalState],
    ['root', rootTerminalState],
  ].filter(([, state]) => state.proposal);

  if (pending.length !== 1) {
    const message = pending.length
      ? 'Jest więcej niż jedna oczekująca propozycja; zatwierdź ją w jej panelu.'
      : 'Brak oczekującej propozycji terminala.';
    if (entry) {
      entry.text = message;
    } else {
      rootTerminalState.error = message;
      updateRootTerminal();
    }
    return;
  }

  const [terminal, state] = pending[0];
  if (state.busy) {
    const message = 'Decyzja dla propozycji jest już przetwarzana.';
    if (entry) entry.text = message;
    else {
      state.error = message;
      if (terminal === 'root') updateRootTerminal();
      else updateUserTerminal();
    }
    return;
  }

  if (terminal === 'root') await resolveRootProposal(approved);
  else await resolveUserProposal(approved);
  if (entry) entry.text = state.error || state.result || 'Decyzja została przetworzona.';
}
