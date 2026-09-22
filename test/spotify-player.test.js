import { test } from 'node:test';
import assert from 'node:assert/strict';
import { SpotifyPlayback } from '../public/spotify.js';

const track = { id: 'queue-id', uri: 'spotify:track:1234567890123456789012', duration: 120 };
const desired = (changes = {}) => ({ room: 'ABC123', track, position: 15, receivedAt: performance.now(), playing: true, ...changes });
function fixture(api = async () => ({ ok: true })) {
  const calls = [];
  const notices = [];
  let state = null;
  const client = new SpotifyPlayback(async (...args) => { calls.push(['api', ...args]); return api(...args); }, message => notices.push(message), () => {});
  client.ready = true; client.enabled = true; client.device = 'browser-device';
  client.player = {
    getCurrentState: async () => state,
    pause: async () => { calls.push(['pause']); if (state) state.paused = true; },
    resume: async () => { calls.push(['resume']); if (state) state.paused = false; },
    seek: async position => { calls.push(['seek', position]); if (state) state.position = position; },
    disconnect: () => calls.push(['disconnect']),
  };
  return { client, calls, notices, setState: value => { state = value; } };
}

test('Spotify browser controller starts own device, seeks drift, pauses, and respects local audio enable', async () => {
  const { client, calls, setState } = fixture();
  client.desired = desired();
  await client.reconcile();
  const start = calls.find(call => call[0] === 'api');
  assert.equal(start[1], '/api/rooms/ABC123/spotify-play');
  assert.deepEqual(JSON.parse(start[2].body), { device: 'browser-device', id: track.id });
  setState({ paused: true, position: 0, track_window: { current_track: { uri: track.uri } } });
  await client.reconcile();
  assert.ok(calls.find(call => call[0] === 'seek' && call[1] >= 15000));
  assert.ok(calls.find(call => call[0] === 'resume'));
  client.desired = desired({ playing: false });
  await client.reconcile();
  assert.equal(calls.at(-1)[0], 'pause');
  client.enabled = false;
  client.desired = desired();
  const count = calls.filter(call => call[0] === 'api').length;
  await client.reconcile();
  assert.equal(calls.filter(call => call[0] === 'api').length, count);
});

test('switching away waits for pending Spotify start and pauses it before allowing uploads', async () => {
  let finish;
  const pending = new Promise(resolve => { finish = resolve; });
  const { client, calls } = fixture(() => pending);
  client.desired = desired();
  const start = client.reconcile();
  await new Promise(resolve => setImmediate(resolve));
  assert.ok(calls.some(call => call[0] === 'api'));
  let stopped = false;
  const stopping = client.stop().then(() => { stopped = true; });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(stopped, false);
  finish({ ok: true });
  await start; await stopping;
  assert.equal(stopped, true);
  assert.equal(client.desired, null);
  assert.equal(calls.at(-1)[0], 'pause');
});

test('provider failures back off rather than looping playback requests', async () => {
  const { client, calls, notices } = fixture(async () => { const error = new Error('Rate limited'); error.status = 429; throw error; });
  client.desired = desired();
  await client.reconcile(); await client.reconcile();
  assert.equal(calls.filter(call => call[0] === 'api').length, 1);
  assert.deepEqual(notices, ['Rate limited']);
  assert.ok(client.retryAt > Date.now());
});

test('a Premium account error disables audio and reports the reason', async t => {
  const previousWindow = globalThis.window, previousDocument = globalThis.document;
  t.after(() => { globalThis.window = previousWindow; globalThis.document = previousDocument; });
  const listeners = new Map();
  globalThis.document = { getElementById: () => ({ value: '0.8' }) };
  globalThis.window = { Spotify: { Player: class {
    addListener(event, callback) { listeners.set(event, callback); }
    async connect() { return true; }
  } } };
  const client = new SpotifyPlayback(async () => ({}), () => {}, () => {});
  client.linked = true;
  await client.connect();
  listeners.get('account_error')({ message: 'Invalid account' });
  assert.equal(client.ready, false);
  assert.equal(client.enabled, false);
  assert.match(client.problem, /Premium is required/);
});
