# P3-C: Secure Transport Between Clients and Providers

**Status: design approved (2026-11-05), C1 and C2 implemented.** The service stays loopback-only. No provider is
exposed, and P3-D is not started.

These choices define the **reference implementation**. They are not exclusive
requirements for every future compatible provider.

**Goal:** an independently operated client reaches an independently operated
provider confidentially, reaches the provider it meant to, and fails safely
otherwise. No new workload.

**Protected surfaces:** no protocol, consensus, RPC, SDK or `job/` change. The
signing format `mbongo-provider-http:v1` is unchanged.

## 1. What TLS adds, and what it does not

| Threat | Today | With P3-C |
|---|---|---|
| Reading the input on the wire | Exposed off loopback | Protected up to the terminator |
| Altering a request | Refused by the signature | Unchanged |
| Replaying a request | Refused, durably | Unchanged |
| Forging or altering a status or an error | Possible | Protected up to the terminator |
| Forging a result | Refused: bytes must match the executor's anchored receipt | Unchanged |
| Reaching the wrong host | Undetected until the capability check | Refused at the handshake |
| A dishonest provider | Not addressed | **Not addressed.** Evidence stops at T1 |

Results never depended on the transport: the client checks them against the
chain. TLS protects confidentiality in transit, and the status and error channel.

The terminator sits **inside the provider's trust boundary**. It sees plaintext,
as the provider does. This is `PUBLIC` confidentiality, not client-to-runner
encryption.

## 2. Architecture

```text
client ── TLS ──> terminator ── loopback HTTP ──> provider_service ──> runner
          host A   |  provider's host, provider's responsibility  |  no network
```

| Component | Owner | Role |
|---|---|---|
| Terminator | Provider | TLS, certificate, connection limits. Forwards bytes unchanged |
| `provider_service` | Provider, code from MbongoChain | Authenticates every request. Stays loopback-only, as it already enforces |
| `ProviderClient` | Client, code from MbongoChain | Verifies the endpoint and signs for one executor |
| Trust and endpoint file | Client | The out-of-band mapping |

## 3. Identity: executor to endpoint

Decision 3 of P3 is out-of-band configuration. The client holds one strict file
per provider:

```json
{
  "schema": "provider-endpoint-v1",
  "executor": "<64 hex characters>",
  "url": "https://provider-a.internal:8443",
  "trust_anchors": "provider-a-ca.pem"
}
```

Each step proves one thing, and none stands in for another:

| Step | When | Proves | Failure |
|---|---|---|---|
| 1. TLS handshake, against `trust_anchors` only | Connection | The host holds a certificate this client was told to trust, for that name | `PROVIDER_TLS_REFUSED` |
| 2. `GET /v1/capabilities`, executor compared | Before any upload | Configuration agrees. **A claim, not a proof**: it catches a wrong file, not an attacker | `WrongProvider` (exists) |
| 3. Recipient-bound signature | Every request | A request is only good for this executor | `AUTH_WRONG_RECIPIENT` (exists) |
| 4. Task names the executor on-chain | Commitment | Only this executor can receive it | Chain rule (exists) |
| 5. Receipt signed by the executor, bytes match | Result | T1 | `Verification` (exists) |

**Nothing cryptographically ties the TLS certificate to the executor key.** An
operator holding a trusted certificate could front another executor until step 5
fails. P4's signed declaration is the place to close this.

## 4. Certificates and trust

- **Trust anchors are per provider.** The client's root store is built from this
  file and nothing else, so a public CA cannot vouch for a provider. The anchors
  must sit beside the endpoint file, so it cannot point at a system bundle. They
  hold a CA, not a leaf, so a provider can rotate its certificate without touching
  clients.
- **Host names are verified.** There is no option to turn verification off.
- **Minimum TLS 1.3.** Both ends are ours or the provider's, so no older client
  needs supporting.
- **Anchor rotation** is out of band: the client replaces the file. Revocation is
  the same step. No OCSP or CRL in P3-C.
- **A client must refuse:**
  - `http://` to anything but a loopback address (`PROVIDER_INSECURE_URL`). A name
    is refused too, even `localhost`: it can resolve elsewhere;
  - **redirects**: the client follows them by default today, which could send a
    signed request and its input elsewhere. They are turned off;
  - proxies, which are off already.

## 5. Request integrity and replay

The signature already covers recipient, method, path, body digest, validity
window and request identifier. Replay protection is durable and fails closed
(P1). P3-C keeps both, and closes what a proxy in the path makes relevant:

| Point | Rule |
|---|---|
| Query string | Not signed, and no route uses one. The service **refuses** a request carrying one |
| Path | Must reach the service byte for byte. A terminator that normalizes it fails with `AUTH_BAD_SIGNATURE`: safe, and a configuration error |
| Other headers, `Host`, `X-Forwarded-*` | Not signed. The service must not act on them, and does not read them |
| Terminator retries | **Off.** A retried request is a replay, and is refused |
| Client retries | Always a fresh signature and identifier. The client speaks HTTP/1.1, so that nothing below it resends a signed request |
| Clock | Unchanged: a 120 s window, 30 s skew. A skewed client fails with `AUTH_NOT_YET_VALID` or `AUTH_EXPIRED` |

## 6. Backend and host

**Backend.** The service already refuses a non-loopback `bind`. Any local process
can reach it, but must still sign. The runner cannot (P3-B). The service does not
trust the terminator: nothing in its authentication depends on where a request
came from.

**Inbound filtering for runner processes.** P3-B filters output only, so a remote
peer can connect to a port the runner listens on. Once a host is reachable, that
matters. `meta skuid` cannot match inbound packets, because no socket owner is
known at that point.

| Option | For | Against |
|---|---|---|
| **Host input policy: refuse new inbound connections except listed ports (recommended)** | Simple, covers every process, independent of the runner | Host-wide. The operator must list what it needs, such as SSH |
| Match the socket's cgroup on input | Targets runners only | Job cgroups are named per job, it depends on newer kernels and `nftables`, and it leaves other local services as they are |

The launcher would own this as one fixed chain in its existing table, with
`MBONGO_INBOUND_ALLOW` listing ports. It would refuse to start without that list,
so a provider cannot lock itself out by accident or skip it by omission. This is
not a firewall framework: no per-job rules, and no other policy.

**Limit:** the service cannot prove this at runtime, because proof needs a vantage
point off the host. At start, the launcher compares what is installed with what was
asked for, and the Linux test proves the effect from a separate network namespace.
The reference is [`PROVIDER_TLS_REFERENCE_LINUX.md`](../runbooks/PROVIDER_TLS_REFERENCE_LINUX.md).

## 7. Failure handling

No failure falls back to a weaker path, and none sends the input.

| Failure | Client sees | Retried |
|---|---|---|
| Untrusted, expired or wrong-name certificate | `PROVIDER_TLS_REFUSED` | No |
| `http://` off loopback | `PROVIDER_INSECURE_URL` | No |
| A redirect | `PROVIDER_REDIRECT_REFUSED`. Not followed | No |
| Malformed endpoint file | Refuses to start | No |
| Capabilities name another executor | `WrongProvider`, before any upload | No |
| `AUTH_WRONG_RECIPIENT` | `Rejected` | No |
| Terminator down, connection refused | `Unavailable` | While polling, until the timeout |
| Backend down: 502, 503, 504, on tracking, the result or registration | `Rejected`, treated as transient | Until the timeout, each with a fresh signature |
| The same, on the job spec or the input | `Rejected` | No. The job ends, and nothing reaches the chain |
| Replay refused | `Rejected` | Only with a new identifier |
| Timeout, including a request that never answers | `TimedOut`, at the client's wait, not later | The task stays on-chain. Recovery is P5 |

Errors carry codes, not content. The client's key never leaves it, and neither key
material nor input reaches a log.

## 8. Linux reference configuration

- **Terminator:** nginx is recommended as the harder case. If signatures survive an
  HTTP proxy, they survive a TCP tunnel. Required settings: TLS 1.3, `proxy_pass`
  with no URI, `proxy_next_upstream off`, `client_max_body_size` at or above the
  service's 256 KiB, and no header the service would read.
- **Any other terminator is acceptable** if it meets section 5, and the suite below
  can test that.
- **Certificates:** a private CA for tests, generated at test time. No key is
  committed.
- `docs/runbooks/` gains the steps. No deployment is implied.

## 9. Tests

**Portable, Linux and Windows CI.** `ProviderClient` against a TLS server built in
the test. `rcgen` and `tokio-rustls` are already in `Cargo.lock`, so no new crate is
expected:

| Case | Expected |
|---|---|
| Valid chain, full job | Verified at T1 |
| Unknown CA, expired, wrong name | `PROVIDER_TLS_REFUSED`, no request received |
| TLS 1.2 only | Refused |
| `http://` off loopback | `PROVIDER_INSECURE_URL` |
| Redirect to another host | `PROVIDER_REDIRECT_REFUSED`, nothing sent to the second host |
| Capabilities name another executor | `WrongProvider`, no upload received |
| 502, 503, 504 while polling | Keeps polling, then succeeds |

**Linux, with a real terminator.** A second network namespace on the same host
stands in for a remote client. It proves traffic crosses a real interface. It is
**not** P3-D, and proves nothing about separate hosts.

| Case | Expected |
|---|---|
| Full job through nginx, from the namespace | Verified at T1 |
| Backend port, from the namespace | Unreachable |
| A port the runner listens on, from the namespace | Unreachable. Reachable without the input rule, as the control |
| Terminator retry forced | Refused as a replay |
| Query string added; path altered; body altered | Refused, each with its code |
| Backend stopped mid-job, then restarted | Client recovers, T1 |
| Launcher with no inbound list | Refuses to start |

Existing P2, P3-A and P3-B checks must keep passing.

## 10. Stages, if approved

| Stage | Content |
|---|---|
| **C1, client and service** | Endpoint file, TLS trust, no redirects, bounded retries on gateway failures, query refusal, portable tests |
| **C2, reference and host** | Inbound policy in the launcher, nginx configuration, runbook, Linux tests |

Each is one PR. C1 needs no host change.

## 11. What stays unresolved

P3-C does not change any of these, and claims nothing about them:

- Unix-domain socket and other IPC access.
- Network policy tampering during a job.
- A compromised provider service, or terminator.
- Isolation on Windows.
- Evidence beyond T1: provider honesty is not verified.
- A cryptographic link between certificate and executor (P4).
- Availability: rate limiting and denial of service are the provider's.
- **The client's link to its node.** T1 relies on what the node reports about
  inclusion, so a client must trust its node, or run its own. A remote node needs
  its own protection. Out of scope here (decision 6).

## 12. Decisions (2026-11-05)

| # | Decision |
|---|---|
| 1 | nginx is the reference TLS terminator |
| 2 | `rustls` is the reference Rust client's TLS |
| 3 | Provider-specific trust anchors, not system roots, in the reference configuration |
| 4 | TLS 1.3 is required for the reference transport |
| 5 | A launcher-managed inbound host firewall policy is required for the supported Linux reference configuration |
| 6 | Client-to-node transport stays outside P3-C, with its separate trust requirements documented |
| 7 | Implementation is split into C1 and C2, each needing separate approval |
