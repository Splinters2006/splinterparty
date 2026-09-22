# splinterparty

A server-hosted MP3 and FLAC listening room with six-digit hexadecimal invitation codes, shared uploads and queue, synchronized play/pause/seek, automatic next track, and local volume controls. No accounts or dependencies.

## Run

Requires Node.js 22 or newer.

```sh
npm start
```

Open http://localhost:3000. Create a room and share its link or code. Anyone with the code can upload and control playback. Each listener must enable audio on their device. To access from another device on your network, use the server's LAN address instead of localhost.

If port 3000 is already in use, run `PORT=3001 npm start` and open http://localhost:3001 instead.

`PORT` (default `3000`) and `HOST` (default `0.0.0.0`) configure the listener. For internet hosting, put this single Node process behind an HTTPS reverse proxy. Disable proxy buffering for `/api/rooms/*/events`, allow long-lived SSE connections, and allow request bodies up to 50 MB. No build step is needed.

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

Integration tests cover room codes, upload validation, audio byte ranges, shared playback events, and duplicate end-of-track notifications.
