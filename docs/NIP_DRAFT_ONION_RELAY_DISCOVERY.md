# NIP-XX

## Onion Relay Discovery

`draft` `optional`

This NIP defines a way for relays to advertise a Tor hidden-service ("onion")
address alongside their normal `wss://` URL, and for that mapping to
propagate through the network via relay list events, so that clients can
connect without exposing their IP address to the relay operator.

## Motivation

A relay operator always learns the IP address of every client that connects
to it — this is inherent to how the underlying WebSocket/TCP connection
works and cannot be hidden by anything at the event/message layer. Combined
with the fact that a client's own `REQ` subscriptions and published events
are signed by its pubkey, a relay operator (or anyone with access to relay
logs) can directly link **IP address → pubkey → online/offline pattern →
what is read and posted**, for every user who connects.

This is a materially different, and arguably more serious, form of leakage
than e.g. embedded media URLs (addressed informally by client-side media
proxy settings), because the relay is not an incidental third party — it is
a party the protocol requires you to connect to directly, repeatedly, and
for extended periods.

Existing NIPs address *content* metadata (NIP-17/NIP-59 gift-wrapping hides
who is messaging whom from the relay's stored data) but nothing addresses
*network-layer* metadata — the fact of the connection itself.

Nostr's relay model is intentionally simple, and this NIP does not propose
changing that. It proposes the smallest possible extension that lets relays
who already choose to run a Tor hidden service (many relay implementations
support this today, informally and without discovery) be found and preferred
by privacy-conscious clients automatically.

## Specification

### 1. Relay self-advertisement (NIP-11)

A relay MAY include an additional `"onion"` field in its
[NIP-11](11.md) Relay Information Document:

```json
{
  "name": "Example Relay",
  "description": "...",
  "pubkey": "...",
  "contact": "...",
  "supported_nips": [1, 11, 42],
  "software": "...",
  "version": "...",
  "onion": "ws://exampleoniondomain1234567890abcdefghijklmnopqrstuvwxyz22.onion"
}
```

- The `onion` field's value MUST be a `ws://` (not `wss://`) URL. Tor's onion
  routing already provides end-to-end authenticated encryption between the
  client and the hidden service (the address itself is derived from the
  service's public key), so an additional TLS layer is redundant — this
  mirrors common practice for onion services generally.
- This mechanism requires one clearnet connection (to fetch NIP-11) before
  the onion address is known. See Security Considerations.

### 2. Onion mirror propagation (NIP-65)

To avoid requiring a clearnet connection to *every* relay before its onion
mirror is known, a client MAY publish the onion↔clearnet mapping it has
already discovered as an additional tag on its own
[NIP-65](65.md) Relay List Metadata event (`kind:10002`):

```json
{
  "kind": 10002,
  "tags": [
    ["r", "wss://relay.example.com", "read"],
    ["onion", "wss://relay.example.com", "ws://exampleoniondomain...onion"]
  ],
  ...
}
```

This lets onion addresses for a relay propagate through the social graph —
a client can learn a relay's onion mirror from *any* NIP-65 event that
references it, fetched over a connection it already trusts (potentially
itself an onion connection), without ever making a clearnet request to that
relay directly.

### 3. Client behavior

- A client SHOULD detect whether a local SOCKS5 proxy (Tor or otherwise) is
  configured.
- When a client has a Tor proxy available and knows (via either mechanism
  above) an onion address for a relay it wishes to connect to, it SHOULD
  prefer the onion address.
- A client MAY fall back to the clearnet address if the onion connection
  fails, times out, or is unavailable. Clients SHOULD offer a setting to
  disable clearnet fallback for users who want connections to fail closed
  rather than silently deanonymize them.

### 4. Relay behavior

- A relay operator who runs a Tor hidden service for their relay SHOULD
  advertise it via the NIP-11 `onion` field.
- No changes are required of relays that do not wish to support this.

## Security Considerations

- **Bootstrap leak**: fetching a relay's NIP-11 document over clearnet to
  *discover* its onion address is itself one clearnet request. This NIP
  does not eliminate that first-contact leak for relays not already known
  via NIP-65 propagation (mechanism 2) or an out-of-band source. Client
  implementers should be transparent with users that this may occur.
- **Scope**: this NIP addresses network-layer IP exposure only. It does not
  address relay-side behavioral fingerprinting (subscription patterns,
  timing correlation between a relay's clearnet and onion listeners run by
  the same operator, etc.), nor the separate problem that NIP-42 AUTH makes
  the IP↔pubkey link explicit and signed when used.
- **Not a replacement for a VPN**: this is a complementary, protocol-native
  option for users who accept Tor's latency characteristics for real-time
  relay traffic. A system-wide VPN remains a simpler, lower-risk way to
  achieve the same network-layer protection without any client-side
  changes, at the cost of trusting the VPN provider instead of the relay
  operator.

## Backward Compatibility

Fully backward compatible. Both the NIP-11 field and the NIP-65 tag are
purely additive and optional; relays and clients that do not implement this
NIP simply ignore them.

## Implementation Note

Connecting to a relay's *clearnet* `wss://` URL through a SOCKS5 proxy
requires manually establishing the SOCKS connection and then layering TLS
on top of it — real engineering work, since most WebSocket client libraries
bundle DNS+TCP+TLS+handshake into one call that doesn't accept a proxy.

Connecting to an onion address is actually simpler in one respect: since
Tor already provides transport security, there is no TLS layer to
reconstruct manually — a client only needs to dial the SOCKS5 proxy and
perform the WebSocket handshake directly over that stream. This NIP's onion
addresses are therefore a meaningfully easier target for client
implementers than general clearnet-relay-over-Tor proxying would be.
