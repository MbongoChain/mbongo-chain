# Provider TLS Reference on Linux (P3-C2)

The reference environment for reaching a provider service over a network: nginx
in front, the service on loopback, and an inbound firewall owned by the launcher.
Design: [`P3C_PROVIDER_TRANSPORT_DESIGN.md`](../architecture/P3C_PROVIDER_TRANSPORT_DESIGN.md).
Runner isolation: [`PROVIDER_CONTAINMENT_LINUX.md`](PROVIDER_CONTAINMENT_LINUX.md).

**This is a reference, not a production deployment guide.** It has been tested on
one host, with a second network namespace standing in for a remote client. No
public provider is approved. Other terminators are acceptable if they meet the
requirements below.

## What is claimed, and what is not

| Not claimed | Why |
|---|---|
| A cryptographic link between the TLS certificate and the executor | TLS proves a host name. The link is configuration, and step 5 of the design's identity table is what proves the executor |
| Complete network or IPC isolation | See the containment runbook |
| Protection from a compromised provider service or terminator | Both sit inside the provider's trust boundary, and both see plaintext |
| Readiness for public deployment | No rate limiting, no certificate lifecycle, no capacity testing |
| Interoperability across hosts | One host, two network namespaces. P3-D has not been done |

## Set it up

1. **Certificates.** Run a private CA for this provider, and give clients its
   certificate out of band. Do not use a public CA: clients trust only the anchors
   in their endpoint file.

2. **Firewall and accounts.** List every TCP port this host must accept, SSH
   included. Everything else inbound is dropped, host-wide:

   ```bash
   sudo MBONGO_INBOUND_ALLOW=22,8443 scripts/provider-containment-linux.sh prepare /srv/mbongo
   ```

3. **Service.** Bind it to loopback, as it requires, on a fixed port, and start it
   with the same `MBONGO_INBOUND_ALLOW`. `run` refuses to start if the installed
   policy differs from it.

4. **nginx.** Fill in [`provider-nginx.conf.template`](../../scripts/provider-nginx.conf.template):

   | Placeholder | Value |
   |---|---|
   | `@PREFIX@` | A directory for logs and the pid file |
   | `@LISTEN@` | The address and port clients reach |
   | `@SERVER_NAME@` | The name in the certificate |
   | `@CERTIFICATE@`, `@CERTIFICATE_KEY@` | The server's certificate and key |
   | `@BACKEND@` | The service's loopback address and port |

5. **Client.** Give it an endpoint file and the CA, in the same directory:

   ```json
   {
     "schema": "provider-endpoint-v1",
     "executor": "<64 hex characters>",
     "url": "https://provider.example:8443",
     "trust_anchors": "provider-ca.pem"
   }
   ```

   ```bash
   job_client --provider-endpoint provider.json --job-spec job.json --input input --key-file client.key
   ```

The client's node is its own concern: it needs a node it trusts, and this does not
protect that link.

## What a terminator must do

Requests are signed, and each can be used once. The reference configuration sets
each of these, and any other terminator must meet them:

| Requirement | Reference setting | Evidence |
|---|---|---|
| TLS 1.3 only | `ssl_protocols TLSv1.3` | A client limited to TLS 1.2 is refused |
| Path and body unchanged | `proxy_pass` with no URI, `merge_slashes off` | Signatures verify through nginx, and a job completes |
| No retries | `proxy_next_upstream off` | **Set, not exercised:** with one backend, nginx has nothing to retry to. A replay itself is refused, tested |
| No redirects rewritten | `proxy_redirect off` | The client refuses redirects, tested in C1 |
| Size limit | `client_max_body_size 256k` | A 300 KiB body gets 413 |
| Bodies stay off disk | `client_body_buffer_size 256k`, `proxy_max_temp_file_size 0` | **Set, not tested** |
| Timeouts | `proxy_read_timeout 60s` and the others in the template | **Set, not tested** |

## Host-level tests

`provider_harness --contained`, on the Linux CI job. Synthetic fixtures only.

| Test | Result |
|---|---|
| From the remote namespace | nginx reachable. The backend, a port nobody listed, and a port the runner listens on are not |
| The control, with no policy | The runner's port is reachable, and it accepts the connection |
| A generic job from the remote namespace, through nginx | Anchored receipt, verified at T1 |
| A signed request sent twice through nginx | Passes once, then refused as a replay |
| Query added, body altered | `AUTH_QUERY_NOT_ALLOWED`, `AUTH_BAD_SIGNATURE` |
| Backend stopped mid-job, then restarted | Tracking gets 502, the client recovers, verified at T1 |
| Launcher: missing or malformed `MBONGO_INBOUND_ALLOW` | Refused |
| Launcher: no inbound chain, policy set to accept, an unlisted port | Refused |

## What the inbound policy does not give

- **The service cannot check it.** Proving it needs a vantage point off the host.
  The launcher compares what is installed with what was asked for, at start only.
  Changes after that are not detected, unlike the runner's output policy.
- **TCP only.** The list opens TCP ports. Nothing else new is accepted.
- **Established traffic** is accepted, as any stateful firewall does.
- **One network namespace,** the launcher's. Other firewalls and container runtimes
  are untested.
- **A mistake can lock you out.** Leaving SSH off the list drops new SSH connections.
