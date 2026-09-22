import http from 'node:http';
import { randomBytes, randomUUID } from 'node:crypto';
import { createReadStream, createWriteStream } from 'node:fs';
import { mkdtemp, rm, stat } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { pipeline } from 'node:stream/promises';
import { Transform } from 'node:stream';
import { createSpotify, SpotifyError } from './spotify.js';
import { loadEnvFile } from 'node:process';

const publicDir = fileURLToPath(new URL('./public/', import.meta.url));
const MAX_FILE = 50 * 1024 * 1024;
const MAX_STORAGE = 1024 * 1024 * 1024;

export async function createApp(options = {}) {
  const spotify = createSpotify(options.spotify);
  const directory = await mkdtemp(path.join(tmpdir(), 'splinterparty-'));
  const rooms = new Map();
  let storage = 0;
  let reservations = 0;
  const position = room => room.playing ? room.position + (Date.now() - room.updatedAt) / 1000 : room.position;
  const snapshot = room => ({ code: room.code, tracks: room.tracks, current: room.current, playing: room.playing, position: position(room), listeners: room.clients.size, serverTime: Date.now() });
  const broadcast = room => {
    room.touched = Date.now();
    const message = `data: ${JSON.stringify(snapshot(room))}\n\n`;
    for (const client of room.clients) {
      if (!client.write(message)) client.destroy();
    }
  };
  const json = (res, status, value) => { res.writeHead(status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' }); res.end(JSON.stringify(value)); };
  const body = async req => {
    let text = '';
    for await (const chunk of req) { text += chunk; if (text.length > 4096) throw new Error('Request too large'); }
    return JSON.parse(text || '{}');
  };
  const server = http.createServer(async (req, res) => {
    res.setHeader('X-Content-Type-Options', 'nosniff');
    res.setHeader('Referrer-Policy', 'no-referrer');
    res.setHeader('Content-Security-Policy', "default-src 'self'; script-src 'self' https://sdk.scdn.co; style-src 'self'; img-src 'self' https://i.scdn.co; media-src 'self' blob: https://*.scdn.co; connect-src 'self' https://*.spotify.com https://*.scdn.co https://*.spotify.net wss://*.spotify.com wss://*.spotify.net; frame-src https://sdk.scdn.co https://accounts.spotify.com; base-uri 'none'; frame-ancestors 'none'");
    try {
      if (req.headers.origin && req.headers.origin !== spotify.origin && req.headers.origin !== `http://${req.headers.host}` && req.headers.origin !== `https://${req.headers.host}`) return json(res, 403, { error: 'Cross-origin requests are not allowed' });
      const url = new URL(req.url, 'http://localhost');
      if (await spotify.handle(req, res, url, json, body)) return;
      if (req.method === 'POST' && url.pathname === '/api/rooms') {
        if (rooms.size >= 100) return json(res, 503, { error: 'Room limit reached. Try again later.' });
        let code;
        do { code = randomBytes(3).toString('hex').toUpperCase(); } while (rooms.has(code));
        const room = { code, tracks: [], current: null, playing: false, position: 0, updatedAt: Date.now(), touched: Date.now(), clients: new Set(), uploads: 0 };
        rooms.set(code, room);
        return json(res, 201, snapshot(room));
      }
      const match = url.pathname.match(/^\/api\/rooms\/([a-fA-F0-9]{6})(?:\/(events|tracks|control|spotify|spotify-play)(?:\/([a-f0-9-]+))?)?$/);
      if (match) {
        const room = rooms.get(match[1].toUpperCase());
        if (!room) return json(res, 404, { error: 'Room not found. Check your code or create a new room.' });
        const [, , action, trackId] = match;
        room.touched = Date.now();
        if (req.method === 'GET' && !action) return json(res, 200, snapshot(room));
        if (req.method === 'POST' && action === 'spotify') {
          spotify.guard(req);
          if (room.tracks.length + room.uploads >= 50) return json(res, 413, { error: 'Room queue is full.' });
          room.uploads++;
          try {
            const data = await body(req);
            const track = { ...await spotify.track(req, data.id), id: randomUUID() };
            room.tracks.push(track);
            if (!room.current) room.current = track.id;
            broadcast(room);
            return json(res, 201, snapshot(room));
          } finally { room.uploads--; }
        }
        if (req.method === 'POST' && action === 'spotify-play') {
          spotify.guard(req);
          const data = await body(req);
          const track = room.tracks.find(track => track.id === room.current);
          if (track?.source !== 'spotify' || !room.playing || data.id !== track.id) return json(res, 409, { error: 'The room playback changed. Sync again.' });
          await spotify.play(req, data.device, track, position(room));
          return json(res, 200, { ok: true });
        }
        if (req.method === 'GET' && action === 'events') {
          if (room.clients.size >= 100) return json(res, 503, { error: 'This room is full' });
          res.writeHead(200, { 'Content-Type': 'text/event-stream', 'Cache-Control': 'no-cache', Connection: 'keep-alive', 'X-Accel-Buffering': 'no' });
          room.clients.add(res);
          broadcast(room);
          req.on('close', () => { room.clients.delete(res); broadcast(room); });
          return;
        }
        if (req.method === 'POST' && action === 'tracks') {
          const length = Number(req.headers['content-length']);
          if (!Number.isInteger(length) || length < 4 || length > MAX_FILE) return json(res, 413, { error: 'Upload an MP3 or FLAC up to 50 MB.' });
          if (room.tracks.length + room.uploads >= 50 || storage + reservations + length > MAX_STORAGE) return json(res, 413, { error: 'Upload storage or room queue is full.' });
          const name = (url.searchParams.get('name') || 'Untitled.mp3').slice(0, 180);
          if (!/\.(mp3|flac)$/i.test(name)) return json(res, 400, { error: 'Only MP3 and FLAC files are supported.' });
          const isFlac = /\.flac$/i.test(name);
          const contentType = isFlac ? 'audio/flac' : 'audio/mpeg';
          const id = randomUUID();
          const filename = path.join(directory, id);
          reservations += length;
          room.uploads++;
          let bytes = 0;
          let header = Buffer.alloc(0);
          try {
            await pipeline(req, new Transform({ transform(chunk, encoding, callback) {
              bytes += chunk.length;
              if (header.length < 4) header = Buffer.concat([header, chunk.subarray(0, 4 - header.length)]);
              callback(bytes > length ? new Error('Upload exceeds declared size') : null, chunk);
            } }), createWriteStream(filename, { flags: 'wx' }));
            const validHeader = isFlac
              ? header.toString('latin1') === 'fLaC'
              : header.subarray(0, 3).toString('latin1') === 'ID3' || (header[0] === 0xff && (header[1] & 0xe0) === 0xe0);
            if (bytes !== length || !validHeader) {
              await rm(filename, { force: true });
              return json(res, 400, { error: `This file does not appear to be ${isFlac ? 'a FLAC' : 'an MP3'}.` });
            }
            storage += bytes;
            room.tracks.push({ id, name: name.replace(/\.(mp3|flac)$/i, ''), size: bytes, contentType });
            if (!room.current) room.current = id;
            broadcast(room);
            return json(res, 201, snapshot(room));
          } catch (error) { await rm(filename, { force: true }); throw error; }
          finally { reservations -= length; room.uploads--; }
        }
        if ((req.method === 'GET' || req.method === 'HEAD') && action === 'tracks' && trackId) {
          const track = room.tracks.find(track => track.id === trackId);
          if (!track || track.source === 'spotify') return json(res, 404, { error: 'Uploaded track not found' });
          const filename = path.join(directory, trackId);
          const { size } = await stat(filename);
          let start = 0, end = size - 1;
          if (req.headers.range) {
            const range = req.headers.range.match(/^bytes=(\d*)-(\d*)$/);
            if (range && (range[1] || range[2])) {
              start = range[1] ? Number(range[1]) : Math.max(0, size - Number(range[2]));
              end = range[1] && range[2] ? Math.min(size - 1, Number(range[2])) : size - 1;
            } else start = size;
            if (start >= size || start > end) { res.writeHead(416, { 'Content-Range': `bytes */${size}` }); return res.end(); }
          }
          res.writeHead(req.headers.range ? 206 : 200, { 'Content-Type': track.contentType, 'Accept-Ranges': 'bytes', 'Content-Length': end - start + 1, 'Cache-Control': 'private, no-store', ...(req.headers.range ? { 'Content-Range': `bytes ${start}-${end}/${size}` } : {}) });
          if (req.method === 'HEAD') return res.end();
          await pipeline(createReadStream(filename, { start, end }), res);
          return;
        }
        if (req.method === 'POST' && action === 'control') {
          const data = await body(req);
          if (!room.current) return json(res, 400, { error: 'Add a track first.' });
          room.position = position(room);
          room.updatedAt = Date.now();
          if (data.action === 'play') room.playing = true;
          else if (data.action === 'pause') room.playing = false;
          else if (data.action === 'seek' && Number.isFinite(data.position) && data.position >= 0 && data.position <= 86400) {
            const track = room.tracks.find(track => track.id === room.current);
            room.position = track?.source === 'spotify' ? Math.min(data.position, track.duration) : data.position;
          }
          else if (data.action === 'select' && room.tracks.some(track => track.id === data.id)) { room.current = data.id; room.position = 0; }
          else if (data.action === 'next' || data.action === 'ended') {
            if (data.action === 'ended' && data.id !== room.current) return json(res, 200, snapshot(room));
            const next = room.tracks[room.tracks.findIndex(track => track.id === room.current) + 1];
            if (next) { room.current = next.id; room.position = 0; }
            else { room.playing = false; room.position = 0; }
          } else return json(res, 400, { error: 'Invalid playback command' });
          broadcast(room);
          return json(res, 200, snapshot(room));
        }
        return json(res, 405, { error: 'Method not allowed' });
      }
      if (req.method === 'GET' && ['/', '/app.js', '/spotify.js', '/style.css', '/privacy.html'].includes(url.pathname)) {
        const filename = url.pathname === '/' ? 'index.html' : url.pathname.slice(1);
        res.setHeader('Content-Type', filename.endsWith('.html') ? 'text/html; charset=utf-8' : filename.endsWith('.css') ? 'text/css' : 'text/javascript');
        await pipeline(createReadStream(path.join(publicDir, filename)), res);
        return;
      }
      json(res, 404, { error: 'Not found' });
    } catch (error) { if (!res.headersSent && !res.destroyed) json(res, error instanceof SpotifyError ? error.status : 400, { error: error instanceof SpotifyError ? error.message : 'Could not complete the request.' }); }
  });
  // The room advances Spotify tracks even if an unlinked listener is present,
  // or browsers are backgrounded and do not deliver player callbacks promptly.
  const playbackTimer = setInterval(() => {
    for (const room of rooms.values()) {
      const track = room.tracks.find(track => track.id === room.current);
      if (room.playing && track?.source === 'spotify' && position(room) >= track.duration) {
        const next = room.tracks[room.tracks.indexOf(track) + 1];
        if (next) room.current = next.id;
        else room.playing = false;
        room.position = 0; room.updatedAt = Date.now(); broadcast(room);
      }
    }
  }, 500);
  playbackTimer.unref();
  const timer = setInterval(() => {
    spotify.sweep();
    for (const [code, room] of rooms) {
      for (const client of room.clients) client.write(': heartbeat\n\n');
      if (!room.clients.size && !room.uploads && Date.now() - room.touched > 24 * 60 * 60 * 1000) {
        rooms.delete(code);
        for (const track of room.tracks) if (track.source !== 'spotify') { storage -= track.size; rm(path.join(directory, track.id), { force: true }).catch(() => {}); }
      }
    }
  }, 15000);
  timer.unref();
  server.on('close', () => { clearInterval(timer); clearInterval(playbackTimer); spotify.close(); rm(directory, { recursive: true, force: true }).catch(() => {}); });
  return server;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { loadEnvFile(fileURLToPath(new URL('./.env', import.meta.url))); }
  catch (error) { if (error.code !== 'ENOENT') throw error; }
  const server = await createApp();
  const port = Number(process.env.PORT || 3000);
  server.on('error', error => {
    if (error.code === 'EADDRINUSE') {
      console.error(`Port ${port} is already in use. Start on another port with: PORT=${port === 65535 ? 3001 : port + 1} npm start`);
    } else {
      console.error(`Could not start splinterparty: ${error.message}`);
    }
    process.exitCode = 1;
    server.close();
  });
  server.listen(port, process.env.HOST || '0.0.0.0', () => console.log(`splinterparty listening on port ${server.address().port}`));
  for (const signal of ['SIGTERM', 'SIGINT']) process.on(signal, () => { server.close(); server.closeAllConnections(); });
}
