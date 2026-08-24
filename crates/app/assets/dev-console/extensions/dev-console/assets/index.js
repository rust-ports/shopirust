const root = document.getElementById('root');
root.innerHTML = `
  <h1>Dev console</h1>
  <p class="muted">UI extensions preview. Connects to the extension WebSocket at <code>/extensions</code>.</p>
  <div id="status">Connecting…</div>
  <div id="log"></div>
`;
const statusEl = document.getElementById('status');
const logEl = document.getElementById('log');
function log(msg) {
  logEl.textContent += msg + '\n';
  logEl.scrollTop = logEl.scrollHeight;
}
const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
const wsUrl = proto + '//' + location.host + '/extensions';
const ws = new WebSocket(wsUrl);
ws.addEventListener('open', () => {
  statusEl.textContent = 'WebSocket connected: ' + wsUrl;
  try {
    ws.send(JSON.stringify({ event: 'connected', data: { client: 'rust-dev-console' } }));
  } catch (e) { log('send error: ' + e); }
});
ws.addEventListener('message', (ev) => {
  log(typeof ev.data === 'string' ? ev.data : JSON.stringify(ev.data));
});
ws.addEventListener('close', () => { statusEl.textContent = 'WebSocket closed'; });
ws.addEventListener('error', () => { statusEl.textContent = 'WebSocket error'; });
