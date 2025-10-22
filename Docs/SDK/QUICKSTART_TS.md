# ORCA TypeScript SDK Quickstart

This quickstart shows how to call the ORCA orchestrator via gRPC from Node using `@grpc/grpc-js`.

## Prerequisites
- Node.js 18+
- `@grpc/grpc-js`, `@grpc/proto-loader`
- ORCA proto: `Docs/API/orca_v1.proto` (or CI artifact)
- CA certificate for TLS validation (e.g., `ca.pem`) if TLS is enabled

## Install
```bash
npm i @grpc/grpc-js @grpc/proto-loader
```

## Example
```ts
import * as fs from 'fs'
import * as grpc from '@grpc/grpc-js'
import * as protoLoader from '@grpc/proto-loader'

const PROTO_PATH = 'Docs/API/orca_v1.proto'
const packageDef = protoLoader.loadSync(PROTO_PATH, {
  keepCase: true,
  longs: String,
  enums: String,
  defaults: true,
  oneofs: true,
})
const proto = grpc.loadPackageDefinition(packageDef) as any
const Orchestrator = proto.orca.v1.Orchestrator

const creds = grpc.credentials.createSsl(fs.readFileSync('ca.pem'))
const client = new Orchestrator('localhost:50051', creds)

const md = new grpc.Metadata()
md.add('authorization', 'Bearer REPLACE_ME')

client.StartRun({ workflow_id: 'wf-1', initial_task: {
  id: 'msg-1', parent_id: '', trace_id: 'trace-1', agent: 'agent.ts',
  kind: 'agent_task', payload_json: '{}', timeout_ms: 1000,
  protocol_version: 1, ts_ms: 0
}, budget: { max_tokens: 1000, max_cost_micros: 0 } }, md, (err: any, resp: any) => {
  if (err) console.error(err)
  else console.log('StartRun ok')
})

client.SubmitTask({ run_id: 'wf-1', task: {
  id: 'msg-1', parent_id: '', trace_id: 'trace-1', agent: 'agent.ts',
  kind: 'agent_task', payload_json: '{}', timeout_ms: 1000,
  protocol_version: 1, ts_ms: 0,
  usage: { tokens: 128, cost_micros: 2500 }
}}, md, (err: any, resp: any) => {
  if (err) {
    if (err.code === grpc.status.RESOURCE_EXHAUSTED) {
      console.error('Budget exceeded:', err.details)
    } else {
      console.error('SubmitTask error:', err)
    }
  }
  else console.log('SubmitTask ok')
})

const stream = client.StreamEvents({ run_id: 'wf-1', start_event_id: '0' }, md)
stream.on('data', (ev: any) => {
  console.log('event', ev)
  stream.cancel()
})
stream.on('error', (e: any) => console.error(e))
```

## Notes
- Auth header: `authorization: Bearer <token>`.
- See `Docs/security.mtls.md` for TLS/mTLS details.
- Envelope fields must match the proto (see `Docs/API/orca_v1.proto`).
