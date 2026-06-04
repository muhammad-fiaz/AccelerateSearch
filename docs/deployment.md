# Deployment

## Single-node (default)

```bash
# Build
cargo build --release

# Run
./target/release/accelerate
```

A `data/` directory is created on first run and holds the embedded
`redb` store, snapshots, daily logs, and the API key store.

## TLS

Generate a self-signed cert for local development:

```bash
mkcert -install
mkcert accelerate.local
```

Set in `config/default.toml`:

```toml
[server]
host = "0.0.0.0"
port = 7700

[server.tls]
enabled = true
cert_path = "./accelerate.local.pem"
key_path = "./accelerate.local-key.pem"
```

For production, use certs from Let's Encrypt or your corporate CA.

## systemd unit

```ini
# /etc/systemd/system/accelerate.service
[Unit]
Description=AccelerateSearch
After=network.target

[Service]
Type=simple
User=accelerate
WorkingDirectory=/var/lib/accelerate
Environment=ACCELERATE_CONFIG=/etc/accelerate/config.toml
ExecStart=/usr/local/bin/accelerate
Restart=on-failure
RestartSec=5
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
```

```bash
sudo useradd -r -s /usr/sbin/nologin accelerate
sudo install -m755 target/release/accelerate /usr/local/bin/
sudo install -d -o accelerate -g accelerate /var/lib/accelerate
sudo install -d -m755 /etc/accelerate
sudo cp config/default.toml /etc/accelerate/config.toml
sudo systemctl daemon-reload
sudo systemctl enable --now accelerate
```

## Docker

```bash
docker build -t accelerate:local .
docker run --rm -p 7700:7700 -v "$PWD/data:/data" accelerate:local
```

`docker-compose.yml` is provided in the repository root for a
single-command bring-up that exposes port 7700.

## Reverse proxy

### nginx (with TLS termination)

```nginx
server {
    listen 443 ssl http2;
    server_name accelerate.example.com;

    ssl_certificate     /etc/letsencrypt/live/accelerate.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/accelerate.example.com/privkey.pem;

    client_max_body_size 10m;

    location / {
        proxy_pass http://127.0.0.1:7700;
        proxy_set_header Host              $host;
        proxy_set_header X-Real-IP         $remote_addr;
        proxy_set_header X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

## Capacity planning

A single node comfortably handles:

* 5 M documents × 1 KB each
* 30 K QPS search (cache-warm, single collection)
* 5 K QPS index (small batch updates)

For higher throughput, scale horizontally behind a load balancer. The
embedded `redb` backend is single-node only; use the `network`
cluster skeleton in `crates/cluster` to coordinate a fleet.

## Documentation site (GitHub Pages)

The user guide is an [mdbook](https://rust-lang.github.io/mdBook/) site
that is published to GitHub Pages via the
`.github/workflows/docs.yml` workflow.

### One-time setup

1. Create the GitHub repository `AccelerateSearch` (the URL slug must
   match the directory name for the project page to live at
   `muhammad-fiaz.github.io/AccelerateSearch/`).
2. In **Settings → Pages**, set **Source** to **GitHub Actions**.
3. (Optional) Configure a custom domain in **Settings → Pages →
   Custom domain** and add a `CNAME` file in `site/` from the workflow.
4. Grant the workflow the `pages: write` and `id-token: write`
   permissions (already declared in the workflow file).

### Build locally

```bash
# One-off: install the mdbook binary
cargo install mdbook --locked --version 0.4.43

# Build the user guide into ./docs/book/
(cd docs && mdbook build)

# Build cargo doc into ./target/doc/
cargo doc --no-deps --workspace --target-dir target

# Combine both into a single ./site/ directory ready for Pages
mkdir -p site
cp -R docs/book/. site/
mkdir -p site/rust-api
cp -R target/doc/. site/rust-api/
touch site/.nojekyll
```

### Deploy

Every push to `main` rebuilds and deploys the site. To force a rebuild
without a code change, go to the **Actions** tab, select **Docs**, and
click **Run workflow**.

The site is served at <https://muhammad-fiaz.github.io/AccelerateSearch/>.

## Backup and restore

```bash
# Snapshot
curl -X POST -H "Authorization: Bearer $MASTER" \
     http://localhost:7700/api/v1/snapshots \
     -d '{"name":"nightly-2026-06-03"}'

# Download the underlying files (tar+zstd snapshot)
curl -O -H "Authorization: Bearer $MASTER" \
     http://localhost:7700/api/v1/snapshots/nightly-2026-06-03

# Restore
curl -X POST -H "Authorization: Bearer $MASTER" \
     http://localhost:7700/api/v1/snapshots/nightly-2026-06-03/restore
```

Always stop the server (or pause writes via `--read-only`) before
restoring, to avoid in-flight write conflicts.

## Health checks

```bash
curl -fsS http://localhost:7700/health
# {"status":"available"}
```

Configure your orchestrator to restart the container on a non-200
response.

## Upgrades

1. Drain writes (optional): set `[search] readonly = true`.
2. Stop the old binary.
3. Install the new binary.
4. Start the new binary; the embedded store is upgraded in place.
5. Re-enable writes.

Schema migrations live in `crates/storage` and run automatically on
startup. They are additive and idempotent; a rollback to a previous
version is always supported.
