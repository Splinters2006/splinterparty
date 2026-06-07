# Splinterparty

Splinterparty is a lightweight Rust fileserver for sharing directories over HTTP with support for protected shares, uploads, downloads, resumable transfers, and optional UPnP port forwarding.

## Features

* Directory browser UI
* File uploads from the browser
* Protected shares with read-only and read/write PINs
* File deletion support
* Resumable downloads (HTTP Range requests)
* ETag caching support
* Duplicate file detection using SHA-256 hashes
* Automatic filename conflict resolution
* Optional UPnP port forwarding
* Systemd user service support

## Installation

Clone the repository:

```bash
git clone https://github.com/Splinters2006/splinterparty.git
cd splinterparty
```

Run the installer:

```bash
./splinterparty.sh all
```

Or perform the steps manually:

```bash
./splinterparty.sh install
./splinterparty.sh setup
./splinterparty.sh service-install
```

The installer:

* Installs required build dependencies
* Installs Rust if necessary
* Builds Splinterparty in release mode
* Installs the binary to:

```bash
~/.local/bin/splinterparty
```

## Service Management

Check service status:

```bash
./splinterparty.sh status
```

View logs:

```bash
./splinterparty.sh logs
```

Restart the service:

```bash
./splinterparty.sh restart
```

Remove the service:

```bash
./splinterparty.sh service-remove
```

## Protected Shares

A protected share can have:

* Read PIN
* Read + Write PIN

Users with the read PIN can:

* Browse files
* Download files

Users with the read + write PIN can:

* Upload files
* Delete files
* Create folders

Files outside protected shares can be deleted without entering a PIN.

## Duplicate Files

Files with identical names may coexist if their contents differ.

Example:

```text
photo.jpg
photo (1).jpg
photo (2).jpg
```

When a file is uploaded:

* Splinterparty computes its SHA-256 hash.
* If a file with the same name already exists but has different content, a new numbered filename is generated automatically.
* If a file with identical content already exists, the upload is rejected.

## Setup

Run:

```bash
~/.local/bin/splinterparty setup
```

Setup asks for:

* Directory to serve
* Bind address
* UPnP port forwarding
* Authentication settings

Configuration is stored in:

```bash
splinterparty.conf
```

## Security

* Directory traversal attacks are blocked.
* Symlink escapes outside the shared root are prevented.
* Protected shares require PIN authentication.
* Write operations require the read/write PIN.
* Authentication can be enabled for the entire server.

## Port Forwarding

UPnP port forwarding is best effort.

If your router does not support UPnP or rejects requests, configure port forwarding manually.
