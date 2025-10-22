# SDK Client Generation (Python & TypeScript)

Proto source: `Docs/API/orca_v1.proto`

## Python (grpcio)

Prereqs:
- Python 3.11+
- `pip install grpcio grpcio-tools`

Generate:

```
python -m grpc_tools.protoc \
  -I Docs/API \
  --python_out=sdk/python \
  --grpc_python_out=sdk/python \
  Docs/API/orca_v1.proto
```

Usage sketch:

```python
import os
import grpc
from sdk.python import orca_v1_pb2 as pb, orca_v1_pb2_grpc as rpc

channel = grpc.insecure_channel("localhost:50051")
stub = rpc.OrchestratorStub(channel)

md = []
token = os.getenv("AGENT_AUTH_TOKEN")
if token:
    md.append(("authorization", token))

env = pb.Envelope(
    id="m1", trace_id="t", agent="A", kind="agent_task",
    payload_json="{}", protocol_version=1, ts_ms=1
)
r = stub.StartRun(pb.StartRunRequest(workflow_id="wf1", initial_task=env), metadata=md)
print(r.run_id)
```

## TypeScript (grpc-tools)

Prereqs:
- Node 18+
- `npm i -D grpc-tools grpc_tools_node_protoc_ts`

Generate:

```
./node_modules/.bin/grpc_tools_node_protoc \
  -I Docs/API \
  --js_out=import_style=commonjs,binary:sdk/ts \
  --grpc_out=grpc_js:sdk/ts \
  Docs/API/orca_v1.proto

./node_modules/.bin/grpc_tools_node_protoc \
  -I Docs/API \
  --plugin=protoc-gen-ts=./node_modules/.bin/protoc-gen-ts \
  --ts_out=grpc_js:sdk/ts \
  Docs/API/orca_v1.proto
```

Usage sketch:

```ts
import * as grpc from '@grpc/grpc-js';
import { OrchestratorClient } from './sdk/ts/orca/v1/orca_v1_grpc_pb';
import { Envelope, StartRunRequest } from './sdk/ts/orca/v1/orca_v1_pb';

const client = new OrchestratorClient('localhost:50051', grpc.credentials.createInsecure());
const md = new grpc.Metadata();
const token = process.env.AGENT_AUTH_TOKEN;
if (token) md.add('authorization', token);

const env = new Envelope();
env.setId('m1'); env.setTraceId('t'); env.setAgent('A'); env.setKind('agent_task');
env.setPayloadJson('{}'); env.setProtocolVersion(1); env.setTsMs(1);

const req = new StartRunRequest();
req.setWorkflowId('wf1'); req.setInitialTask(env);

client.startRun(req, md, (err, resp) => {
  if (err) throw err;
  console.log(resp.getRunId());
});
```
