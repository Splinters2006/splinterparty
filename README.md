# Splinterparty

Splinterparty is a small Rust fileserver backend for serving a local directory or mounted partition over HTTP.

## Setup

Run the interactive setup:

```bash
cargo run -- setup
```

Setup asks for:

- the directory path to serve
- the bind address, defaulting to `0.0.0.0:8080`
- whether to configure router port forwarding through UPnP
- whether to require username and password authentication

The default username and password are both `admin`. Setup lets you change both values.

Setup saves local machine settings in `splinterparty.conf`. That file is ignored by git because it contains machine-specific paths and credentials.

## Run

After setup:

```bash
cargo run
```

Serve a directory directly without using saved config:

```bash
cargo run -- /mnt/storage 0.0.0.0:8080
```

Serve directly and request UPnP port forwarding:

```bash
cargo run -- /mnt/storage 0.0.0.0:8080 --port-forward
```

Downloads support HTTP byte ranges, so browsers and media clients can resume downloads and seek within large files.

File responses include weak ETags and support `If-None-Match`, so clients can avoid re-downloading unchanged files.

Directory pages render a built-in browser UI with item counts, file types, sizes, modified times, large-file labels, and download links.

The browser UI can create protected folders and upload files from your device. Files do not have their own PINs; uploading into a protected folder requires that directory to be unlocked with its read+write PIN.

## Commands

```bash
cargo run -- --help
cargo run -- config
cargo run -- hash <file>
cargo run -- dedup [root]
cargo run -- split-large <file-or-directory>
cargo run -- reassemble <manifest>
```

The `config` command prints the saved config summary but hides the password.

The `hash` command prints a file's SHA-256 hash.

The `dedup` command scans a directory, groups files by size, hashes files that could be duplicates, and reports duplicate groups. It does not delete or modify files.

The `split-large` command classifies files over 100 MiB as large files and splits them into 100 MiB parts. Each part gets its own SHA-256 hash in a `manifest.txt` file under `<filename>.parts/`.

The `reassemble` command reads a split manifest, verifies every part hash and size, then rebuilds the original file.

## Install

Run:

```bash
./install.sh
```

The installer uses the system package manager when available to install build prerequisites, installs Rust through rustup if Cargo is missing, and builds the release binary.

## Port Forwarding

Automatic port forwarding is best effort. It only works when the network router supports UPnP IGD and allows `AddPortMapping` requests. If the router rejects the request or UPnP is disabled, configure the router manually.

## Security

When serving outside your own machine, keep authentication enabled and change the default credentials during setup.

Requests are resolved and checked against the served root, so `..` paths and symlinks cannot be used to escape into the rest of the filesystem.
