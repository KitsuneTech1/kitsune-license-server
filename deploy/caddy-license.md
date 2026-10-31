# Caddy route for the license server

If you run Caddy from a JSON config (an override forcing `--config caddy.json`),
merge this as a route into that JSON. The intent as a Caddyfile:

```caddyfile
license.example.com {
    # public carve-outs: phone-home, end-user device view, health/pubkey
    @public path /activate /validate /heartbeat /devices /pubkey /health /embed
    handle @public {
        reverse_proxy 192.0.2.71:8080
    }

    # everything else (admin UI "/" + "/admin", admin API "/api/*")
    # behind Authentik forward_auth like your other internal apps
    handle {
        forward_auth 192.0.2.64:9000 {
            uri /outpost.goauthentik.io/auth/caddy
            copy_headers X-Authentik-Username X-Authentik-Groups X-Authentik-Email
        }
        reverse_proxy 192.0.2.71:8080
    }
}
```

Notes
- `192.0.2.71:8080` is the license-server LXC. Point it at wherever you deployed it.
- The Authentik proxy/forward-auth outpost is whatever instance you already run for
  other internal apps.
- TLS: Caddy auto-certs `license.example.com` once DNS resolves to your public IP and
  inbound 80/443 reach the Caddy host.
- After editing the Caddy config, reload it (`systemctl reload caddy` or your project's
  reload hook).

## DNS
Add at your DNS provider:
```
license.example.com.  A  <your public IP>
```
(or a CNAME to an existing host that already points there). If you already have a
wildcard record for your domain, no new record is needed.

## Public vs protected, endpoint summary
| Endpoint | Access |
|---|---|
| `POST /activate`, `/validate`, `/heartbeat` | public (clients) |
| `POST /devices`, `GET /embed`, `/pubkey`, `/health` | public (read-only/info) |
| `GET /`, `/admin`, `GET/POST /api/*` | Authentik + admin bearer token |
