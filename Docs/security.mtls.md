# mTLS Configuration

Env variables (set to enable):
- `AGENT_TLS_CERT_FILE`: server certificate chain (PEM)
- `AGENT_TLS_KEY_FILE`: server private key (PKCS#8 PEM)
- `AGENT_TLS_CA_FILE`: CA bundle used to authenticate clients

When all are set, the orchestrator enables mTLS and requires client certificates. ALPN is set to `h2` for gRPC.

Client should present cert signed by the CA and set `authorization` metadata if token auth is enabled.
