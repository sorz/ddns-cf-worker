# ddns-cf-worker

Run your own DDNS service with Cloudflare Worker.

Once deployed, it provides a simple HTTP GET endpoint to update DNS records.

```mermaid
flowchart LR
    A[Your device] -->|GET /update| B[CF Worker]
    B -->|CF API| C[Update DNS Zone]
```

The main goal is to provide an interface that is simple enough to be
compatible with home routers & other legacy devices.

The one-password-per-hostname model also reduces the risk of leaking tokens
compared to calling the Cloudflare API directly from the devices.

## Deployment

1. Create KV store for hostname & password
  ```bash
  npx wrangler kv namespace create ddns-host-password
  ```

  Add hostname (key) and password (value) to the KV store using either
  `wrangler` or Cloudflare dashboard.

2. Edit `wrangler.toml`, specifically
  - `DDNS_CF_API_TOKEN`: Cloudflare API token used to update your zone
  - `DDNS_CF_ZONE_ID`: copy from Cloudflare dashboard
  - `DDNS_DOMAIN_SUFFIX`: FQDN of DDNS domains, besides their hostname
  - `[[routes]] pattern`: domain name of `/update` endpoint
  - `[[kv_namespaces]] id`: KV store id got from the last step

3. Deploy
  ```bash
  npx wrangler deploy
  ```

## Usage

Assume you have `example.com` on Cloudflare, set both `DDNS_DOMAIN_SUFFIX`
and `[[routes]] pattern` to `dyn.example.com`, and added `home=pwd123` to
the KV store.

The update endpoint will be `https://dyn.example.com/update`, and your
DDNS name will be `home.dyn.example.com`.

For maximum compatibility, it accepts a range of parameter names and auth:

- HTTP Basic Auth: `home:password`
- Credential in queries: `?hostname=home&password=pwd123`
- Hostname (`home`) or FQDN (`home.dyn.example.com`) as username
- Addresses in `?ip=` or `?myip=` queries
- No `?ip=` at all: use client's public IP address

Following are some examples:

### Update to current IP address
```
$ curl https://dyn.example.com/update -u home:password
> success

$ dig +short home.dyn.example.com
> 198.51.100.5  # <- Your public IP address
```

### Multiple address, IPv4 & IPv6

Specify all addresses in a single request to prevent them from overwriting each other.

```bash
curl "https://dyn.example.com/update?ip=2001:DB8::1968:08:17&ip=192.0.2.8&ip=198.51.100.5" -u home:password
```

## See also

[sorz/nsupdate-web](https://github.com/sorz/nsupdate-web) if you like
self-host both the web and the DNS server (bind9).
