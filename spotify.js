import { createHash, randomBytes } from 'node:crypto';

export class SpotifyError extends Error {
  constructor(message, status = 400) { super(message); this.status = status; }
}

export function spotifyTrack(item) {
  if (!item || !/^[a-zA-Z0-9]{22}$/.test(item.id) || item.type !== 'track' || item.is_local || item.is_playable === false || !Number.isFinite(item.duration_ms) || item.duration_ms <= 0) {
    throw new SpotifyError('This Spotify track is not available for playback.');
  }
  return {
    source: 'spotify', spotifyId: item.id, uri: `spotify:track:${item.id}`,
    name: String(item.name).slice(0, 300), artist: (item.artists || []).map(artist => artist.name).join(', ').slice(0, 300),
    duration: item.duration_ms / 1000, url: `https://open.spotify.com/track/${item.id}`,
    image: (item.album?.images || []).find(image => /^https:\/\/i\.scdn\.co\//.test(image.url))?.url || null,
  };
}

export function createSpotify({ clientId = process.env.SPOTIFY_CLIENT_ID, publicUrl = process.env.PUBLIC_URL, fetchImpl = fetch } = {}) {
  let origin = null;
  if (clientId) {
    let url;
    try { url = new URL(publicUrl); } catch { throw new Error('Set PUBLIC_URL to your HTTPS app URL to enable Spotify.'); }
    if ((url.protocol !== 'https:' && !(url.protocol === 'http:' && ['127.0.0.1', '[::1]'].includes(url.hostname))) || url.pathname !== '/' || url.search || url.hash || url.username || url.password) {
      throw new Error('PUBLIC_URL must be an HTTPS origin (or an HTTP loopback IP for local development), without a path.');
    }
    origin = url.origin;
  }
  const configured = Boolean(clientId && origin);
  const redirectUri = origin && `${origin}/api/spotify/callback`;
  const sessions = new Map();
  const random = () => randomBytes(32).toString('base64url');
  const cookie = (res, id, age = 86400) => res.setHeader('Set-Cookie', `splinterparty_spotify=${id}; HttpOnly; SameSite=Lax; Path=/; Max-Age=${age}${origin?.startsWith('https:') ? '; Secure' : ''}`);
  const sweep = () => { for (const [id, session] of sessions) if (session.expires < Date.now()) sessions.delete(id); };
  const lookup = req => {
    sweep();
    const id = req.headers.cookie?.split(';').map(part => part.trim()).find(part => part.startsWith('splinterparty_spotify='))?.slice('splinterparty_spotify='.length);
    return id && sessions.get(id);
  };
  const active = session => sessions.get(session.id) === session && session.expires > Date.now();
  const requireSession = req => {
    const session = lookup(req);
    if (!session?.tokens) throw new SpotifyError('Link your Spotify Premium account first.', 401);
    return session;
  };
  const guard = req => {
    if (!configured) throw new SpotifyError('Spotify is not configured on this server.', 503);
    if (req.headers.origin !== origin || req.headers['x-splinterparty'] !== '1') throw new SpotifyError('Open the app at its configured public URL and try again.', 403);
  };
  const provider = async (url, options) => {
    try { return await fetchImpl(url, { ...options, signal: AbortSignal.timeout(12000) }); }
    catch { throw new SpotifyError('Spotify could not be reached. Try again shortly.', 502); }
  };
  const exchange = async params => {
    const response = await provider('https://accounts.spotify.com/api/token', {
      method: 'POST', headers: { 'Content-Type': 'application/x-www-form-urlencoded' }, body: new URLSearchParams({ client_id: clientId, ...params }),
    });
    if (!response.ok) throw new SpotifyError('Spotify authorization expired or was denied. Link your account again.', 401);
    const data = await response.json();
    if (!data.access_token || !Number.isFinite(data.expires_in)) throw new SpotifyError('Invalid response from Spotify.', 502);
    return { access: data.access_token, refresh: data.refresh_token, expires: Date.now() + data.expires_in * 1000 };
  };
  const token = async session => {
    if (!active(session)) throw new SpotifyError('Link Spotify again.', 401);
    if (session.tokens.expires < Date.now() + 60000) {
      if (!session.refreshing) session.refreshing = (async () => {
        try {
          const next = await exchange({ grant_type: 'refresh_token', refresh_token: session.tokens.refresh });
          if (!active(session)) throw new SpotifyError('Spotify was disconnected.', 401);
          session.tokens = { ...next, refresh: next.refresh || session.tokens.refresh };
        } catch (error) { if (error.status === 401) sessions.delete(session.id); throw error; }
        finally { session.refreshing = null; }
      })();
      await session.refreshing;
    }
    return session.tokens.access;
  };
  const request = async (session, endpoint, options = {}, retried = false) => {
    if (session.retryAt > Date.now()) throw new SpotifyError('Spotify is rate limiting requests. Wait a little before trying again.', 429);
    const access = await token(session);
    const response = await provider(`https://api.spotify.com/v1${endpoint}`, { ...options, headers: { Authorization: `Bearer ${access}`, 'Content-Type': 'application/json' } });
    if (!active(session)) throw new SpotifyError('Spotify was disconnected.', 401);
    if (response.status === 401 && !retried) { session.tokens.expires = 0; return request(session, endpoint, options, true); }
    if (response.status === 429) {
      session.retryAt = Date.now() + Math.max(1, Number(response.headers.get('retry-after')) || 30) * 1000;
      throw new SpotifyError('Spotify is rate limiting requests. Wait a little before trying again.', 429);
    }
    if (!response.ok) {
      const message = response.status === 403 ? 'Spotify denied access. Premium and Spotify app access are required.'
        : response.status === 404 ? 'Track or Spotify device unavailable. Enable Spotify audio again.'
          : 'Spotify could not complete this request. Try again.';
      throw new SpotifyError(message, response.status === 401 ? 401 : response.status === 403 ? 403 : 502);
    }
    return response.status === 204 ? null : response.json();
  };
  return {
    configured, origin, sweep,
    close() { sessions.clear(); },
    guard,
    async track(req, id) {
      guard(req);
      if (!/^[a-zA-Z0-9]{22}$/.test(id || '')) throw new SpotifyError('Choose a valid Spotify track.');
      return spotifyTrack(await request(requireSession(req), `/tracks/${id}`));
    },
    async play(req, device, track, position) {
      guard(req);
      if (!/^[a-zA-Z0-9_-]{1,128}$/.test(device || '')) throw new SpotifyError('Enable Spotify audio on this device first.');
      await request(requireSession(req), `/me/player/play?device_id=${encodeURIComponent(device)}`, {
        method: 'PUT', body: JSON.stringify({ uris: [track.uri], position_ms: Math.floor(Math.max(0, Math.min(position, track.duration - 0.1)) * 1000) }),
      });
    },
    async handle(req, res, url, json, body) {
      if (!url.pathname.startsWith('/api/spotify/')) return false;
      if (req.method === 'GET' && url.pathname === '/api/spotify/status') {
        const session = lookup(req);
        json(res, 200, { configured, linked: Boolean(session?.tokens), name: session?.name || null, origin });
        return true;
      }
      if (!configured) throw new SpotifyError('Spotify is not configured on this server.', 503);
      if (req.method === 'POST') guard(req);
      if (req.method === 'POST' && url.pathname === '/api/spotify/login') {
        const data = await body(req);
        const room = /^[A-Fa-f0-9]{6}$/.test(data.room || '') ? data.room.toUpperCase() : '';
        sweep();
        if (sessions.size >= 1000) throw new SpotifyError('Too many Spotify sessions. Try again later.', 503);
        const previous = lookup(req);
        if (previous) sessions.delete(previous.id);
        const session = { id: random(), state: random(), verifier: randomBytes(64).toString('base64url'), room, expires: Date.now() + 600000 };
        sessions.set(session.id, session);
        cookie(res, session.id);
        const params = new URLSearchParams({ client_id: clientId, response_type: 'code', redirect_uri: redirectUri, state: session.state,
          code_challenge_method: 'S256', code_challenge: createHash('sha256').update(session.verifier).digest('base64url'),
          scope: 'streaming user-read-private user-read-email user-read-playback-state user-modify-playback-state',
        });
        json(res, 200, { url: `https://accounts.spotify.com/authorize?${params}` });
      } else if (req.method === 'GET' && url.pathname === '/api/spotify/callback') {
        const session = lookup(req);
        if (!session?.state || session.state !== url.searchParams.get('state')) throw new SpotifyError('Invalid or expired Spotify login. Return to the app and try again.', 403);
        const verifier = session.verifier;
        delete session.state; delete session.verifier;
        let result = 'connected';
        try {
          if (url.searchParams.has('error') || !url.searchParams.get('code')) throw new SpotifyError('Authorization denied.');
          session.tokens = await exchange({ grant_type: 'authorization_code', code: url.searchParams.get('code'), redirect_uri: redirectUri, code_verifier: verifier });
          const profile = await request(session, '/me');
          // New Development Mode apps no longer receive `product`. The SDK
          // enforces Premium at connection time when this field is absent.
          if (profile.product && profile.product !== 'premium') { result = 'premium-required'; throw new SpotifyError('Premium required.'); }
          session.name = String(profile.display_name || 'Spotify listener').slice(0, 100);
          if (!active(session)) throw new SpotifyError('Session expired.');
          sessions.delete(session.id);
          session.id = random(); session.expires = Date.now() + 86400000;
          sessions.set(session.id, session); cookie(res, session.id);
        } catch {
          sessions.delete(session.id); cookie(res, '', 0);
          if (result !== 'premium-required') result = 'failed';
        }
        res.writeHead(302, { Location: `${origin}/?spotify=${result}${session.room ? '#' + session.room : ''}`, 'Cache-Control': 'no-store' }); res.end();
      } else if (req.method === 'POST' && url.pathname === '/api/spotify/disconnect') {
        const session = lookup(req);
        if (session) sessions.delete(session.id);
        cookie(res, '', 0); json(res, 200, { linked: false });
      } else if (req.method === 'POST' && url.pathname === '/api/spotify/token') {
        json(res, 200, { accessToken: await token(requireSession(req)) });
      } else if (req.method === 'GET' && url.pathname === '/api/spotify/search') {
        const session = requireSession(req);
        const query = url.searchParams.get('q')?.trim();
        if (!query || query.length > 200) throw new SpotifyError('Enter a track name, artist, or Spotify track link (up to 200 characters).');
        const direct = query.match(/^(?:spotify:track:|https:\/\/open\.spotify\.com\/(?:intl-[a-z]+\/)?track\/)([a-zA-Z0-9]{22})(?:\?[^\s]*)?$/);
        const result = direct ? { tracks: { items: [await request(session, `/tracks/${direct[1]}`)] } }
          : await request(session, `/search?${new URLSearchParams({ q: query, type: 'track', limit: '10' })}`);
        const tracks = (result.tracks?.items || []).flatMap(item => { try { return [spotifyTrack(item)]; } catch { return []; } });
        json(res, 200, { tracks });
      } else { json(res, 404, { error: 'Spotify endpoint not found.' }); }
      return true;
    },
  };
}
