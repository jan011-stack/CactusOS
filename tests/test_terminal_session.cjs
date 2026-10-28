const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');

function client(fetch) {
  const meta = { content: 'old-token' };
  const module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../web/ollama.js'), 'utf8'), {
    module, document: { querySelector: () => meta }, fetch,
  });
  return { ...module.exports, meta };
}

test('terminal approval commands recognize approve and reject aliases', () => {
  const context = {};
  const source = fs.readFileSync(path.join(__dirname, '../web/root_terminal.js'), 'utf8');
  vm.runInNewContext(`${source}\nglobalThis.approvalCommand = terminalApprovalCommand;`, context);

  for (const command of ['approve', ' ACCEPT ', 'Yes']) {
    assert.equal(context.approvalCommand(command), true);
  }
  for (const command of ['reject', ' DENY ', 'No']) {
    assert.equal(context.approvalCommand(command), false);
  }
  for (const command of ['approved', 'yeah', 'status']) {
    assert.equal(context.approvalCommand(command), null);
  }
});

test('expired CSRF refreshes once and resends the same payload', async () => {
  const calls = [];
  const api = client(async (url, options) => {
    calls.push({ url, options });
    if (url === '/api/session') return Response.json({ csrf: 'new-token' });
    if (options.headers['X-Terminal-CSRF'] === 'old-token') {
      return Response.json({ code: 'csrf' }, { status: 403 });
    }
    return Response.json({ ok: true });
  });
  const result = await api.terminalFetch('/api/ollama/chat', { messages: ['godzina'] });
  assert.equal(result.status, 200);
  assert.equal(api.meta.content, 'new-token');
  assert.equal(calls.length, 3);
  assert.equal(calls[0].options.body, calls[2].options.body);
});

test('foreign origins are not retried or treated as an Ollama outage', async () => {
  let calls = 0;
  const api = client(async () => {
    calls++;
    return Response.json({ code: 'origin' }, { status: 403 });
  });
  await assert.rejects(api.terminalFetch('/api/ollama/chat', {}), /HTTP 403/);
  assert.equal(calls, 1);
});

test('explicit ping requests go to the executor proposal, not ordinary chat', () => {
  const api = client(async () => { throw new Error('No network expected'); });
  for (const text of [
    'ping 127.0.0.1',
    'Wyślij ping 127.0.0.1',
    'wyslij ping',
    'uruchom polecenie ping localhost',
    'Kazałem wysłać ping 127.0.0.1',
    'proszę sprawdzić ping localhost',
  ]) {
    assert.equal(api.isPingExecutionRequest(text), true, text);
  }
  for (const text of ['Co robi ping?', 'Wyjaśnij ping', 'cześć', 'pingwin']) {
    assert.equal(api.isPingExecutionRequest(text), false, text);
  }
  assert.equal(api.isPingExecutionRequest('wyślij ping localhost'), true);
});

test('ordinary terminal distinguishes execution requests from conversation', () => {
  const api = client(async () => { throw new Error('No network expected'); });
  for (const text of [
    'Sprawdź pliki w src', 'Uruchom testy Cargo', 'Run tests',
    'Połącz z siecią deep space NETWORK USA', 'Connect to private network',
    'ls -la', 'git status', 'python3 -m unittest', 'ping 127.0.0.1',
  ]) {
    assert.equal(api.isCommandExecutionRequest(text), true, text);
  }
  for (const text of [
    'Cześć', 'Jak działa Cargo?', 'Jak połączyć się z prywatną siecią?',
    'What does this test do?', 'Co robi ping?',
  ]) {
    assert.equal(api.isCommandExecutionRequest(text), false, text);
  }
});

test('chat history keeps image payloads for multimodal model requests', () => {
  const api = client(async () => { throw new Error('No network expected'); });
  const image = 'data:image/jpeg;base64,/9j/AA==';
  const messages = api.buildMessages([
    { role: 'user', content: 'Co jest na obrazie?', images: [image] },
  ], 'system');
  assert.deepEqual(messages[1].images, [image]);
});

test('chat history is capped to twelve messages to bound browser memory', () => {
  const api = client(async () => { throw new Error('No network expected'); });
  const history = Array.from({ length: 20 }, (_, index) => ({
    role: 'user',
    content: `message ${index}`,
  }));
  api.trimChatHistory(history);
  assert.equal(history.length, 12);
  assert.equal(history[0].content, 'message 8');
  assert.equal(history.at(-1).content, 'message 19');
});
