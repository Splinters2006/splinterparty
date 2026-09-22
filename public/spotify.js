let sdkPromise;
function loadSDK() {
  if (window.Spotify) return Promise.resolve();
  if (!sdkPromise) sdkPromise = new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('Spotify player took too long to load. Refresh and try again.')), 20000);
    window.onSpotifyWebPlaybackSDKReady = () => { clearTimeout(timer); resolve(); };
    const script = document.createElement('script');
    script.src = 'https://sdk.scdn.co/spotify-player.js';
    script.onerror = () => { clearTimeout(timer); reject(new Error('Spotify player could not load. Check your connection or browser settings.')); };
    document.head.append(script);
  });
  return sdkPromise;
}

export class SpotifyPlayback {
  constructor(api, notice, changed) {
    this.api = api; this.notice = notice; this.changed = changed;
    this.configured = false; this.linked = false; this.ready = false;
    this.enabled = false; this.player = null; this.device = null;
    this.desired = null; this.revision = 0; this.retryAt = 0;
  }
  post(url, data = {}) {
    return this.api(url, { method: 'POST', headers: { 'Content-Type': 'application/json', 'X-Splinterparty': '1' }, body: JSON.stringify(data) });
  }
  async init() {
    Object.assign(this, await this.api('/api/spotify/status'));
    this.changed();
    if (this.linked) await this.connect();
  }
  async login(room) {
    const result = await this.post('/api/spotify/login', { room });
    location.assign(result.url);
  }
  async disconnect() {
    const stopped = this.stop();
    await this.post('/api/spotify/disconnect');
    await stopped;
    this.player?.disconnect(); this.player = null; this.connecting = null; this.problem = null;
    this.linked = false; this.ready = false; this.enabled = false; this.device = null;
    this.changed();
  }
  async connect() {
    if (this.connecting) return this.connecting;
    this.connecting = (async () => {
      await loadSDK();
      if (!this.linked) return;
      const player = new window.Spotify.Player({
        name: 'splinterparty', volume: Number(document.getElementById('volume').value),
        getOAuthToken: callback => this.post('/api/spotify/token').then(data => callback(data.accessToken)).catch(error => {
          this.ready = false; this.enabled = false; this.notice(error.message); this.changed();
        }),
      });
      this.player = player;
      player.addListener('ready', ({ device_id }) => {
        if (this.player !== player) return;
        this.device = device_id; this.ready = true; this.retryAt = 0; this.problem = null; this.changed(); this.reconcile();
      });
      player.addListener('not_ready', () => { if (this.player !== player) return; this.device = null; this.ready = false; this.enabled = false; this.problem = 'Spotify device disconnected. Unlink and reconnect if it does not recover.'; this.changed(); });
      player.addListener('autoplay_failed', () => { if (this.player !== player) return; this.enabled = false; this.changed(); this.notice('Click Enable Spotify audio to listen on this device.'); });
      for (const event of ['initialization_error', 'authentication_error', 'account_error', 'playback_error']) {
        player.addListener(event, () => {
          if (this.player !== player) return;
          this.enabled = false;
          if (event !== 'playback_error') this.ready = false;
          this.problem = event === 'account_error' ? 'Spotify Premium is required. Uploads still work without Spotify.' : 'Spotify playback is unavailable. Check your account, browser, and app access, then reconnect.';
          this.changed();
          this.notice(this.problem);
        });
      }
      player.addListener('player_state_changed', state => {
        // Never allow Spotify autoplay to leak into uploads or after leaving.
        if (state && !state.paused && (!this.desired?.playing || !this.enabled)) player.pause().catch(() => {});
      });
      if (!await player.connect()) throw new Error('Spotify could not connect. Try unlinking and linking again.');
    })();
    try { await this.connecting; } catch (error) { this.connecting = null; this.problem = error.message; this.changed(); this.notice(error.message); }
  }
  enable() {
    if (!this.ready) { this.notice('Wait for Spotify to connect, or unlink and reconnect your account.'); return; }
    this.enabled = true; this.retryAt = 0;
    this.player.activateElement().then(() => this.reconcile()).catch(error => { this.enabled = false; this.notice(error.message); this.changed(); });
    this.changed();
  }
  stop() {
    this.desired = null; this.revision++; this.loadedId = null;
    const player = this.player;
    const pause = () => player?.pause().catch(() => {});
    // Wait for an in-flight start request too, so it cannot restart Spotify
    // after uploaded audio has begun.
    return Promise.all([pause(), this.idle]).then(() => { if (!this.desired) return pause(); });
  }
  setVolume(value) { this.player?.setVolume(value).catch(() => {}); }
  update(desired) { this.desired = desired; this.revision++; this.reconcile(); }
  async reconcile() {
    if (this.busy) { this.rerun = true; return; }
    if (!this.player || !this.ready || Date.now() < this.retryAt) return;
    this.busy = true;
    let settle;
    this.idle = new Promise(resolve => { settle = resolve; });
    const revision = this.revision;
    const desired = this.desired;
    try {
      const current = await this.player.getCurrentState();
      if (revision !== this.revision) { this.rerun = true; return; }
      if (!desired?.playing || !this.enabled) {
        if (current && !current.paused) await this.player.pause();
        return;
      }
      const position = Math.max(0, Math.min(desired.position + (performance.now() - desired.receivedAt) / 1000, desired.track.duration - 0.1));
      if (this.loadedId !== desired.track.id || current?.track_window.current_track.uri !== desired.track.uri) {
        await this.post(`/api/rooms/${desired.room}/spotify-play`, { device: this.device, id: desired.track.id });
        this.loadedId = desired.track.id;
      } else {
        if (Math.abs(current.position - position * 1000) > 1500) await this.player.seek(Math.floor(position * 1000));
        if (revision !== this.revision) { this.rerun = true; return; }
        if (current.paused) await this.player.resume();
      }
      if (revision !== this.revision) {
        if (!this.desired?.playing || !this.enabled) await this.player.pause();
        this.rerun = true;
      }
    } catch (error) {
      if (error.status !== 409) { this.retryAt = Date.now() + (error.status === 429 ? 30000 : 15000); this.notice(error.message); }
    } finally {
      this.busy = false;
      this.idle = null; settle();
      if (this.rerun) { this.rerun = false; this.reconcile(); }
    }
  }
}
