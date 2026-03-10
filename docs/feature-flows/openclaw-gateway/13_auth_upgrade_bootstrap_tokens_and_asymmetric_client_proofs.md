# OpenClaw Gateway: 13 - Auth Upgrade with Single-Use Bootstrap Tokens and Asymmetric Client Proofs

## 1. Purpose

This document defines the next authentication model for the OpenClaw Gateway.

It supersedes the current "single long-lived `OPENCLAW_API_KEY` with root-equivalent access" model for runtime traffic and replaces it with:

1. single-use bootstrap tokens for first-time enrollment
2. per-client asymmetric keys for runtime authentication
3. multi-client support so one ACPMS instance can trust more than one OpenClaw installation
4. replay-resistant signed requests for REST, SSE, and WebSocket upgrade handshakes

The goal is to preserve the existing OpenClaw control-plane architecture while materially reducing the blast radius of credential leaks.

## 2. Current State and Problem Statement

The current OpenClaw Gateway design is implemented as:

* one global `OPENCLAW_API_KEY`
* direct `Authorization: Bearer <OPENCLAW_API_KEY>` authentication for `/api/openclaw/*`
* a synthetic `OpenClaw Gateway` service principal mapped to system-admin-equivalent access

This model is simple, but it has four material weaknesses:

1. anyone who obtains the gateway key can call the full mirrored admin surface
2. the server cannot distinguish one OpenClaw installation from another
3. the server cannot revoke one OpenClaw client without rotating the global secret
4. the first-run trust model is operator-friendly, but runtime trust is too coarse

The next model keeps the UX of "generate a credential for onboarding a new OpenClaw", but changes the semantics:

* the onboarding credential becomes a short-lived, single-use bootstrap token
* runtime access moves to public-key-based proof-of-possession

## 3. Goals

### 3.1 Goals

* Remove long-lived shared-secret runtime authentication from `/api/openclaw/*`.
* Support multiple OpenClaw clients on the same ACPMS instance.
* Allow an operator to generate a new enrollment credential whenever they want to add another OpenClaw.
* Make every runtime request attributable to a specific OpenClaw client and key.
* Protect REST, SSE, and WebSocket upgrade requests from replay.
* Keep the existing mirrored route surface and synthetic admin semantics.
* Preserve stream-first operation. SSE and WebSocket business behavior should not change.
* Keep the first-run UX simple enough for installer, admin UI, and future automation.

### 3.2 Non-Goals

* This document does not replace TLS. Transport encryption still relies on HTTPS and WSS.
* This document does not require hardware-backed keys or "one machine only" binding.
* This document does not require mTLS for the first release, although mTLS remains compatible as a future hardening layer.
* This document does not redesign ACPMS domain RBAC. OpenClaw remains a synthetic super-admin-equivalent actor unless explicitly changed elsewhere.

## 4. Design Summary

The upgraded trust model is:

1. An ACPMS administrator creates a bootstrap token for a new OpenClaw installation.
2. The bootstrap token is single-use and short-lived.
3. OpenClaw generates its own asymmetric keypair locally.
4. OpenClaw proves possession of the private key during enrollment by signing a server-issued challenge.
5. ACPMS stores the OpenClaw client's public key and assigns it a stable `client_id`.
6. All runtime OpenClaw traffic uses signed requests identified by `client_id` and `key_id`.
7. Bootstrap tokens are never accepted on runtime endpoints.

This means leaked bootstrap tokens are useful only during a narrow enrollment window, and leaked runtime traffic cannot be replayed because each signed request carries a nonce and freshness window.

## 5. Terminology

### 5.1 Bootstrap Token

A short-lived, single-use secret created by ACPMS so a new OpenClaw installation can enroll itself.

### 5.2 OpenClaw Client

One logical OpenClaw installation trusted by ACPMS. Each client has:

* a stable `client_id`
* one or more active public keys
* an independent lifecycle and revocation state

### 5.3 Client Key

One asymmetric keypair owned by one OpenClaw client. The private key stays in OpenClaw. ACPMS stores only the public key and metadata.

### 5.4 Proof-of-Possession

A signature created with the client's private key over a canonical representation of the request. ACPMS verifies the signature using the stored public key.

## 6. Trust Model

### 6.1 What ACPMS Trusts

ACPMS trusts three things in order:

1. a bootstrap token may enroll exactly one new client before expiry
2. an enrolled client may act only if it can prove possession of a registered private key
3. a registered client may be revoked independently of all other clients

### 6.2 What OpenClaw Must Store

Each OpenClaw installation stores:

* `client_id`
* `key_id`
* private key
* ACPMS base URL

It does not need to keep the bootstrap token after enrollment succeeds.

### 6.3 Threats This Design Mitigates

* Leakage of a long-lived shared runtime secret
* Replay of captured HTTP requests inside the allowed freshness window
* Inability to distinguish one OpenClaw from another
* Coarse revocation that currently requires global secret rotation

### 6.4 Threats Not Fully Solved

* A bootstrap token stolen before first use can still be used to enroll a rogue client.
* A stolen private key still allows runtime access for that client until revoked.
* A compromised OpenClaw host can still issue valid signed requests while compromised.

These are acceptable residual risks for v2 and are still materially better than the current shared-secret runtime model.

## 7. Cryptographic Choices

### 7.1 Algorithm

The recommended signing algorithm is `Ed25519`.

Reasons:

* simple and fast
* widely supported
* small keys and signatures
* deterministic signatures
* good fit for application-layer request signing

### 7.2 Hashing

Use `SHA-256` for:

* request body digest
* nonce hashing at rest
* bootstrap token hashing at rest

### 7.3 Randomness

Bootstrap tokens, request nonces, and server enrollment challenges must come from a cryptographically secure random source.

## 8. Data Model

The runtime auth model requires three new persistence concepts.

### 8.1 `openclaw_bootstrap_tokens`

Purpose:

* track pending enrollment credentials
* enforce single-use semantics
* support operator audit and revocation

Recommended schema:

```sql
CREATE TABLE openclaw_bootstrap_tokens (
    id UUID PRIMARY KEY,
    token_hash TEXT NOT NULL UNIQUE,
    label TEXT NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ NULL,
    revoked_at TIMESTAMPTZ NULL,
    consumed_by_client_id UUID NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX idx_openclaw_bootstrap_tokens_expires_at
    ON openclaw_bootstrap_tokens (expires_at);
```

Rules:

* Store only a hash of the token.
* A token is valid only if `used_at IS NULL`, `revoked_at IS NULL`, and `expires_at > NOW()`.
* `consumed_by_client_id` is set when enrollment completes.

### 8.2 `openclaw_clients`

Purpose:

* identify each enrolled OpenClaw installation
* support list/revoke/audit operations

Recommended schema:

```sql
CREATE TABLE openclaw_clients (
    id UUID PRIMARY KEY,
    client_id TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'revoked', 'disabled')),
    enrolled_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    enrolled_via_bootstrap_token_id UUID NULL REFERENCES openclaw_bootstrap_tokens(id),
    last_seen_at TIMESTAMPTZ NULL,
    last_seen_ip TEXT NULL,
    last_seen_user_agent TEXT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);
```

Rules:

* `client_id` is the stable identifier used in runtime headers.
* Revoking a client revokes all runtime access for all its keys.

### 8.3 `openclaw_client_keys`

Purpose:

* support key rotation without replacing the client identity
* support audit and revocation at key level

Recommended schema:

```sql
CREATE TABLE openclaw_client_keys (
    id UUID PRIMARY KEY,
    client_id UUID NOT NULL REFERENCES openclaw_clients(id) ON DELETE CASCADE,
    key_id TEXT NOT NULL,
    algorithm TEXT NOT NULL,
    public_key TEXT NOT NULL,
    public_key_fingerprint TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'revoked', 'expired')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at TIMESTAMPTZ NULL,
    last_used_at TIMESTAMPTZ NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    UNIQUE (client_id, key_id)
);
```

Rules:

* ACPMS accepts runtime signatures only from active keys belonging to active clients.
* `public_key_fingerprint` is stored for UI, audit, and troubleshooting.

### 8.4 `openclaw_request_nonces`

Purpose:

* prevent replay of signed runtime requests inside the freshness window

Recommended schema:

```sql
CREATE TABLE openclaw_request_nonces (
    id BIGSERIAL PRIMARY KEY,
    client_id UUID NOT NULL REFERENCES openclaw_clients(id) ON DELETE CASCADE,
    key_id TEXT NOT NULL,
    nonce_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    UNIQUE (client_id, key_id, nonce_hash)
);

CREATE INDEX idx_openclaw_request_nonces_expires_at
    ON openclaw_request_nonces (expires_at);
```

Rules:

* Nonces are retained only for the replay window plus a small buffer.
* Cleanup can run in bounded batches.

## 9. Configuration

Recommended new configuration values:

```env
OPENCLAW_GATEWAY_ENABLED=true
OPENCLAW_BOOTSTRAP_TOKEN_TTL_MINUTES=15
OPENCLAW_REQUEST_SIGNATURE_MAX_AGE_SECONDS=300
OPENCLAW_REQUEST_NONCE_RETENTION_SECONDS=600
OPENCLAW_CLOCK_SKEW_TOLERANCE_SECONDS=60
OPENCLAW_EVENT_RETENTION_HOURS=168
```

The old `OPENCLAW_API_KEY` should be removed from runtime auth design and replaced by bootstrap-token issuance managed in database and admin APIs.

## 10. Enrollment Flow

### 10.1 Operator Flow

When an operator wants to add a new OpenClaw installation:

1. open ACPMS admin UI or CLI
2. generate a bootstrap token with label and TTL
3. deliver that token to the intended OpenClaw installation
4. OpenClaw uses it once to enroll
5. ACPMS consumes the token

To add a second or third OpenClaw, the operator simply generates another bootstrap token.

### 10.2 Bootstrap Flow Overview

The recommended enrollment flow is two-step:

1. `prepare`
2. `complete`

### 10.3 `POST /api/openclaw/bootstrap/prepare`

Purpose:

* validate the bootstrap token
* return a server challenge and ACPMS bootstrap metadata

Request:

```http
POST /api/openclaw/bootstrap/prepare
Authorization: Bearer <BOOTSTRAP_TOKEN>
Content-Type: application/json
```

```json
{
  "display_name": "OpenClaw Production",
  "requested_algorithm": "Ed25519",
  "public_key": "base64-encoded-public-key",
  "metadata": {
    "version": "1.0.0",
    "environment": "prod"
  }
}
```

Response:

```json
{
  "success": true,
  "code": "0000",
  "message": "Bootstrap challenge issued",
  "data": {
    "challenge_id": "uuid",
    "challenge_nonce": "base64-random-nonce",
    "challenge_expires_at": "2026-03-10T10:45:00Z",
    "acpms_profile": {
      "base_endpoint_url": "https://api.example.com/api/openclaw/v1",
      "openapi_url": "https://api.example.com/api/openclaw/openapi.json",
      "guide_url": "https://api.example.com/api/openclaw/guide-for-openclaw",
      "events_stream_url": "https://api.example.com/api/openclaw/v1/events/stream",
      "websocket_base_url": "wss://api.example.com/api/openclaw/ws"
    }
  }
}
```

Server behavior:

* verify bootstrap token status
* hash and store the pending challenge
* do not consume the token yet
* require the same public key to appear in `complete`

### 10.4 `POST /api/openclaw/bootstrap/complete`

Purpose:

* prove possession of the private key
* create the OpenClaw client and first active key
* consume the bootstrap token

Request:

```http
POST /api/openclaw/bootstrap/complete
Authorization: Bearer <BOOTSTRAP_TOKEN>
Content-Type: application/json
```

```json
{
  "challenge_id": "uuid",
  "display_name": "OpenClaw Production",
  "algorithm": "Ed25519",
  "public_key": "base64-encoded-public-key",
  "signature": "base64-signature-over-server-challenge",
  "metadata": {
    "version": "1.0.0",
    "environment": "prod"
  }
}
```

The signature input should be:

```text
ACPMS-OPENCLAW-ENROLL
challenge_id
challenge_nonce
display_name
public_key
algorithm
```

Response:

```json
{
  "success": true,
  "code": "0000",
  "message": "OpenClaw client enrolled successfully",
  "data": {
    "client_id": "oc_client_01HV...",
    "key_id": "key_01HV...",
    "algorithm": "Ed25519",
    "guide_url": "https://api.example.com/api/openclaw/guide-for-openclaw",
    "openapi_url": "https://api.example.com/api/openclaw/openapi.json",
    "events_stream_url": "https://api.example.com/api/openclaw/v1/events/stream",
    "websocket_base_url": "wss://api.example.com/api/openclaw/ws"
  }
}
```

Server behavior:

* verify challenge existence and expiry
* verify bootstrap token still valid and unused
* verify signature with the supplied public key
* create `openclaw_clients` row
* create `openclaw_client_keys` row
* mark token `used_at`
* bind token to `consumed_by_client_id`

### 10.5 Bootstrap Token Semantics

Required rules:

* single use
* short TTL
* accepted only on bootstrap endpoints
* not accepted on `/api/openclaw/v1/*`
* not accepted on `/api/openclaw/ws/*`
* not accepted on `/api/openclaw/openapi.json` after runtime auth becomes signature-only

## 11. Runtime Request Signing

### 11.1 Required Runtime Headers

Every signed OpenClaw runtime request must include:

```http
X-OpenClaw-Client-Id: oc_client_...
X-OpenClaw-Key-Id: key_...
X-OpenClaw-Timestamp: 2026-03-10T10:30:00Z
X-OpenClaw-Nonce: a-random-unique-value
X-OpenClaw-Signature: base64-signature
```

`Authorization: Bearer ...` is not used on runtime endpoints in the target design.

### 11.2 Canonical Request Format

The signature input should be:

```text
METHOD
PATH
NORMALIZED_QUERY
BODY_SHA256
X-OpenClaw-Client-Id
X-OpenClaw-Key-Id
X-OpenClaw-Timestamp
X-OpenClaw-Nonce
SIGNED_HEADERS_VERSION=v1
```

Where:

* `METHOD` is uppercase
* `PATH` is the exact request path
* `NORMALIZED_QUERY` is the normalized query string or empty string
* `BODY_SHA256` is the lowercase hex SHA-256 of the raw request body bytes

For empty bodies, use the SHA-256 of the empty byte sequence.

### 11.3 Server Verification Rules

The server must:

1. resolve `client_id`
2. resolve active `key_id` under that client
3. reject revoked or inactive client/key
4. reject timestamps outside the freshness window
5. reject reused nonce for the same `client_id + key_id`
6. recompute canonical request
7. verify signature with stored public key
8. only then map to the synthetic OpenClaw admin principal

### 11.4 Audit Fields

Every successful and failed auth decision should log:

* `client_id`
* `key_id`
* `auth_mode=signature`
* `auth_result`
* `path`
* `user_agent`
* `forwarded_for`

## 12. REST API Impact

REST handlers and DTOs do not need to change.

Only the auth entry point changes:

* remove shared-secret runtime comparison
* add signed-request verification
* continue injecting the same synthetic admin principal after auth success

This preserves the current mirrored route surface and minimizes blast radius in business code.

## 13. SSE Impact

### 13.1 What Changes

The initial `GET` request that opens the SSE stream must be signed like any other runtime request.

### 13.2 What Does Not Change

* event payload format
* `Content-Type: text/event-stream`
* replay behavior
* `Last-Event-ID` and `?after=` semantics
* keep-alive comments

### 13.3 Replay Considerations

To avoid ambiguity between signed query and unsigned resumptions:

* `?after=<cursor>` remains the preferred resume mechanism
* `Last-Event-ID` may stay supported, but if retained it should be included in the canonical signed-header set whenever present

This keeps resume state under the same replay-resistant auth model.

### 13.4 Reconnect Behavior

Each reconnect is a new signed request with:

* a fresh nonce
* a fresh timestamp
* the current resume cursor

No per-event signatures are required once the stream is established.

## 14. WebSocket Impact

### 14.1 What Changes

WebSocket authentication moves to the HTTP upgrade request.

The upgrade request must carry the signed runtime headers:

* `X-OpenClaw-Client-Id`
* `X-OpenClaw-Key-Id`
* `X-OpenClaw-Timestamp`
* `X-OpenClaw-Nonce`
* `X-OpenClaw-Signature`

### 14.2 What Does Not Change

* route shapes under `/api/openclaw/ws/*`
* message formats after successful upgrade
* business authorization checks after auth

### 14.3 Browser Constraints

This gateway is server-to-server. OpenClaw is not limited by browser header restrictions, so request-signature headers are acceptable and preferred over passing secrets in `Sec-WebSocket-Protocol`.

### 14.4 Replay Considerations

The handshake is replay-protected by:

* timestamp freshness window
* nonce uniqueness

No per-frame signing is required in v2.

## 15. Guide and OpenAPI Endpoint Impact

### 15.1 `guide-for-openclaw`

Recommended behavior:

* bootstrap token may access a bootstrap-safe guide response before enrollment
* enrolled clients may access the full runtime guide with signed auth

This keeps the "OpenClaw can self-bootstrap" UX intact.

### 15.2 `openapi.json`

Recommended behavior:

* pre-enrollment: optional access via bootstrap token if needed for bootstrap UX
* post-enrollment: signed auth only

This avoids leaving the full control-plane contract readable without either enrollment or bootstrap authorization.

## 16. Internal Admin API for Operators

ACPMS should add internal admin endpoints or UI actions for managing enrollment and clients.

### 16.0 Super Admin Settings UX

The primary operator surface for this feature should live in the global `Super Admin Settings` screen.

Recommended UX:

* `SettingsPage.tsx` exposes an `OpenClaw Access` or `Manage OpenClaw` button inside the super-admin settings area
* clicking the button opens a modal or popup dedicated to OpenClaw client management
* the modal becomes the main place to:
  * view all enrolled OpenClaw clients
  * view whether each client currently has access
  * generate a new bootstrap prompt for adding another OpenClaw installation
  * disable or re-enable access for an existing OpenClaw client

The modal should not expose raw runtime secrets because the target model has no long-lived shared runtime secret.

### 16.0.1 Modal Sections

Recommended layout:

1. `Enrolled OpenClaw Clients`
2. `Create Bootstrap Prompt`
3. `Client Details` drawer or inline expanded row for the selected client

### 16.0.2 Enrolled Clients Table

Recommended columns:

* display name
* `client_id`
* status: `active`, `disabled`, or `revoked`
* enrolled at
* last seen at
* key fingerprint summary
* actions

Recommended row actions:

* `Disable Access`
* `Enable Access`
* `View Details`
* optional `Revoke Client`

Rules:

* `Disable Access` should be reversible and map to `status=disabled`
* `Enable Access` should reactivate a previously disabled client without reenrollment
* `Revoke Client` should remain a stronger action than disable and should require explicit confirmation

### 16.0.3 Create Bootstrap Prompt

The modal should let the admin generate a ready-to-send prompt for a new OpenClaw installation.

Recommended form fields:

* label
* bootstrap token TTL
* optional environment or metadata tag
* optional suggested display name for the new OpenClaw client

When submitted, ACPMS should:

1. create a single-use bootstrap token
2. render a complete bootstrap prompt
3. show the prompt once for copy/export

The generated prompt should include:

* ACPMS base URL
* guide endpoint
* OpenAPI endpoint
* SSE endpoint
* WebSocket base
* single-use bootstrap token
* instruction that the token is for enrollment only and will stop working after the first successful enrollment

The admin should be able to:

* copy the prompt
* regenerate a new prompt if the old token expires unused
* close the dialog without storing the raw token in the UI again later

### 16.0.4 Access Toggle Semantics

The admin requirement for "tắt bật access" should map to client status, not to bootstrap tokens.

Recommended status model:

* `active`: client may authenticate and use runtime APIs
* `disabled`: client record remains enrolled, but all runtime auth requests are rejected until re-enabled
* `revoked`: client is permanently blocked and should generally require a fresh enrollment flow

This lets the operator temporarily pause one OpenClaw installation without affecting others.

### 16.1 Bootstrap Token Management

Recommended internal endpoints:

* `POST /api/v1/admin/openclaw/bootstrap-tokens`
* `GET /api/v1/admin/openclaw/bootstrap-tokens`
* `POST /api/v1/admin/openclaw/bootstrap-tokens/{id}/revoke`

Request shape for create:

```json
{
  "label": "OpenClaw staging bot",
  "expires_in_minutes": 15,
  "metadata": {
    "environment": "staging"
  }
}
```

Response should return the raw token only once at creation time.

### 16.2 Client Management

Recommended internal endpoints:

* `GET /api/v1/admin/openclaw/clients`
* `GET /api/v1/admin/openclaw/clients/{client_id}`
* `POST /api/v1/admin/openclaw/clients/{client_id}/disable`
* `POST /api/v1/admin/openclaw/clients/{client_id}/enable`
* `POST /api/v1/admin/openclaw/clients/{client_id}/revoke`
* `POST /api/v1/admin/openclaw/clients/{client_id}/keys/{key_id}/revoke`

This lets operators add more OpenClaw installations whenever needed without touching already-enrolled clients.

## 17. Key Rotation

### 17.1 Client Key Rotation

An enrolled client should be able to rotate its own key through a signed runtime endpoint.

Recommended flow:

1. client generates a new keypair
2. client calls `POST /api/openclaw/v1/client-keys/rotate` signed with the old active key
3. request includes the new public key and proof-of-possession for the new key
4. ACPMS activates the new key and optionally retires the old one

This avoids needing a fresh bootstrap token for normal key rotation on the same OpenClaw installation.

### 17.2 Lost Key Recovery

If the private key is lost:

* revoke the old client or old key in ACPMS
* issue a new bootstrap token
* enroll the installation again

## 18. Revocation Model

### 18.1 Bootstrap Token Revocation

Bootstrap tokens may be revoked before use without affecting existing clients.

### 18.2 Client Revocation

Revoking one client must:

* block all future runtime requests from that client
* leave other OpenClaw clients unaffected

### 18.3 Key Revocation

Revoking one key must:

* block runtime requests signed by that key
* allow other active keys on the same client to continue if policy allows multiple active keys

## 19. Compatibility and Migration Plan

Migration should be staged to avoid breaking current installations.

### 19.1 Phase A: Data Structures and Dual-Stack Support

Add:

* new tables for bootstrap tokens, clients, keys, and request nonces
* signature-verification service
* internal admin APIs for bootstrap-token management

Keep current API-key runtime auth working during this phase.

### 19.2 Phase B: Enrollment Endpoints

Add:

* `/api/openclaw/bootstrap/prepare`
* `/api/openclaw/bootstrap/complete`

Allow new OpenClaw builds to enroll while old ones still use the current API-key-only runtime path.

### 19.3 Phase C: Runtime Dual Auth Mode

Introduce a temporary server mode:

```env
OPENCLAW_RUNTIME_AUTH_MODE=dual
```

Behavior:

* accept current API-key runtime auth
* also accept signed-client runtime auth
* emit warnings whenever API-key runtime auth is used

### 19.4 Phase D: Signature-Only Runtime

After clients are migrated:

```env
OPENCLAW_RUNTIME_AUTH_MODE=signature_only
```

Behavior:

* runtime endpoints reject API-key auth
* bootstrap endpoints continue accepting bootstrap tokens

### 19.5 Phase E: Documentation and Installer Update

Update:

* installer output
* bootstrap prompt
* guide endpoint docs
* OpenAPI descriptions
* operator runbooks for adding and revoking OpenClaw clients

## 20. Operational Considerations

### 20.1 Clock Skew

Signed-request freshness depends on clocks. ACPMS should allow a small skew window, for example `60` seconds.

### 20.2 Nonce Cleanup

Expired nonce rows should be removed in bounded batches.

### 20.3 Rate Limiting

Bootstrap endpoints should be aggressively rate-limited because they are the highest-value pre-runtime trust boundary.

### 20.4 Logging Hygiene

Never log:

* raw bootstrap tokens
* private keys
* full signatures

It is acceptable to log:

* token ID
* client ID
* key ID
* public-key fingerprint

## 21. Optional Future Hardening

These items are recommended but not required for the first release:

* mTLS between OpenClaw and ACPMS
* OS-keystore-backed private key storage on OpenClaw
* asymmetric signing for ACPMS -> OpenClaw optional webhooks, replacing the current HMAC secret model
* approval policy requiring an existing signed client to authorize enrollment of an additional client

## 22. Acceptance Criteria

The auth upgrade is complete only when all of the following are true:

* ACPMS can mint a single-use bootstrap token with TTL and label.
* One bootstrap token can enroll exactly one OpenClaw client.
* ACPMS stores a unique `client_id` and public key for each enrolled OpenClaw.
* REST runtime requests are accepted only when the signature is valid, fresh, and non-replayed.
* SSE connections can authenticate and reconnect with signed requests.
* WebSocket upgrade requests can authenticate with signed headers.
* One compromised or revoked OpenClaw client can be disabled without rotating other clients.
* Operators can add another OpenClaw installation by generating another bootstrap token.
* Audit logs identify which OpenClaw client and key issued each request.

## 23. Final Recommendation

The recommended target architecture is:

* no long-lived shared runtime secret
* single-use bootstrap tokens for enrollment
* per-client Ed25519 keys for runtime proof-of-possession
* multi-client support with independent revocation and rotation
* unchanged mirrored ACPMS business surface for `/api/openclaw/v1/*`, SSE, and WebSocket routes

This is the smallest upgrade that meaningfully improves trust separation without forcing a redesign of the existing OpenClaw Gateway architecture.

## 24. Implementation Slice Reference

For the narrower `Super Admin Settings -> OpenClaw Access` management slice, including modal UX, admin endpoints, and granular checklist items, see:

* `14_super_admin_openclaw_access_management_breakdown.md`
