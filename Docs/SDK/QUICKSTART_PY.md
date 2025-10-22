# ORCA Python SDK Quickstart

This quickstart shows how to call the ORCA orchestrator via gRPC from Python with TLS and auth.

## Prerequisites
- Python 3.11+
- `grpcio`, `grpcio-tools`, `protobuf`
- ORCA proto: `Docs/API/orca_v1.proto` (or the CI artifact download)
- CA certificate for mTLS validation (e.g., `ca.pem`) if TLS is enabled

## Generate stubs
See `Docs/SDK/GENERATION.md` for full codegen steps. Minimal example:

```bash
python -m pip install grpcio grpcio-tools protobuf
python -m grpc_tools.protoc -I Docs/API \
  --python_out=. --grpc_python_out=. Docs/API/orca_v1.proto
```

This produces `orca_v1_pb2.py` and `orca_v1_pb2_grpc.py`.

## Example (async)

```python
import asyncio
import grpc
import orca_v1_pb2 as pb
import orca_v1_pb2_grpc as rpc

AUTH_TOKEN = "REPLACE_ME"

async def main():
    # TLS (optional): provide root CA; for dev you may use plaintext
    creds = grpc.ssl_channel_credentials(root_certificates=open("ca.pem", "rb").read())
    async with grpc.aio.secure_channel("localhost:50051", creds) as channel:
        stub = rpc.OrchestratorStub(channel)

        md = [("authorization", f"Bearer {AUTH_TOKEN}")]

        # StartRun
        env = pb.Envelope(
            id="msg-1", parent_id="", trace_id="trace-1", agent="agent.py",
            kind="agent_task", payload_json="{}", timeout_ms=1000,
            protocol_version=1, ts_ms=0
        )
        start = pb.StartRunRequest(workflow_id="wf-1", initial_task=env, budget=pb.Budget(max_tokens=1000, max_cost_micros=0))
        try:
            await stub.StartRun(start, metadata=md)
        except grpc.aio.AioRpcError as e:
            print("StartRun error:", e.code(), e.details())
            return

        # SubmitTask (include optional usage hints if available)
        # Optionally include usage hints (if available from your tool/LLM response)
        env.usage.tokens = 128
        env.usage.cost_micros = 2500
        task = pb.SubmitTaskRequest(run_id="wf-1", task=env)
        try:
            await stub.SubmitTask(task, metadata=md)
        except grpc.aio.AioRpcError as e:
            if e.code() == grpc.StatusCode.RESOURCE_EXHAUSTED:
                print("Budget exceeded:", e.details())
            else:
                print("SubmitTask error:", e.code(), e.details())

        # StreamEvents
        async for ev in stub.StreamEvents(pb.StreamEventsRequest(run_id="wf-1", start_event_id="0"), metadata=md):
            print("event:", ev)
            break

if __name__ == "__main__":
    asyncio.run(main())
```

## Notes
- Auth: send `authorization: Bearer <token>` metadata.
- mTLS: see `Docs/security.mtls.md` for server setup; provide CA to the client.
- Timeouts: `Envelope.timeout_ms` is enforced by the orchestrator.
- Idempotency: duplicate `Envelope.id` is accepted and deduped by the orchestrator.
