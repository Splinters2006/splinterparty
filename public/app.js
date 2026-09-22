import { SpotifyPlayback } from './spotify.js';

const $ = id => document.getElementById(id);
const audio = $('audio');
let code = null, state = null, events = null, enabled = false, receivedAt = 0, sourceId = null, noticeTimer;
audio.volume = 0.8;
const spotify = new SpotifyPlayback(api, notice, updateSpotifyUI);
const currentTrack = () => state?.tracks.find(track => track.id === state.current);
function notice(message) { $('notice').textContent = message; $('notice').hidden = false; clearTimeout(noticeTimer); noticeTimer = setTimeout(() => $('notice').hidden = true, 6000); }
async function api(url, options = {}) {
  const response = await fetch(url, options);
  const result = await response.json();
  if (!response.ok) { const error = new Error(result.error || 'Something went wrong. Please try again.'); error.status = response.status; throw error; }
  return result;
}
function targetPosition() { return state ? state.position + (state.playing ? (performance.now() - receivedAt) / 1000 : 0) : 0; }
async function sync() {
  if (!state || !state.current) return;
  const track = currentTrack();
  if (track?.source === 'spotify') {
    audio.pause();
    spotify.update({ room: code, track, playing: state.playing, position: state.position, receivedAt });
    return;
  }
  await spotify.stop();
  if (currentTrack()?.id !== track?.id || !state || (events && events.readyState !== EventSource.OPEN)) return;
  if (sourceId !== state.current) { sourceId = state.current; audio.src = `/api/rooms/${code}/tracks/${sourceId}`; audio.load(); }
  const target = targetPosition();
  if (audio.readyState >= 1 && Number.isFinite(audio.duration) && Math.abs(audio.currentTime - target) > 0.6) audio.currentTime = Math.min(target, Math.max(0, audio.duration - 0.05));
  if (state.playing && enabled) {
    try { await audio.play(); } catch { enabled = false; $('enable').hidden = false; }
  } else audio.pause();
}
function render(next) {
  state = next; receivedAt = performance.now();
  $('room-code').textContent = code;
  $('listener-count').textContent = state.listeners;
  $('listener-label').textContent = state.listeners === 1 ? 'listener' : 'listeners';
  const track = state.tracks.find(item => item.id === state.current);
  $('track-title').textContent = track?.name || 'Room for something good';
  $('track-subtitle').textContent = track?.source === 'spotify' ? `${track.artist} · Spotify` : track ? 'Shared with everyone in this room.' : 'Upload an MP3 or FLAC to start the session.';
  $('spotify-now-link').hidden = track?.source !== 'spotify';
  $('record-placeholder').hidden = track?.source === 'spotify';
  if (track?.source === 'spotify') {
    $('spotify-now-link').href = track.url;
    $('spotify-art').hidden = !track.image;
    if (track.image) $('spotify-art').src = track.image;
    else $('spotify-art').removeAttribute('src');
  } else { $('spotify-art').removeAttribute('src'); $('spotify-now-link').removeAttribute('href'); }
  updateSpotifyUI();
  $('play').disabled = !track; $('next').disabled = !track;
  $('play').textContent = state.playing ? 'Ⅱ' : '▶';
  $('play').setAttribute('aria-label', state.playing ? 'Pause' : 'Play');
  $('track-count').textContent = state.tracks.length;
  $('empty-queue').hidden = !!state.tracks.length;
  $('queue').replaceChildren(...state.tracks.map((item, index) => {
    const li = document.createElement('li');
    const button = document.createElement('button'); button.className = `track${item.id === state.current ? ' active' : ''}`;
    const number = document.createElement('span'); number.className = 'number'; number.textContent = item.id === state.current ? '♫' : String(index + 1).padStart(2, '0');
    const name = document.createElement('span'); name.className = 'name'; name.textContent = item.source === 'spotify' ? `${item.name} · ${item.artist}` : item.name;
    const size = document.createElement('small'); size.textContent = item.source === 'spotify' ? 'Spotify' : `${(item.size / 1024 / 1024).toFixed(1)} MB`;
    button.append(number, name, size); button.onclick = () => control('select', { id: item.id }); li.append(button);
    if (item.source === 'spotify') {
      const link = document.createElement('a'); link.className = 'queue-spotify-link'; link.href = item.url;
      link.target = '_blank'; link.rel = 'noopener noreferrer'; link.textContent = 'Open in Spotify ↗';
      link.setAttribute('aria-label', `Open ${item.name} in Spotify`); li.append(link);
    }
    return li;
  }));
  sync();
}
async function enter(roomCode) {
  const next = await api(`/api/rooms/${roomCode}`);
  events?.close(); code = next.code; state = null; sourceId = null;
  history.replaceState(null, '', `/#${code}`);
  $('lobby').hidden = true; $('room').hidden = false;
  $('connection').textContent = 'Connecting…';
  render(next);
  events = new EventSource(`/api/rooms/${code}/events`);
  events.onmessage = event => { $('connection').textContent = '● Live together'; render(JSON.parse(event.data)); };
  events.onerror = () => { $('connection').textContent = 'Reconnecting…'; audio.pause(); spotify.stop(); };
}
async function control(action, extra = {}) {
  if (!code) return;
  try { await api(`/api/rooms/${code}/control`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action, ...extra }) }); } catch (error) { notice(error.message); }
}
$('create').onclick = async () => {
  $('create').disabled = true;
  try { const room = await api('/api/rooms', { method: 'POST' }); await enter(room.code); } catch (error) { notice(error.message); }
  finally { $('create').disabled = false; }
};
$('join-form').onsubmit = async event => { event.preventDefault(); try { await enter($('code').value.trim().toUpperCase()); } catch (error) { notice(error.message); } };
$('code').oninput = () => { $('code').value = $('code').value.replace(/[^a-f0-9]/gi, '').toUpperCase(); };
$('leave').onclick = () => { events?.close(); events = null; audio.pause(); spotify.stop(); audio.removeAttribute('src'); audio.load(); code = null; state = null; sourceId = null; $('spotify-results').replaceChildren(); $('room').hidden = true; $('lobby').hidden = false; history.replaceState(null, '', '/'); };
$('share').onclick = async () => { try { await navigator.clipboard.writeText(location.href); notice('Invitation link copied. Send it to your people.'); } catch { notice(`Share room code ${code} or copy this page’s address.`); } };
$('enable').onclick = () => {
  if (currentTrack()?.source === 'spotify') {
    if (!spotify.linked) spotify.login(code).catch(error => notice(error.message));
    else spotify.enable();
    return;
  }
  enabled = true; updateSpotifyUI();
  if (state?.current) audio.play().then(() => sync()).catch(() => { enabled = false; updateSpotifyUI(); notice('Upload a playable MP3 or FLAC, then enable audio.'); });
};
$('play').onclick = () => {
  if (!state.playing) {
    if (currentTrack()?.source === 'spotify') { if (spotify.ready && !spotify.enabled) spotify.enable(); }
    else { enabled = true; updateSpotifyUI(); audio.play().catch(() => { enabled = false; updateSpotifyUI(); }); }
  }
  control(state.playing ? 'pause' : 'play');
};
$('next').onclick = () => control('next');
$('seek').onchange = () => control('seek', { position: Number($('seek').value) });
$('volume').oninput = () => { audio.volume = Number($('volume').value); spotify.setVolume(audio.volume); };
audio.onloadedmetadata = () => sync();
audio.onended = () => { if (currentTrack()?.source !== 'spotify' && state?.playing && events?.readyState === EventSource.OPEN) control('ended', { id: sourceId }); };
audio.onerror = () => { if (sourceId) notice('This track could not be played. Try another file or a browser that supports this audio format.'); };
function time(value) { if (!Number.isFinite(value)) return '0:00'; return `${Math.floor(value / 60)}:${String(Math.floor(value % 60)).padStart(2, '0')}`; }
setInterval(() => {
  const track = currentTrack();
  const duration = track?.source === 'spotify' ? track.duration : audio.duration;
  const elapsed = track?.source === 'spotify' ? Math.min(targetPosition(), duration) : audio.currentTime;
  $('elapsed').textContent = time(elapsed); $('duration').textContent = time(duration);
  $('seek').disabled = !Number.isFinite(duration); $('seek').max = Number.isFinite(duration) ? duration : 100;
  if (document.activeElement !== $('seek')) $('seek').value = elapsed;
  if (track?.source !== 'spotify' && state?.playing && enabled && events?.readyState === EventSource.OPEN && audio.readyState >= 2 && Math.abs(audio.currentTime - targetPosition()) > 1.5) sync();
}, 1000);
setInterval(() => { if (currentTrack()?.source === 'spotify' && events?.readyState === EventSource.OPEN) sync(); }, 5000);

function updateSpotifyUI() {
  $('spotify-status').textContent = !spotify.configured ? 'Spotify is not enabled on this server.'
    : !spotify.linked ? 'Link your Premium account to add tracks and listen.'
      : spotify.problem ? spotify.problem
        : !spotify.ready ? `Linked as ${spotify.name}. Connecting player — Premium required…`
        : spotify.enabled ? `Listening as ${spotify.name}` : `Connected as ${spotify.name}. Enable audio to join in.`;
  $('spotify-link').hidden = !spotify.configured || spotify.linked;
  $('spotify-disconnect').hidden = !spotify.linked;
  $('spotify-enable').hidden = !spotify.ready || spotify.enabled;
  $('spotify-search').hidden = !spotify.ready;
  if (currentTrack()?.source === 'spotify') {
    $('enable').hidden = spotify.enabled;
    $('enable').textContent = !spotify.linked ? 'Link Spotify Premium to listen' : 'Enable Spotify audio';
    $('enable').disabled = !spotify.configured || (spotify.linked && !spotify.ready);
  } else { $('enable').hidden = enabled; $('enable').textContent = 'Enable audio on this device'; $('enable').disabled = false; }
}
$('spotify-link').onclick = () => spotify.login(code).catch(error => notice(error.message));
$('spotify-enable').onclick = () => spotify.enable();
$('spotify-disconnect').onclick = async () => {
  try { await spotify.disconnect(); $('spotify-results').replaceChildren(); $('spotify-search-status').textContent = ''; }
  catch (error) { notice(error.message); }
};
$('spotify-search').onsubmit = async event => {
  event.preventDefault();
  const searchRoom = code;
  $('spotify-search-button').disabled = true; $('spotify-search-status').textContent = 'Searching Spotify…'; $('spotify-results').replaceChildren();
  try {
    const { tracks } = await api(`/api/spotify/search?q=${encodeURIComponent($('spotify-query').value.trim())}`);
    if (code !== searchRoom || !spotify.linked) return;
    $('spotify-search-status').textContent = tracks.length ? 'Add a track to the shared queue.' : 'No playable tracks found.';
    $('spotify-results').replaceChildren(...tracks.map(track => {
      const li = document.createElement('li');
      if (track.image) { const image = document.createElement('img'); image.src = track.image; image.alt = ''; li.append(image); }
      const link = document.createElement('a'); link.href = track.url; link.target = '_blank'; link.rel = 'noopener noreferrer'; link.textContent = `${track.name} · ${track.artist} ↗ Spotify`;
      const add = document.createElement('button'); add.className = 'primary'; add.textContent = '+ Add'; add.setAttribute('aria-label', `Add ${track.name} to the room`);
      add.onclick = async () => {
        add.disabled = true;
        try { await spotify.post(`/api/rooms/${searchRoom}/spotify`, { id: track.spotifyId }); add.textContent = 'Added'; }
        catch (error) { add.disabled = false; notice(error.message); }
      };
      li.append(link, add); return li;
    }));
  } catch (error) { $('spotify-search-status').textContent = error.message; }
  finally { $('spotify-search-button').disabled = false; }
};
let uploading = false;
async function upload(files) {
  if (uploading || !code) return;
  uploading = true; $('files').disabled = true;
  const uploadCode = code;
  let count = 0;
  try {
    for (const file of files) {
      if (!/\.(mp3|flac)$/i.test(file.name) || file.size > 50 * 1024 * 1024 || file.size < 4) { notice(`${file.name}: choose an MP3 or FLAC up to 50 MB.`); continue; }
      $('upload-status').textContent = `Uploading ${file.name}…`;
      await api(`/api/rooms/${uploadCode}/tracks?name=${encodeURIComponent(file.name)}`, { method: 'POST', headers: { 'Content-Type': /\.flac$/i.test(file.name) ? 'audio/flac' : 'audio/mpeg' }, body: file });
      count++;
    }
    $('upload-status').textContent = count ? `${count} track${count === 1 ? '' : 's'} added. Good choice.` : '';
  } catch (error) { $('upload-status').textContent = 'Upload interrupted. Please try again.'; notice(error.message); }
  finally { uploading = false; $('files').disabled = false; $('files').value = ''; }
}
$('files').onchange = () => upload([...$('files').files]);
for (const type of ['dragenter', 'dragover']) $('dropzone').addEventListener(type, event => { event.preventDefault(); $('dropzone').classList.add('drag'); });
for (const type of ['dragleave', 'drop']) $('dropzone').addEventListener(type, event => { event.preventDefault(); $('dropzone').classList.remove('drag'); });
$('dropzone').addEventListener('drop', event => upload([...event.dataTransfer.files]));
const initialCode = location.hash.slice(1).toUpperCase();
const spotifyResult = new URLSearchParams(location.search).get('spotify');
if (spotifyResult) {
  history.replaceState(null, '', `/${location.hash}`);
  notice(spotifyResult === 'connected' ? 'Spotify linked. Enable audio to listen.' : spotifyResult === 'premium-required' ? 'Spotify Premium is required to listen.' : 'Spotify linking failed or was cancelled. Try again.');
}
spotify.init().catch(error => notice(error.message));
if (/^[A-F0-9]{6}$/.test(initialCode)) enter(initialCode).catch(error => notice(error.message));
