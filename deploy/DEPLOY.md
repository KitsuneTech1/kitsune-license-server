# Deploying the Kitsune License Server

Example pattern: unprivileged LXC on a Proxmox host, Caddy forward_auth via
Authentik in front of the admin UI, systemd unit, SQLite at
`/opt/kitsune-license-server/data/`. Swap in your own host, IPs, and domain.

## 1. Create the LXC (on your Proxmox host)
Pick a free CT id and IP on your network (example below, edit for your setup):
```bash
# on the Proxmox host
pct create 128 local:vztmpl/debian-12-standard_*.tar.zst \
  --hostname license --cores 2 --memory 1536 --swap 512 \
  --net0 name=eth0,bridge=vmbr0,ip=192.0.2.71/24,gw=192.0.2.1 \
  --rootfs local-lvm:8 --unprivileged 1 --features nesting=1 --onboot 1
pct start 128
```

## 2. Push source + install (on the LXC)
From this workstation (repo root):
```bash
# copy the source tree in (excluding target/)
rsync -a --exclude target --exclude .git --exclude .demo ./ root@192.0.2.71:/root/kitsune-license-server/
ssh root@192.0.2.71 'bash /root/kitsune-license-server/deploy/install.sh'
```
`install.sh` builds the release binary, creates the `kls` user, writes `.env` with a
generated admin token (printed once, save it), installs the systemd unit, starts it,
and prints the server's public key (embed that in the CLI/guard build).

## 3. Caddy + DNS
See `caddy-license.md`. Add the route to your Caddy config, point your license
subdomain (e.g. `license.example.com`) at your public IP, reload Caddy.

## 4. Verify
```bash
curl https://license.example.com/health           # {"ok":true}
curl https://license.example.com/pubkey            # {"public_key":"..."}
# admin UI: https://license.example.com/  (Authentik login, then paste admin token)
```

## 5. Wire clients
- Build the CLI/guard with the server's public key:
  `KTSN_PUBLIC_KEY=<pubkey> KTSN_SERVER=https://license.example.com` (or bake the key into
  `EMBEDDED_PUBLIC_KEY` in `cli/src/main.rs` for a real release).
- Embeddable end-user device view: `https://license.example.com/embed?key=KTSN-...`.

## Upgrades
Re-run steps 2 (`rsync` + `install.sh`); systemd restarts the rebuilt binary. The DB and
Ed25519 seed in `data/` are preserved, so existing keys/activations and the public key
remain valid.
