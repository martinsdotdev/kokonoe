# Deploying Kokonoe

Kokonoe runs on one Hetzner Cloud server. Docker Compose runs three containers on it:

- the app, built from the repo's `Dockerfile`;
- Postgres 18, whose data is in the `db` volume;
- Caddy, which serves HTTPS with a Let's Encrypt certificate.

CI's Deploy job ships every merge to master that passes its checks. It builds the image, loads it on the server over SSH as the `deploy` user, and runs `deploy.sh`.

## Settings

The GitHub environment `production`, which only master may use, holds these settings:

| Name | Kind | Holds |
| --- | --- | --- |
| `DOMAIN` | variable | the site's name, which Caddy gets a certificate for |
| `DEPLOY_HOST` | variable | the server's IPv4 address |
| `DEPLOY_SSH_KEY` | secret | the `deploy` user's private key |
| `DEPLOY_KNOWN_HOSTS` | secret | the server's host keys, from `ssh-keyscan` |

The server's `/opt/kokonoe/.env` holds `DOMAIN`, `POSTGRES_PASSWORD`, and the `IMAGE` that's running.

## Making the server

1. Make it with the `hcloud` CLI:
   - Fill in `${DOMAIN}` and `${DEPLOY_KEY}` in `cloud-init.yaml`.
   - Pass that as `--user-data-from-file` to `hcloud server create`, with an admin key as `--ssh-key` for root.
   - Attach a firewall that lets in only SSH, HTTP and HTTPS.
2. Point the domain's A record at the server's address.
3. Set the environment's variables and secrets.

The next merge to master deploys.

## Running it

From `/opt/kokonoe`, as `deploy`:

- `docker compose logs -f app` shows the app's logs.
- `docker compose exec db psql -U kokonoe` opens the database.
- `IMAGE=kokonoe:<sha> docker compose up -d` rolls back to an earlier image. `deploy.sh` keeps the three newest.
