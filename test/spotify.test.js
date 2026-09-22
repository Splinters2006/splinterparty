import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createApp } from '../server.js';

const origin = 'https://party.example.com';
const id = '1234567890123456789012';
const nextId = 'abcdefghijklmnopqrstuv';
const track = (trackId = id) => ({ id: trackId, type: 'track', name: 'Test song', artists: [{ name: 'Test artist' }], duration_ms: 60000, album: { images: [{ url: 'https://i.scdn.co/image/test' }] } });
const json = (data, status = 200, headers) => new Response(JSON.stringify(data), { status, headers });

async function setup(t, mock) {
  const server = await createApp({ spotify: { clientId: 'test-client', publicUrl: origin, fetchImpl: mock } });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(async () => { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); });
  const base = `http://127.0.0.1:${server.address().port}`;
  const request = (path, options = {}) => fetch(base + path, { ...options, redirect: 'manual' });
  const post = (path, data = {}, cookie = '') => request(path, { method: 'POST', headers: { Origin: origin, 'X-Splinterparty': '1', 'Content-Type': 'application/json', Cookie: cookie }, body: JSON.stringify(data) });
  const begin = async (room = '') => {
    const response = await post('/api/spotify/login', { room });
    assert.equal(response.status, 200);
    assert.match(response.headers.get('set-cookie'), /HttpOnly; SameSite=Lax; Path=\/; Max-Age=86400; Secure/);
    return { cookie: response.headers.get('set-cookie').split(';')[0], url: new URL((await response.json()).url) };
  };
  const finish = (login, code = 'alice') => request(`/api/spotify/callback?state=${login.url.searchParams.get('state')}&code=${code}`, { headers: { Cookie: login.cookie } });
  return { request, post, begin, finish };
}

test('Spotify PKCE, separate user sessions, track queue, playback, refresh and disconnect', async t => {
  const calls = [];
  let verifier, challenge, rejectOnce = false, refreshes = 0;
  const app = await setup(t, async (url, options) => {
    calls.push({ url, options });
    if (url.endsWith('/api/token')) {
      const form = options.body;
      if (form.get('grant_type') === 'authorization_code') {
        verifier = form.get('code_verifier');
        assert.equal(createHash('sha256').update(verifier).digest('base64url'), challenge);
        assert.equal(form.get('redirect_uri'), origin + '/api/spotify/callback');
        const user = form.get('code');
        return json({ access_token: `${user}-access`, refresh_token: `${user}-refresh`, expires_in: 3600 });
      }
      refreshes++;
      assert.equal(form.get('refresh_token'), 'alice-refresh');
      return json({ access_token: 'alice-renewed', expires_in: 3600 });
    }
    if (url.endsWith('/me')) return json({ display_name: options.headers.Authorization.includes('alice') ? 'Alice' : 'Bob' }); // `product` intentionally absent.
    if (url.includes('/search?')) {
      if (rejectOnce) { rejectOnce = false; return json({}, 401); }
      return json({ tracks: { items: [track()] } });
    }
    if (url.includes('/tracks/')) return json(track(url.split('/').at(-1)));
    if (url.includes('/me/player/play?')) return new Response(null, { status: 204 });
    throw new Error('Unexpected provider request');
  });
  const room = await (await app.post('/api/rooms')).json();
  const route = `/api/rooms/${room.code}`;
  assert.equal((await app.post(route + '/spotify', { id })).status, 401);
  const login = await app.begin(room.code);
  challenge = login.url.searchParams.get('code_challenge');
  assert.equal(login.url.searchParams.get('code_challenge_method'), 'S256');
  assert.ok(login.url.searchParams.get('scope').includes('streaming'));
  const callback = await app.finish(login);
  assert.equal(callback.status, 302);
  assert.equal(callback.headers.get('location'), `${origin}/?spotify=connected#${room.code}`);
  const cookie = callback.headers.get('set-cookie').split(';')[0];
  assert.notEqual(cookie, login.cookie);
  assert.equal((await app.finish(login)).status, 403); // Single-use state and rotated cookie.
  const profile = await (await app.request('/api/spotify/status', { headers: { Cookie: cookie } })).json();
  assert.equal(profile.linked, true); assert.equal(profile.name, 'Alice');
  assert.equal((await app.post('/api/spotify/token', {}, cookie)).headers.get('cache-control'), 'no-store');
  const alice = await (await app.post('/api/spotify/token', {}, cookie)).json();
  assert.equal(alice.accessToken, 'alice-access');
  const bobLogin = await app.begin(); challenge = bobLogin.url.searchParams.get('code_challenge');
  const bobCallback = await app.finish(bobLogin, 'bob');
  const bobCookie = bobCallback.headers.get('set-cookie').split(';')[0];
  assert.equal((await (await app.post('/api/spotify/token', {}, bobCookie)).json()).accessToken, 'bob-access');
  assert.equal((await app.post('/api/spotify/token')).status, 401);
  assert.equal((await app.request('/api/spotify/token', { method: 'POST', headers: { Cookie: cookie } })).status, 403);
  assert.equal((await app.request('/api/spotify/token', { method: 'POST', headers: { Cookie: cookie, Origin: 'https://evil.example', 'X-Splinterparty': '1' } })).status, 403);
  const search = await app.request('/api/spotify/search?q=test', { headers: { Cookie: cookie } });
  assert.equal(search.status, 200);
  assert.equal((await search.json()).tracks[0].spotifyId, id);
  const direct = await app.request(`/api/spotify/search?q=${encodeURIComponent('https://open.spotify.com/track/' + id + '?si=abc')}`, { headers: { Cookie: cookie } });
  assert.equal((await direct.json()).tracks[0].uri, `spotify:track:${id}`);
  const addition = await app.post(route + '/spotify', { id }, cookie);
  assert.equal(addition.status, 201);
  const shared = await addition.json();
  assert.equal(shared.tracks[0].source, 'spotify');
  assert.equal(shared.tracks[0].duration, 60);
  assert.ok(!/alice|access|refresh|verifier/i.test(JSON.stringify(shared)));
  assert.equal((await app.request(route + '/tracks/' + shared.current)).status, 404);
  assert.equal((await app.post(route + '/spotify', { id: 'https://evil.example' }, cookie)).status, 400);
  assert.equal((await app.post(route + '/spotify-play', { device: 'device1', id: shared.current }, cookie)).status, 409);
  await app.post(route + '/control', { action: 'play' });
  await app.post(route + '/control', { action: 'seek', position: 12 });
  const playback = await app.post(route + '/spotify-play', { device: 'device1', id: shared.current }, cookie);
  assert.equal(playback.status, 200);
  const playCall = calls.find(call => call.url.includes('/me/player/play?'));
  assert.equal(playCall.options.headers.Authorization, 'Bearer alice-access');
  assert.deepEqual(JSON.parse(playCall.options.body).uris, [`spotify:track:${id}`]);
  assert.ok(JSON.parse(playCall.options.body).position_ms >= 12000);
  rejectOnce = true;
  assert.equal((await app.request('/api/spotify/search?q=refresh', { headers: { Cookie: cookie } })).status, 200);
  assert.equal(refreshes, 1);
  assert.equal((await (await app.post('/api/spotify/token', {}, cookie)).json()).accessToken, 'alice-renewed');
  const next = await (await app.post(route + '/spotify', { id: nextId }, cookie)).json();
  await app.post(route + '/control', { action: 'seek', position: 60 });
  await new Promise(resolve => setTimeout(resolve, 650));
  const advanced = await (await app.request(route)).json();
  assert.equal(advanced.current, next.tracks[1].id);
  assert.equal(advanced.playing, true);
  const upload = await app.request(route + '/tracks?name=local.mp3', { method: 'POST', body: 'ID3fixture' });
  const uploaded = (await upload.json()).tracks.at(-1);
  await app.post(route + '/control', { action: 'next' });
  assert.equal((await (await app.request(route)).json()).current, uploaded.id);
  assert.equal((await app.post(route + '/spotify-play', { device: 'device1', id: shared.current }, cookie)).status, 409);
  assert.equal((await app.post('/api/spotify/disconnect', {}, cookie)).status, 200);
  assert.equal((await app.post('/api/spotify/token', {}, cookie)).status, 401);
  assert.equal((await (await app.request('/api/spotify/status', { headers: { Cookie: cookie } })).json()).name, null);
  assert.equal((await app.post('/api/spotify/token', {}, bobCookie)).status, 200);
  const publicScript = await (await app.request('/spotify.js')).text();
  assert.ok(!publicScript.includes('refresh_token'));
  assert.equal((await app.request('/privacy.html')).status, 200);
});

test('Spotify rejects bad state, cancelled authorization, known free accounts and handles rate limits', async t => {
  let free = true, searchCalls = 0;
  const app = await setup(t, async url => {
    if (url.endsWith('/api/token')) return json({ access_token: 'token', refresh_token: 'refresh', expires_in: 3600 });
    if (url.endsWith('/me')) return json({ product: free ? 'free' : 'premium', display_name: 'Listener' });
    if (url.includes('/search?')) { searchCalls++; return json({}, 429, { 'Retry-After': '30' }); }
    throw new Error('Unexpected provider request');
  });
  const login = await app.begin();
  assert.equal((await app.request('/api/spotify/callback?state=wrong&code=x', { headers: { Cookie: login.cookie } })).status, 403);
  assert.equal((await app.request(`/api/spotify/callback?state=${login.url.searchParams.get('state')}&code=x`)).status, 403);
  const failed = await app.finish(login);
  assert.equal(failed.headers.get('location'), `${origin}/?spotify=premium-required`);
  assert.equal((await app.post('/api/spotify/token', {}, login.cookie)).status, 401);
  const denied = await app.begin();
  const cancel = await app.request(`/api/spotify/callback?state=${denied.url.searchParams.get('state')}&error=access_denied`, { headers: { Cookie: denied.cookie } });
  assert.equal(cancel.headers.get('location'), `${origin}/?spotify=failed`);
  free = false;
  const success = await app.finish(await app.begin());
  const cookie = success.headers.get('set-cookie').split(';')[0];
  for (let i = 0; i < 2; i++) assert.equal((await app.request('/api/spotify/search?q=test', { headers: { Cookie: cookie } })).status, 429);
  assert.equal(searchCalls, 1);
});

test('Spotify config rejects non-HTTPS deployment origins', async () => {
  await assert.rejects(createApp({ spotify: { clientId: 'test', publicUrl: 'http://192.168.1.1:3000' } }), /HTTPS/);
  await assert.rejects(createApp({ spotify: { clientId: 'test', publicUrl: 'https://example.com/path' } }), /without a path/);
});
