# Run a plugin store with Docker

The store's image, `ghcr.io/bitknox/kaeri-store:0.2`, runs on any machine with Docker: a VPS, a
NAS or a computer at home. It is built for x86-64 and 64-bit ARM, and keeps everything in one
volume mounted at `/data`.

Kaeri only joins stores over HTTPS, so the store needs an HTTPS address in front of it. This
folder has two ways to get one:

- `compose.yaml`, for a machine with a domain name. Caddy serves the store over HTTPS and gets
  its certificate from Let's Encrypt.
- `compose.tailscale.yaml`, for a machine with no domain and no open ports, such as one at home.
  Tailscale gives the store an HTTPS address ending in `.ts.net`, and Funnel makes it reachable
  for friends who do not use Tailscale.

If you already run a reverse proxy, use [`docker run`](#behind-your-own-proxy) instead.

## With a domain name

1. Point the domain (for example `plugins.example.com`) at the machine, and open ports 80 and 443.
2. In this folder, write the domain to `.env` and start both containers:

   ```sh
   echo STORE_DOMAIN=plugins.example.com > .env
   docker compose up -d
   ```

3. [Become the owner](#become-the-owner).

Add `STORE_NAME=Our plugins` to `.env` to change the name members see.

## Without a domain, with Tailscale

Funnel needs three things in the [Tailscale admin console](https://login.tailscale.com/admin):

1. On the DNS page, MagicDNS and HTTPS certificates turned on. The same page shows your tailnet's
   DNS name, such as `tail1234.ts.net`.
2. The `funnel` node attribute in the access policy. New tailnets grant it to every member, as
   `"nodeAttrs": [{"target": ["autogroup:member"], "attr": ["funnel"]}]`.
3. An auth key, from Settings, Keys. The store's machine joins your tailnet as you.

Then, in this folder:

```sh
cat > .env <<'EOF'
TS_AUTHKEY=tskey-auth-...
TAILNET=tail1234.ts.net
EOF
docker compose -f compose.tailscale.yaml up -d
```

The store is at `https://kaeri-store.tail1234.ts.net`. The first request can take a few
seconds while Tailscale fetches the certificate. Run every other command in this README with
`-f compose.tailscale.yaml`, then [become the owner](#become-the-owner).

A few things to know:

- The address is built from the machine's name, `kaeri-store` unless `STORE_HOSTNAME` says
  otherwise. If Tailscale shows the machine under another name, such as `kaeri-store-1`, set
  `STORE_HOSTNAME` to that name and run `up -d` again.
- Machines on a tailnet have to log in again after 180 days. Turn off key expiry for this
  machine in the admin console so the store stays reachable.
- To keep the store on your tailnet only, add `STORE_EXPOSE=serve` to `.env`. Friends then need to
  be on your tailnet, or have the machine shared with them, to join.

## Behind your own proxy

```sh
docker run -d --name kaeri-store --restart unless-stopped \
  -p 127.0.0.1:7878:7878 \
  -v kaeri-store:/data \
  -e KAERI_STORE_URL=https://plugins.example.com \
  -e KAERI_STORE_TRUST_PROXY=true \
  ghcr.io/bitknox/kaeri-store:0.2
```

Point the proxy at `127.0.0.1:7878`. `KAERI_STORE_TRUST_PROXY` makes the store believe the
`X-Forwarded-*` headers the proxy sets, which is only safe while the proxy is the only way in, so
the port is published on loopback alone. For the commands below, use `docker exec kaeri-store`
and `docker logs kaeri-store` in place of the `docker compose` forms.

## Become the owner

On its first start, the store prints a one-time owner invite:

```sh
docker compose logs store
```

In Kaeri, go to Providers, press Enter, choose join a store and paste the link. The link works
once, for seven days, and each start prints a new one until somebody uses it. The device that joins
with it owns the store.

From then on you invite people and publish plugins from the store's page in Kaeri. Its admin
page in a browser has the same and a little more; open it with "open the admin page" on the store
in Kaeri, or print a sign-in link here:

```sh
docker compose exec store kaeri-store admin
```

## Invite from the command line

```sh
docker compose exec store kaeri-store invite --name Alex
```

The link works once, for seven days. Add `--owner` to make Alex an owner too.

## Update

```sh
docker compose pull
docker compose up -d
```

The store brings its database up to date when the new version starts, and refuses to start on a
database a newer version has used, so going back means restoring a backup.

The `0.2` tag follows every 0.2.x release. A new minor version can change things, so it needs the
tag changed by hand, after reading its release notes. To stay on one release, use a full version such as
`ghcr.io/bitknox/kaeri-store:0.2.0` in the compose file.

The store's admin page can tell you when a new release is out: open This store and choose Check
for updates. The check runs in your browser and asks GitHub, not the store.

## Back up

Everything is in the store's volume: members, invites, plugins and the key that signs the store's
plugin list. Keep backups private. To archive it, stop the store for a moment:

```sh
docker compose stop store
docker run --rm --volumes-from "$(docker compose ps -aq store)" -v "$PWD:/backup" busybox \
  tar czf /backup/kaeri-store.tar.gz -C /data .
docker compose start store
```

To restore, run the same commands with `sh -c 'rm -rf /data/* && tar xzf /backup/kaeri-store.tar.gz -C /data'`
in place of the `tar czf` line.

## A folder instead of a volume

The store runs as user 65532, not root. A named volume, as above, gets the right owner by itself.
If you mount a folder from the host instead, which is common on a NAS, give it to that user first:

```sh
mkdir -p data && sudo chown 65532:65532 data
```
