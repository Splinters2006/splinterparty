# splinterparty

A server-hosted MP3 and FLAC listening room with six-digit hexadecimal invitation codes, shared uploads and queue, synchronized play/pause/seek, automatic next track, and local volume controls. No accounts needed for uploads and no external Node dependencies.

Optional Spotify integration lets listeners link their own Premium accounts, search or paste track links, add Spotify tracks to the shared queue, and follow the room's playback through Spotify's browser player.

## Run

Requires Node.js 22 or newer.

```sh
npm start
```

Open http://localhost:3000. Create a room and share its link or code. Anyone with the code can upload and control playback. Each listener must enable audio on their device. To access from another device on your network, use the server's LAN address instead of localhost.

If port 3000 is already in use, run `PORT=3001 npm start` and open http://localhost:3001 instead.

`PORT` (default `3000`) and `HOST` (default `0.0.0.0`) configure the listener. For internet hosting, put this single Node process behind an HTTPS reverse proxy. Disable proxy buffering for `/api/rooms/*/events`, allow long-lived SSE connections, and allow request bodies up to 50 MB. No build step is needed.

## Spotify setup

1. Create an app in the [Spotify Developer Dashboard](https://developer.spotify.com/dashboard) with Web API and Web Playback SDK access. Development Mode currently requires the app owner to have Premium and supports up to five authorized users; add your listeners in the dashboard. Each listener needs their own Premium account.
2. Serve splinterparty at an **HTTPS URL**. Plain HTTP on a LAN/public IP will still work for uploads, but will not work for Spotify linking. Local development may use `http://127.0.0.1:3000` (not `localhost`).
3. Copy `.env.example` to `.env`, set `SPOTIFY_CLIENT_ID` to the app's client ID, and set `PUBLIC_URL` to the exact origin listeners open, such as `https://party.example.com`. Set `PORT` to your usual server port. No client secret is needed: authorization uses PKCE.
4. Register the exact callback URL in your Spotify app, such as `https://party.example.com/api/spotify/callback`.
5. Restart with `npm start`. The server automatically reads `.env`; existing environment variables take precedence. `.env` is ignored by Git and is preserved by `update.sh`.
6. In a room, choose **Link Spotify**, approve access, and choose **Enable Spotify audio**. Once the Spotify player connects, search by track/artist or paste a Spotify track URL/URI, then select **+ Add**.

The room shares play, pause, seek, selection, and next-track actions. Spotify playback uses each listener's account directly and can move playback away from another device on that same account. Unlinked listeners see Spotify metadata and the room timeline but hear no Spotify audio; uploads still play for them. Browser support, autoplay permission, track availability in each listener's country, and Spotify rate limits can affect playback. Synchronization is approximate, with drift corrected periodically. Spotify tracks advance using the server timeline. Uploaded and Spotify audio never intentionally overlap.

Premium eligibility is enforced by Spotify's browser player. The app also rejects non-Premium profiles when Spotify returns the `product` field; new Development Mode apps may no longer receive that field. A linked account is not considered playback-ready until the SDK connects. If access fails, check Premium, dashboard user access, HTTPS, and browser support; unlink and link again if authorization has expired.

Sessions and refresh tokens live only in server memory, expire after 24 hours, and disappear on restart. Unlinking deletes the session and profile; it does not remove public song metadata already added to a room. A user-facing [privacy page](/privacy.html) describes data handling and revoking access. Adapt it if you add analytics or other hosting data collection. This integration has no server-side Spotify audio downloading or rebroadcasting. Review [Spotify's developer policy](https://developer.spotify.com/policy) for your deployment, including restrictions on broadcasting and combining services; separate Premium streams are not a guarantee of Spotify approval for a group-listening app.

References: [Web Playback SDK](https://developer.spotify.com/documentation/web-playback-sdk/reference), [redirect URI requirements](https://developer.spotify.com/documentation/web-api/concepts/redirect_uri), [Development Mode limits](https://developer.spotify.com/documentation/web-api/concepts/quota-modes), [2026 API changes](https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide).

## Update

For an installation cloned from this repository, run:

```sh
./update.sh
```

The script works from any directory, fetches `main` from `origin`, and applies only fast-forward updates. It stops if you have local changes, untracked files, a different branch, or local commits that are not on the remote. It never resets or overwrites your work. Git and Bash are required; there are no dependencies to install or build steps.

After updating, restart the server using your usual command (for example, `PORT=3001 npm start` after stopping the old process). The script does not stop or restart processes. Restarting clears active rooms and uploads.

If your existing checkout predates the script, run `git pull --ff-only origin main` once to get it. For a fresh installation:

```sh
git clone https://github.com/Splinters2006/splinterparty.git
cd splinterparty
npm start
```

## Behavior and limits

- The server owns the playback timeline; server-sent events broadcast changes, and clients correct drift. Synchronization is approximate, with network latency and browser buffering affecting alignment.
- Rooms and files are temporary. Rooms with no connected listeners or activity for 24 hours expire. Restarting the process clears rooms. Uploads live in a private OS temporary directory, removed on graceful shutdown; forced process termination may leave an orphaned temporary directory.
- MP3 and FLAC files are limited to 50 MB each, 50 tracks per room, 1 GB total stored files, 100 rooms, and 100 connected listeners per room. Upload validation checks file signatures, not complete audio decoding. FLAC files are served directly as `audio/flac`; playback requires browser support for FLAC (no server transcoding).
- Room codes are invitations, not strong authentication. Anyone who knows a code has equal access. Before running an unrestricted public service, add per-IP rate limiting and any authentication/moderation your deployment needs.
- Run one server process: room state is in memory. Multiple instances require shared state, storage, and pub/sub.

## Tests

```sh
npm test
```

Integration tests cover room codes, upload validation, audio byte ranges, shared playback events, duplicate end-of-track notifications, and Spotify authorization and playback using mocked provider responses. Live Spotify playback requires configured app credentials and Premium test accounts.
