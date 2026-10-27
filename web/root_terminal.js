const rootTerminalState = {
  busy: false, proposal: null, vector: '', reply: '', result: '', error: '',
  executionAllowed: false, rootAvailable: false, consent: false,
};

const userTerminalState = {
  busy: false, proposal: null, result: '', error: '', executionAllowed: false, consent: false,
};

function rootTerminalPage() {
  return `<div class="page-heading"><div><div class="eyebrow">BINARY HYPERSPACE · 256 BIT</div><h1>Terminal hiperprzestrzeni</h1><p class="page-subtitle">Kontener Codespaces · katalog roboczy /workspaces/CactusOS</p></div><span class="runtime-tag" id="root-status">Sprawdzanie wykonawcy root</span></div>
    <form id="root-command-form" class="terminal-composer">
      <label for="root-command-input" class="composer-prompt">$</label>
      <input id="root-command-input" class="terminal-input" maxlength="2000" required autocomplete="off" placeholder="Polecenie lub opis zmiany w swobodnym języku" aria-label="Polecenie hiperterminala">
      <button class="button primary" type="submit">${icon('send')} Wyślij do modelu</button>
    </form>
    <section id="root-approval" class="approval-inline" aria-live="polite" hidden>
      <strong>Wymagana zgoda · uprawnienia root</strong>
      <p>Sprawdź dokładne polecenie przed uruchomieniem jako root:</p>
      <pre id="root-approval-command" class="root-text"></pre>
      <p id="root-approval-error" class="root-error" role="status"></p>
      <label class="root-consent"><input type="checkbox" id="root-approval-consent"> Zatwierdzam dokładnie to polecenie jako root.</label>
      <div class="heading-actions">
        <button id="root-approval-approve" class="button primary" type="button" disabled>Wykonaj jako root</button>
        <button id="root-approval-reject" class="button" type="button">Odrzuć</button>
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
    control.disabled = state.busy || Boolean(state.proposal) || chatBusy
      || rootTerminalState.busy || Boolean(rootTerminalState.proposal);
  });
}

function bindUserTerminal() {
  const form = document.querySelector('#command-form');
  if (!form) return;
  userTerminalState.executionAllowed = Boolean(document.querySelector('meta[name="terminal-csrf"]'));
  pageContent.querySelector('#user-approval-consent')?.addEventListener('change', event => {
    userTerminalState.consent = event.target.checked;
    updateUserTerminal();
  });
  pageContent.querySelector('#user-approval-approve')
    ?.addEventListener('click', () => resolveUserProposal(true));
  pageContent.querySelector('#user-approval-reject')
    ?.addEventListener('click', () => resolveUserProposal(false));
  updateUserTerminal();
}

async function submitUserCommand(prompt, entry) {
  const state = userTerminalState;
  state.busy = true;
  state.error = '';
  state.result = '';
  updateUserTerminal();
  try {
    const proposal = await rootRequest('/api/terminal/propose', { prompt });
    entry.text = proposal.reply;
    state.consent = false;
    state.proposal = proposal.proposal_id ? proposal : null;
  } catch (error) {
    entry.kind = 'notice';
    entry.text = error.message;
  } finally {
    state.busy = false;
    updateTerminalOutput(entry);
    updateUserTerminal();
  }
}

async function resolveUserProposal(approved) {
  const state = userTerminalState;
  if (state.busy || !state.proposal) return;
  if (approved && !state.consent) return;
  const proposal = state.proposal;
  state.busy = true;
  state.error = '';
  updateUserTerminal();
  try {
    const result = await rootRequest('/api/terminal/resolve', {
      proposal_id: proposal.proposal_id, command: proposal.command, approved,
    });
    state.proposal = null;
    state.consent = false;
    state.result = result.cancelled ? 'Odrzucono. Niczego nie wykonano.'
      : `${result.output}\nKod wyjścia: ${result.exit_code}${result.truncated ? '\nWyjście ucięte do 64 KiB.' : ''}`;
    events.unshift({time: new Date().toLocaleTimeString('pl-PL'), type: 'CLI',
      message: approved ? 'Zatwierdzone polecenie użytkownika zakończone' : 'Propozycja polecenia odrzucona',
      code: result.cancelled ? 'CANCEL' : String(result.exit_code)});
    events = events.slice(0, 16);
  } catch (error) {
    state.error = error.message;
    if (responseInvalidatesProposal(error)) state.proposal = null;
  } finally {
    state.busy = false;
    updateUserTerminal();
  }
}

function updateRootTerminal() {
  updateApprovalNotice();
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
    control.disabled = state.busy || Boolean(state.proposal)
      || userTerminalState.busy || Boolean(userTerminalState.proposal);
  });
}

function bindRootTerminal() {
  const form = document.querySelector('#root-command-form');
  if (!form) return;
  pageContent.querySelector('#root-approval-consent')?.addEventListener('change', event => {
    rootTerminalState.consent = event.target.checked;
    updateRootTerminal();
  });
  pageContent.querySelector('#root-approval-approve')
    ?.addEventListener('click', () => resolveRootProposal(true));
  pageContent.querySelector('#root-approval-reject')
    ?.addEventListener('click', () => resolveRootProposal(false));
  updateRootTerminal();
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
    if (state.busy || state.proposal || userTerminalState.busy || userTerminalState.proposal) return;
    const prompt = document.querySelector('#root-command-input').value.trim();
    if (!prompt) return;
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
      state.consent = false;
      state.proposal = proposal.proposal_id ? proposal : null;
    } catch (error) {
      state.error = error.message;
    } finally {
      state.busy = false;
      updateRootTerminal();
    }
  });
}

function updateApprovalNotice() {
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
    const consent = document.querySelector(`#${prefix}-approval-consent`);
    consent.checked = state.consent;
    consent.disabled = state.busy;
    const approve = document.querySelector(`#${prefix}-approval-approve`);
    approve.textContent = state.busy ? 'Przetwarzanie…' : scope;
    // Consent alone enables the button. Backend enforces actual privileges.
    approve.disabled = state.busy || !state.consent;
    document.querySelector(`#${prefix}-approval-reject`).disabled = state.busy;
  }
}

async function resolveRootProposal(approved) {
  const state = rootTerminalState;
  if (state.busy || !state.proposal) return;
  if (approved && !state.consent) return;
  const proposal = state.proposal;
  state.busy = true;
  state.error = '';
  updateRootTerminal();
  try {
    const result = await rootRequest('/api/root/resolve', {
      proposal_id: proposal.proposal_id, command: proposal.command, approved,
    });
    state.proposal = null;
    state.consent = false;
    state.result = result.cancelled ? 'Odrzucono. Niczego nie wykonano.'
      : `${result.output}\nKod wyjścia: ${result.exit_code}${result.truncated ? '\nWyjście ucięte do 64 KiB.' : ''}`;
    if (!result.cancelled && result.exit_code !== 0 && !state.rootAvailable) {
      state.error = 'Polecenie wykonane bez pełnych uprawnień root (sudo -n niedostępne). Skonfiguruj NOPASSWD jeśli potrzebujesz root.';
    }
    events.unshift({time: new Date().toLocaleTimeString('pl-PL'), type: 'ROOT',
      message: approved ? 'Zatwierdzone polecenie zakończone' : 'Propozycja odrzucona',
      code: result.cancelled ? 'CANCEL' : String(result.exit_code)});
    events = events.slice(0, 16);
  } catch (error) {
    state.error = error.message;
    if (responseInvalidatesProposal(error)) state.proposal = null;
  } finally {
    state.busy = false;
    updateRootTerminal();
  }
}
