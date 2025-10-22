AgentMesh Roadmap (Deterministic, Observable Multi-Agent Execution)

Note: This roadmap aligns with the AgentMesh architectural blueprint and incorporates best practices from recent multi-agent systems research. It is a single-track plan targeting macOS and Linux initially (Windows support in a later phase).

1. Executive Summary

AgentMesh is a developer-local, self-hosted runtime for running multi-agent AI systems with deterministic execution, rich observability, and strict cost governance. The goal is to provide an enterprise-grade framework where AI agents can coordinate on complex tasks under controlled conditions. Unlike ad-hoc agent scripts or cloud services, AgentMesh emphasizes reproducibility and auditability: every agent action is event-sourced and replayable, preventing "black box" behavior common in other frameworks. It also features built-in telemetry, budget enforcement, and security guardrails to ensure reliable and safe operation on local machines.

Principles:

Determinism & Reproducibility: All agent interactions are recorded (with fixed random seeds where applicable) so runs can be reproduced exactly for debugging or compliance.

Observability by Design: Every workflow emits structured logs and trace spans for real-time insight and post-run analysis. Low-cardinality metadata is used to avoid telemetry overload.

Cost Awareness: The runtime monitors token usage and API costs continuously, enforcing budgets and preventing runaway expenses (fail-safes for token limits, rate limiting, etc.).

Security First: Agents operate under strict guardrails – with role-based access, sandboxed execution, and audit logging – ensuring sensitive data and actions stay controlled.

Developer-Centric: Provided SDKs (Python & TypeScript) offer simple, declarative APIs to compose agents and tools. Integration is local-first (no cloud dependency), focusing on fast iteration and ease of use in development environments.

2. Architecture & Components

Scope: Core event-sourced runtime, orchestrator, multi-language SDKs, control planes for cost and policy, observability, and security.

Orchestrator Core: A single-controller orchestrator that schedules and coordinates agent tasks in a deterministic state machine. It is the only writer to shared state and sequence of events, ensuring an ordered, repeatable execution. It enforces task dependencies, timeouts, and consolidates results, acting as the "brain" of the multi-agent system. The orchestrator core is implemented in Rust starting in Phase 0.

Agents (Python & TS): Pluggable AI agents each with a specialized role (e.g. code generation, testing, debugging, etc.). Agents interact with the orchestrator via a defined message protocol, receiving tasks and returning results. They run in isolated sandboxes (separate process or environment) so that errors or malicious actions cannot affect the orchestrator or other agents directly. Developers can implement custom agents using the Python/TypeScript SDK, which ensures their inputs/outputs conform to the protocol.

Event Log & Trace Model: Every significant action (agent task request, tool invocation, model response, etc.) is recorded as an event in a persistent log (write-ahead log). Each event carries unique identifiers (monotonic event ID, correlation IDs like traceId and parent links) to build a causality chain. This event-sourced approach enables time-travel debugging and audit: given a log of events, one can replay or inspect the exact sequence of decisions that occurred.

Metadata & Message Schema: A standardized envelope for agent messages captures metadata such as agent identity, task type, timestamps, and resource usage metrics. This schema is consistent across languages and aligns with emerging standards (e.g. JSON-based protocols like A2A) for compatibility. It ensures all contextual data (e.g. original prompt, tool name, token count) is available for logging and policy checks.

Budget Control Plane: A built-in subsystem that tracks consumption of external resources (LLM API tokens, etc.) against predefined limits. It can estimate costs per call and aggregate usage per workflow or per agent. If a max budget per workflow is set (e.g. a dollar or token cap), the orchestrator will enforce it by pausing or stopping agents when the budget is nearly exhausted. This prevents surprise bills and allows developers to govern costs tightly. The control plane also handles rate limiting and can issue warnings or require confirmations if usage spikes.

Observability Subsystem: Comprehensive telemetry is integrated via OpenTelemetry (OTel). Each agent task generates a trace span with key attributes (agent role, task ID, status, latency, tokens used) and all spans are correlated under a top-level workflow trace for end-to-end visibility. Structured logs (e.g. JSON lines) capture events with relevant details (and redact any sensitive content). The system collects metrics like number of tokens consumed, errors encountered, and response times for each agent, enabling dashboards and performance tuning. Developers can sample or filter traces to manage volume, but critical events are always logged for post-mortem analysis.

Policy Engine: A rule-based guardrail layer that intercepts agent operations. It enforces content policies (e.g. no disallowed content in prompts or outputs, using either regex filters or external moderation APIs), tool access policies (agents can only invoke whitelisted tools appropriate to their role), and data handling rules (e.g. PII must not be exposed in logs). If a rule is violated, the engine can block the action or redact the output, and it logs an audit event for traceability. This provides fine-grained control and prevents the agent system from doing anything outside of approved bounds.

Security Framework: AgentMesh is built on a "secure-by-default" philosophy. Each agent and user is assigned an identity; all internal communications can be secured (e.g. via mTLS if running in distributed mode). RBAC (Role-Based Access Control) governs who can start agent workflows or access certain data (for example, only authorized users can run agents with production credentials). Tenant Isolation is supported: in multi-user scenarios, data and contexts are partitioned by tenant so agents operating for one user cannot leak information to another. All actions are logged to an audit trail with tamper-evident records, which is essential for compliance. Additionally, any code execution by agents (for example, a DebugAgent running user code) occurs in sandboxed environments with resource limits (CPU, memory, IO) to contain security risks.

3. Phases Overview & Success Criteria

Phase 0: Architecture Foundations — Establish core project standards and baseline architecture. Set up the event-sourced logging mechanism, basic OpenTelemetry integration, and a security baseline (identity and config scaffolding). Prepare developer infrastructure (testing, CI) to enforce determinism and code quality from the start.

Phase 1: Core Orchestrator & Agent Contracts — Implement the deterministic orchestrator and agent interaction protocol. Define explicit contracts for agent inputs/outputs (message envelope schema, allowed actions, TTL/versioning) and the orchestrator's state machine for task execution. Ensure the orchestrator uses the event log for all state changes, enabling replay and consistency.

Phase 2: SDKs & Metadata Model — Build the Python and TypeScript SDKs and the unified event metadata model. Provide high-level APIs for developers to define custom agents and launch workflows. Define a consistent schema for events and telemetry across languages. Validate extensibility with example agents and ensure cross-language parity.

Phase 3: Budget & Cost Governance — Introduce cost-tracking and budget enforcement. Instrument all LLM/tool calls to accumulate token counts and estimated cost, and implement user-defined budgets at workflow or agent level. Ensure the system can preempt or halt runs that would exceed cost limits and provide real-time feedback on usage.

Phase 4: Observability & Debugging Tools — Deepen telemetry and add "time-travel" debugging. Complete integration of OTel tracing (spans for all agent interactions) and structured logging. Implement trace sampling or truncation for high-volume workflows. Develop tooling to replay or step through a recorded execution trace, allowing developers to inspect agent states over time for debugging.

Phase 5: Policy Engine & Guardrails — Enforce content and action guardrails via a policy engine. Integrate content moderation (prompt/response filtering for disallowed content), tool usage policies (per-role tool allowlists), and automatic redaction of sensitive info in logs. Generate audit log entries for any policy violations or blocks, establishing a clear audit trail.

Phase 6: Security & Multi-Tenancy Hardening — Finalize the security model for production use. Introduce authentication and RBAC controls, ensuring only authorized users or processes can invoke agent runs or access data. Implement tenant isolation in data storage and context to support multi-user scenarios without data leakage. Add PII detection and optional encryption of sensitive data at rest. Conduct security testing and ensure compliance measures (audit logs, zero-trust networking config) are in place.

Phase 7: Integration & Release — Polish for production release and integrate with external frameworks. Add Windows compatibility and resolve any cross-platform issues. Optimize performance (minimize orchestration overhead, tune logging). Validate compatibility with external agent ecosystems (through the internal agentic SWE agent, e.g. adapters for A2A or LangChain) to ensure AgentMesh can plug into larger workflows. Complete documentation, publish SDK packages, and set up governance for maintenance (CI, versioning, etc.).

Success Criteria (Global):

Quality Gates: Comprehensive test coverage (≥85% unit test line coverage, with deterministic tests using fixed seeds). No critical runtime errors across supported OS environments. Performance SLOs met: orchestration overhead adds negligible latency (target <10% overhead vs direct API calls) and memory footprint remains within budget under typical loads.

Determinism: Given the same inputs and random seed, agent workflows produce the same sequence of events (all nondeterministic aspects controlled). Each event carries stable identifiers, and idempotency keys are used where appropriate to avoid double-processing on replays. Trace logs have bounded variability (e.g., no uncontrolled high-cardinality data), ensuring reproducibility.

Observability: 100% of agent actions are traceable through logs or spans. Telemetry data avoids high cardinality pitfalls (only approved fields are used as span attributes to prevent metric explosion). The system provides a clear timeline of actions for any given run, and critical events (errors, policy triggers) are always captured.

Cost Governance: The budget control prevents any run from exceeding specified limits in testing (verified with intentional over-budget scenarios). Cost metrics (token counts, $$ estimates) are accurate within a small margin and are exposed for monitoring. Users can confidently run long agent sessions knowing there are automatic brakes on cost.

Security & Privacy: All interactions can be restricted by RBAC policies (validated in multi-user simulation). TLS/mTLS can be enabled for any network communication. No sensitive user data is present in logs or traces in plain form (verified via tests injecting sample PII and seeing it redacted). Audit logs contain every important action and decision, supporting forensic analysis. The system passes a security review for deployment in an internal environment (e.g., no open ports without auth, no code injection vectors, etc).

Maintainability: The design and implementation are aligned with the specification (traceability matrix shows all spec items addressed). Documentation is complete and developer-friendly, enabling on-boarding of new contributors or users. CI/CD is in place to catch regressions, and a plan exists for future improvements or support.

4. Traceability Matrix (Spec ↔ Roadmap Phases)

Deterministic Event Sourcing: Foundational architecture for reproducible execution ↔ Addressed in Phase 0 (WAL/log setup) and Phase 1 (orchestrator uses event log) with replay tooling in Phase 4.

Trace Model & IDs: Unique trace and correlation identifiers for events ↔ Implemented in Phase 0 (ID scheme, baseline span context) and refined in Phase 1 (message envelope with id/parentId propagation) and Phase 4 (full end-to-end tracing).

Isolation Boundaries: Sandboxing and process isolation for agents ↔ Designed in Phase 0/1 (isolation approach defined) and enforced in Phase 6 (secure sandbox execution and tenant isolation).

Python & TypeScript SDKs: Multi-language developer APIs ↔ Built in Phase 2 (initial SDK implementations) and polished in Phase 7 (final packaging, cross-platform tests).

Metadata/Event Schema: Unified event and telemetry model ↔ Defined in Phase 1 (message envelope fields) and utilized across Phase 2 (SDK event structures) and Phase 4 (trace/log attributes).

Budget Control Plane: Cost tracking and limits enforcement ↔ Implemented in Phase 3 (usage counting, budget checks).

Observability & Telemetry: Tracing, logging, metrics subsystem ↔ Basic logging in Phase 0, full OTel tracing and metrics in Phase 4 (with trace sampling and debug tooling).

Time-Travel Debugging: Replayable execution and state inspection ↔ Delivered in Phase 4 (log replay tool and trace viewer).

Policy Guardrails: Prompt/tool usage policies, redaction, audit ↔ Implemented in Phase 5 (content moderation, tool allowlists, audit logging).

Security Model (RBAC & Privacy): Secure multi-tenant operation ↔ Baseline in Phase 0 (security scaffolding), fully realized in Phase 6 (auth, RBAC, tenant data isolation, PII control).

External Framework Integration: Compatibility with external agent ecosystems ↔ Prepared in Phase 7 (adapters and validation with internal SWE agent for LangChain/A2A interoperability).

5. Risks & Mitigations

Non-deterministic Outputs: LLM-based agents might produce varying results or not all data might be logged, undermining reproducibility. Mitigation: Enforce deterministic mode for critical steps (use temperature=0 or fixed random seeds) and log all prompts/responses so that any run can be fully reconstructed. Regularly test replay of agent traces to catch sources of nondeterminism early.

Trace Volume & Performance: Detailed tracing could generate high volume of events, impacting performance or trace clarity. Mitigation: Adopt low-cardinality, high-value logging only (strict allowlist of log attributes) and implement trace sampling or summarization for long-running loops. Load-test the observability pipeline and use backpressure (e.g. drop or aggregate less critical events) to keep overhead <10%.

Sandbox Escape or Rogue Agent Behavior: If an agent executes arbitrary code or uses tools unsafely, it could threaten the host system or data. Mitigation: Run agents in restricted subprocesses with OS-level limits (e.g. time/CPU/memory limits). The orchestrator also enforces timeouts and tool access rules (no agent gets capabilities beyond its role). Perform security audits to ensure an agent cannot write or read data outside its permitted scope.

Cost Overrun or Infinite Loop: An agent might get stuck consuming API calls (e.g. in a logic loop) leading to excessive cost. Mitigation: The budget control plane will terminate workflows that exceed budgets and can impose per-agent call caps (max calls or tokens per agent). Additionally, implement loop-detection or circuit-breakers: if an agent repeats an action excessively, orchestrator can halt or require manual intervention.

Multi-Language Consistency: Developing parallel SDKs in Python and TS raises risk of inconsistency or maintenance overhead. Mitigation: Define core logic and schemas in a language-neutral way (e.g. a JSON schema for messages) and generate/derive SDK models from it. Create cross-language integration tests (e.g. run equivalent workflows in Python vs TS) to ensure both implementations produce identical event logs. Any protocol changes must be versioned and applied in both SDKs to keep them in sync.

Integration Uncertainty: Aligning with external frameworks or evolving standards (A2A, MCP, etc.) may pose compatibility challenges. Mitigation: Keep the AgentMesh message format flexible and close to known standards (JSON-based, with clearly defined fields). The internal SWE agent will act as a bridge to test integration with third-party frameworks early; we will adjust our APIs or provide adapters as needed to avoid late surprises. We also monitor emerging protocols to ensure our design remains compatible (e.g. ensure we can map our messages to A2A if required in future).

Phase 0 — Architecture Foundations

Scope

Establish the fundamental architecture and project standards. This includes validating the event-sourced design, setting up baseline observability and security scaffolding, and configuring developer tooling. Key tasks:

Design the core event log (write-ahead log) structure and unique ID scheme for events/traces.

Implement baseline OpenTelemetry integration (define a tracer and example spans) and structured logging for a simple operation.

Outline the security approach (identity model, configuration for TLS/mTLS, placeholders for RBAC) without fully implementing yet.

Set up development infrastructure: code style enforcement, CI pipeline, and deterministic test practices (e.g. fixed random seeds in tests).

Deliverables

Architecture Design Document: A document detailing AgentMesh's overall architecture, focusing on how an event-sourced runtime will achieve determinism and replay. It will reference relevant rules/specs (e.g., determinism guidelines from the blueprint) and diagram the major components (orchestrator, agents, event log, etc.). This serves as a blueprint for later phases.

Event Log Prototype: A simple implementation of a persistent event log (e.g., an append-only JSON file or lightweight database) with append and read capabilities. It should support writing events in order and replaying them, forming the basis for deterministic replays.

Unique ID & Trace Schema: Definition of how IDs are generated and used. For example, decide on using UUIDs or structured IDs for workflowId, taskId, etc., and how traceId and parentId correlate events. Deliver a short spec or code module for ID generation and an example showing an event with its IDs.

Baseline Telemetry Example: Integrated OpenTelemetry (or similar) with a minimal span emitted. For instance, executing a dummy agent action produces a trace span named "agent.action" with attributes like agent.name, task.id, etc., and a corresponding log entry. This demonstrates the observability setup (ensuring we use low-cardinality attributes per guidelines).

Security Configuration Stubs: Initial configuration and environment variables for security features. e.g., placeholders for TLS certificates (AGENT_TLS_CERT_FILE, etc.) and a sample API key or token mechanism to be expanded later. Also, a note on how agent/user identities will be represented (perhaps just a string user ID in events for now).

Dev Environment Setup: Project scaffold with linters (for Python/TS), formatters, and CI scripts. The codebase should enforce "no exceptions unhandled" and other standards (e.g., treat warnings as errors in compilation if compiled components exist). Also include an initial test suite run (even if just trivial tests for now) integrated into CI to ensure the pipeline is working.

Execution Steps

Architectural Validation: Conduct a kickoff review of requirements (from spec) and finalize choices for core architecture. This includes choosing storage for the log (file vs database), confirming that a single-writer orchestrator will own all state changes (to avoid race conditions), and deciding on any frameworks (e.g., using OpenTelemetry SDK for traces). Document these decisions in the Architecture Design Document, with rationale.

Implement Event Log: Create a simple EventLog class or module with methods append(event) and retrieve(from, to) or similar. Start with a straightforward approach (e.g., append JSON lines to a file). Implement basic error handling (ensure partial writes are either retried or detectable). Test by appending a few dummy events and reading them back.

ID Generation & Trace Schema: Develop a small utility for generating unique IDs (could use UUID4 or a monotonic sequence combined with a prefix). Decide on a trace context propagation strategy: e.g., when a workflow starts, assign a traceId, and every event gets that traceId. Also plan a parentId field for linking task->result. Implement this in the dummy workflow (like generate an ID, pass it through).

Baseline Telemetry Integration: Initialize OpenTelemetry (or a similar tracing system) in the project. Create a tracer provider and console or file exporter. Wrap a dummy operation (like adding two numbers or calling a no-op agent function) in a span, setting a couple of attributes (workflow ID, maybe a dummy agent name). Have this span automatically include the trace ID from our event context. Similarly, log a structured message for the operation (as JSON) including the same IDs. Verify that trace and log can be correlated via the trace ID.

Security Baseline: Establish a configuration file or environment variable scheme for security settings. For example, prepare .env entries for enabling TLS and paths to certs (even if we don't fully implement TLS handshake yet). Write a short doc or comments describing how these will be used in Phase 6. Also, create placeholders for RBAC (e.g., a simple roles config file with one admin user for now) to be expanded later.

Dev Tooling: Set up linting (e.g., ESLint for TS, flake8/black for Python) and add a pre-commit or CI step to run them. Configure a formatter to enforce code style (PEP8, etc.). Set up unit testing frameworks (PyTest, Jest, etc. for respective languages) and ensure a sample test (like a test for the EventLog append/retrieve) runs and passes. Integrate these into a CI pipeline (GitHub Actions or similar) so that every commit/PR will run lint + tests on at least one platform.

Review & Iterate: Perform an internal review of all the above with the team or stakeholders. Make sure the architecture doc addresses all key points (like how determinism is achieved, how observability and security are approached at a high level). Address any feedback, such as clarifying any ambiguous design decisions or adjusting the log format if reviewers see an issue.

Acceptance

Architecture Spec Completed: The architecture design document is finished, reviewed, and approved. It clearly aligns with the spec's goals (e.g., it explicitly notes the use of WAL for reliability and determinism). No major aspect is left undefined (no "TBD" sections). This document will serve as a reference for all subsequent phases.

Event Log Functional: The event log prototype is operational and can persist and replay events reliably. In tests, after writing a series of events (with dummy data), the retrieve function returns them in order with correct data. If the system is killed mid-write (simulated), it recovers without corrupting past data (perhaps the last event might be partial, which should be detected or handled gracefully).

Trace & Log Correlation Demonstrated: Running the dummy operation in this phase produces console output for a span and a structured log line, both sharing the same trace ID or correlation indicator. For example, a span might show traceId=1234 and the log line for that action contains "traceId": "1234". This proves the basic observability wiring is in place. The attributes used are compliant with our low-cardinality policy (e.g., using an agent name or task ID, not entire prompt text).

Security Placeholders in Place: Configuration for TLS and basic auth are in the code, even if they are not fully active. For instance, one could set AGENT_TLS_ENABLE=1 and the system would attempt to load cert files (maybe not actually open a port yet, but at least not ignore it). Similarly, a basic roles/permissions config exists (though enforcement might come later). This ensures we haven't ignored security until the last minute; we have the hooks ready.

Development Environment Ready: The project can be built and tested on macOS and Linux successfully. Linting passes (meaning code adheres to standards). CI reports green on the initial pipeline. The team can confidently start building features on this foundation without needing to retrofit basic tooling. Importantly, the deterministic testing principle is established: for example, any randomness in tests is seeded, and we've documented that practice so future tests will follow it.

No Spec Conflicts: A final check against the spec shows that Phase 0 has not introduced anything conflicting. For example, if the spec mandates explicit error handling (no unchecked exceptions), our code practices reflect that (we might be using Result types or try/except with logging). If the spec had performance guidelines, we haven't violated them (though Phase 0 is more about setup, we ensure no obvious inefficiencies are introduced, e.g., not choosing an extremely slow logging method). Essentially, the foundation is solid and aligned with all higher-level requirements, as verified by the acceptance review.

Phase 1 — Core Orchestrator & Agent Contracts

Scope

Develop the core orchestrator logic and formalize the agent interaction contract. Major tasks in this phase:

Implement the orchestrator's state machine that drives multi-agent workflows (initially sequentially or with simple branching). The orchestrator must be the single authority updating state, ensuring deterministic task ordering.

Define role-specific contracts for agents: what inputs they receive, what outputs they must produce, and what actions are forbidden. These contracts serve as a "spec within the spec" for each agent role.

Create the message envelope schema for agent communication. This includes fields like id (unique message ID), parentId (to link results to the triggering task), traceId (to tie all messages in a workflow together), agent (name or role), type (task vs result), payload (content of the request or result), and control metadata (timeouts, version numbers, etc.).

Establish versioning and TTL (time-to-live) rules for messages. For example, decide on a protocol version number to include in each message, and how to handle outdated messages or results that arrive late (possibly ignore if past TTL).

Lay groundwork for error handling and retry: define how the orchestrator represents failure states (e.g., an AgentResult with an error status), and how it will react (e.g., simple retry once, or mark workflow failed – advanced policies can be deferred but basic behavior should be specified).

Deliverables

Orchestrator Module: The initial implementation of the orchestrator component, capable of managing a workflow of tasks. For now, it can be simplified (e.g., execute tasks in a fixed sequence or a basic conditional flow), but it must handle multiple agent interactions in one run. It should include logic to send tasks to agents (via the SDK interface or a direct call to agent stubs) and wait/collect their results, updating an internal state (like a list of completed tasks).

Agent Role Contract Spec: Documentation of the contract for each agent role anticipated (even if not all are implemented yet). For example, CodeGenAgent – input: specification or prompt, output: code diff; forbidden from executing code. TestAgent – input: code or test description, output: test results; must not modify code. This spec ensures each agent's responsibilities and limits are well-defined, aligning with multi-agent coordination rules.

Message Envelope Schema: A concrete definition (in JSON Schema or equivalent) of the format for all inter-component messages (AgentTask, AgentResult, AgentError, etc.). This schema should list all fields and their types. For instance:

{
  "id": "uuid",
  "parentId": "uuid or null",
  "traceId": "uuid",
  "agent": "string",
  "type": "AgentTask",
  "payload": {...}, 
  "timeoutMs": 30000,
  "protocolVersion": 1
}


A similar schema for AgentResult including perhaps status and resultData. The envelope must incorporate version/TTL (e.g., a field expiresAt or an implicit rule that results after timeout are ignored) and any determinism flags (maybe a deterministic: true/false field to signal if an agent call was made with random sampling or not, for auditing).

Basic Agent Stubs: Implementation of a couple of simple agent stubs that adhere to the contract and can interact with the orchestrator. For example, a NoOpAgent that returns a fixed response immediately, and perhaps a DummyToolAgent that simulates performing a tool action. These stubs will be used to test orchestrator logic in absence of real AI calls.

Example Workflow & Log: A demonstrative workflow (could be a unit test or a scripted sequence) showing the orchestrator sending tasks to an agent stub and receiving results. Provide the resulting event log or trace output for that run as an example. It should show the chain of events with id and parentId linking tasks and results, confirming that the message envelope is being used consistently.

Execution Steps

Draft Agent Contracts: Start by listing out the agent roles we expect (e.g., Orchestrator itself, CodeGenAgent, TestAgent, etc., per spec context). For each, write a short contract description: inputs, outputs, invariants. Include forbidden actions (e.g., a TestAgent should not modify code, only read/execute tests). Use the spec's multi-agent coordination rules as a reference to ensure completeness. This will likely be an iterative draft reviewed with the team for completeness and clarity.

Design Message Schema: Using the agent contract needs and Phase 0's trace/ID scheme, design the message envelope structure. Decide on field names and ensure all needed information is captured (like including agent.role or agent.name, a workflowId if traceId doubles as that, etc.). Pay attention to avoiding duplication of information in telemetry – e.g., ensure we don't need to log something in two separate fields redundantly. Draft this schema and get consensus.

Implement Orchestrator Core: Develop the orchestrator logic in code. Likely this involves a loop or function that goes through a series of steps: (a) create an AgentTask message, (b) log it and maybe start a span for it, (c) deliver it to the target agent (calling the stub synchronously or via an async interface), (d) await result or timeout, (e) on result, log it (and end span), then decide next step (move to next task or finish). Ensure that state transitions are deterministic – e.g., if an agent is slow or out-of-order, orchestrator should still handle in a consistent way (for now, maybe one at a time, so out-of-order isn't an issue yet).

Integrate with Event Log: Hook the orchestrator up to Phase 0's event log. Each time an AgentTask is dispatched or an AgentResult is received, append it to the log with all required metadata. This not only tests our logging in a multi-step scenario but also ensures we capture every interaction. Use the IDs properly: e.g., set parentId of a result to the id of the originating task so we can correlate them in logs. If a task spawns sub-tasks (maybe simulated now), propagate the traceId to them.

Timeout/TTL Handling: Implement basic timeout handling: e.g., if an agent stub doesn't return within timeoutMs, have the orchestrator record an error or mark that task as failed. Use the TTL field if included – e.g., orchestrator can ignore results that come after their TTL. This ensures no ghost events disturb determinism (the spec likely wants that if something comes late it's not applied).

Use Agent Stubs for Testing: Utilize the stub agents to simulate a short workflow. For instance, Orchestrator sends a task to NoOpAgent, which returns immediately, then sends another to DummyToolAgent, which returns. Ensure orchestrator aggregates results correctly (maybe just logs them and finishes). Write tests asserting that the event log contains a Task event followed by a matching Result event with correct parent linking. Also test an error path: perhaps have a stub simulate a failure or no response to test timeout logic.

Review and Refinement: Have the team or a domain expert review the agent contracts and message schema. Confirm that all necessary fields are present and that forbidden actions cover potential misuse (this ties into later policy engine). Also review orchestrator logic for any edge cases (like what if an agent returns unexpected data – ensure it's handled). Incorporate feedback to refine contracts or add comments in code to clarify intentions.

Acceptance

Deterministic Orchestrator Behavior: The orchestrator processes tasks in a predictable, repeatable way. In tests, running the same sequence of tasks yields the same event log (same ordering, same IDs if seeded). There is no race condition or nondeterministic branching at this stage. This fulfills the spec mandate for a deterministic orchestrator core.

Complete Agent Contracts: The agent contract documentation covers all key roles with clear input/output specs and explicitly lists forbidden behaviors. Reviewers agree that these contracts address the expected responsibilities of each agent type (as derived from the spec use cases). No role is left ambiguous. This will guide both development of agents and the policy engine rules in Phase 5 (ensuring we already know what to forbid/allow).

Message Schema Matches Requirements: The message envelope includes all fields needed for tracing, version control, and control flow. For example, it has a field for version (so we can evolve the protocol), a timeout or expiration (to avoid lingering tasks), and carries correlation IDs (trace and parent IDs) correctly. The schema was tested by sending a message and observing in log or debugging that all fields are populated as expected. Importantly, there's no duplication of info across messages and telemetry (e.g., we don't log the same ID in two separate places unnecessarily).

Orchestrator & Log Integration: The orchestrator successfully logs every task dispatch and result. An example log sequence from a workflow shows an AgentTask event with some ID, and later an AgentResult event with parentId equal to that ID, and the traceId constant throughout. This demonstrates full traceability of the multi-step process. If the orchestrator encounters a timeout or error, it logs an appropriate error event. The event log thus captures the state machine progression step by step.

Basic Workflow Functionality: A simple end-to-end test workflow runs without issues. For instance, orchestrator -> NoOpAgent -> result -> orchestrator -> DummyToolAgent -> result -> orchestrator finish. The orchestrator correctly handles the sequence: it doesn't crash, it respects timeouts (if we simulate a long wait, it times out as configured), and it can move to the next task based on the result of the previous (even if that logic is basic now). This proves the orchestrator can coordinate multiple agents in one run, achieving the multi-agent execution concept.

No Telemetry Duplication: As per spec guidelines, we verify that our instrumentation does not double-log or create conflicting telemetry. For example, we ensure that if an AgentTask event is recorded, we're not also logging a nearly identical "started task" message elsewhere. The acceptance is that the observability data is clean and each piece of information has one source of truth (span or log). This was confirmed by inspecting logs/traces and our logging code paths.

Alignment with Spec & Phase 0: The outcomes of Phase 1 are consistent with the earlier design and spec. The orchestrator upholds the single-writer principle (only it mutates workflow state) and deterministic behavior as required. The message schema is compatible with any standards we aim to support (for instance, if the spec mentioned A2A, we ensured our schema can map to it easily). There are no features creeping in that spec didn't ask for (we focus on required functionality). Overall, Phase 1 deliverables set a solid core in line with all specified expectations.

Phase 2 — SDKs & Metadata Model

Scope

Build developer-facing SDKs in Python and TypeScript and establish a unified metadata/event model they share. Key aspects:

Implement a Python SDK that wraps orchestrator functionality, allowing users to easily define agents (e.g., via subclass or decorator) and start workflows from Python code. It should abstract away low-level details (like constructing message envelopes) while ensuring all necessary metadata is attached.

Implement a TypeScript SDK with similar capabilities for Node.js/TypeScript developers. It should mirror the Python SDK's interface and behavior to maintain consistency across languages.

Define a unified event metadata model that specifies what information is captured for each event or agent interaction. This includes defining standard attributes (like workflowId, agentName, toolName, tokensUsed, etc.) and ensuring both SDKs and the core runtime use these consistently in logs and traces.

Ensure cross-language compatibility: the Rust core orchestrator is the single control-plane service; both SDKs communicate with it via gRPC/HTTP. Behavior must be identical across SDKs.

Provide hooks or extensibility in SDKs anticipating integration: for instance, allow registering custom agent implementations, and possibly allow the internal agentic SWE agent to plug in external logic through these SDKs.

Deliverables

Python SDK Package: A Python module (or set of modules) that includes:

An Agent base class or decorator for defining new agents. Developers can subclass it and implement a run(task) method, for example.

A Workflow or OrchestratorClient class that provides methods to submit tasks or plans to AgentMesh. For example, mesh = AgentMesh(...); mesh.run(plan) which internally handles connecting to the orchestrator and streaming results or final output.

Data model classes corresponding to the message schema (AgentTask, AgentResult) so that Python developers can interact with structured data if needed (or these could be simple dicts hidden behind the scenes).

Integration with Python logging or OpenTelemetry to funnel SDK-level events into the core telemetry (ensuring e.g. if a user uses the SDK, their actions are traced under the same traceId).

Documentation (docstrings and a README section) with examples of usage.

TypeScript SDK Package: A Node.js library (to be published to npm) that includes:

Similar abstractions: perhaps an AgentMeshClient class and an Agent interface for defining agent behavior in TS.

Type definitions that mirror the message schema (could be generated from the JSON schema to avoid divergence). E.g., a TypeScript AgentTask type with fields exactly as defined.

Methods to start workflows and receive results asynchronously (likely using async/await or event emitters). For example, a runWorkflow(plan): Promise<WorkflowResult> or a stream of events.

If needed, an internal mechanism to communicate with the core (e.g., making HTTP requests to a Python service or spawning a Python subprocess if the orchestrator runs in Python). This should be abstracted from the user.

Documentation and usage examples in the README (showing how a developer would use it to accomplish a task).

Unified Metadata Model Spec: A document or structured data (maybe part of the code as constants) listing all event/telemetry metadata keys and their meanings. For example: define that every event will have workflowId, agent, role, timestamp, duration, tokenCount, etc., and describe each. This model ensures consistency: e.g., the Python SDK might auto-add a workflowId to each log event, and the TS SDK does exactly the same.

Cross-Language Test Results: A set of tests or a small report demonstrating that workflows run via Python and via TypeScript yield equivalent outcomes. For instance, run a simple two-agent sequence in Python only, TS only, and (if applicable) one where a Python agent calls a TS agent or vice versa (depending on integration mode). Show that the logs/traces from these runs align and that both SDKs correctly handle the orchestrator interactions.

Extensibility Hooks Documented: If applicable, notes or examples on how one might integrate an external framework's agent through these SDKs. For instance, an example where the Python SDK's agent calls out to a LangChain pipeline, demonstrating that our system can wrap external logic. This foreshadows Phase 7 integration and assures that we have not designed ourselves into a corner.

Execution Steps

Design SDK API (Python): Outline what the Python developer experience should look like. Aim for simplicity: e.g., @agent decorator to turn a function into an agent, or a base class with a clear method. Decide how workflows are initiated: perhaps a high-level function execute(workflow_plan) over an SDK client that calls the Rust orchestrator service.

Implement Python SDK: Write the classes/functions decided above. Implement an SDK client to the Rust orchestrator (gRPC/HTTP). Ensure that when tasks are submitted via SDK, they carry all required metadata (attach traceId if not provided, start spans, etc.). Use the unified metadata model.

Design SDK API (TypeScript): Mirror Python concepts for TypeScript. Communicate with the Rust orchestrator via the same service API.

Implement TypeScript SDK: Set up a Node project, define AgentMeshClient. Use native fetch/HTTP or gRPC bindings to call the Rust orchestrator. Implement message types matching the schema. Provide async streaming interfaces if applicable.

Metadata Model Unification: Create a central reference (e.g., JSON/YAML) listing all metadata fields; both SDKs auto-attach the same fields.

Testing & Alignment: Write tests for each SDK in isolation; run integration tests against the Rust orchestrator service. Adjust to unify behavior.

Team Review: Have both an experienced Python dev and a TS dev on the team try out the SDKs (or at least review the API). Get feedback on usability and consistency. For instance, ensure naming is analogous (if Python uses run_workflow, TS shouldn't use a completely different term). Align the terminology with industry standards if possible (maybe look at how LangChain or others expose APIs, to be familiar to users).

Adjust for Extensibility: Think ahead to integration: ensure that the SDKs will allow hooking in custom behavior. For example, can a user override how an agent action is executed (maybe needed when plugging external frameworks)? Possibly add an interface for tool execution or allow passing a custom function to an agent. Document these extension points for later use.

Acceptance

Python SDK Functional: A developer can use the Python SDK to define and run a simple multi-agent workflow without dealing with low-level details. This is verified by an example or test where a custom Python agent is created (using our base class/decorator) and the orchestrator (via SDK) successfully calls it and gets a result. The developer-facing API is clear and minimal (e.g., they don't have to manually create IDs or open log files – the SDK handles it).

TypeScript SDK Functional: Similarly, a developer using Node.js can invoke AgentMesh workflows. We demonstrate this by starting a workflow from a Node script that contacts the orchestrator and returns a correct result. Any needed background communication (like to the Python process) is encapsulated – the user just calls methods on the TS SDK. The TS SDK also properly types the interactions (TypeScript types match the actual JSON fields).

Consistency Across SDKs: The Python and TS SDKs yield consistent behavior and use the same underlying model. For instance, if a workflow is run in Python vs TS with the same agents, the event logs and outcomes are equivalent. The unified metadata model is evidently enforced: we can see that an event logged from a TS-initiated run has the same fields as one from a Python-initiated run. No important metadata is missing in one or the other. This consistency was confirmed by cross-testing and inspecting logs.

Unified Metadata Model Established: There is a definitive list of what metadata is tracked for each event, and both SDKs and the core abide by it. For example, we decided on workflowId vs traceId usage, and that decision is reflected everywhere. The acceptance here is that a single source of truth exists for event field definitions, and any consumer of our logs or traces can rely on those fields being present and uniformly meaning the same thing across languages. This makes later analysis or integration (like plugging into monitoring tools) much easier.

Cross-Language Interoperability: If our design requires the Python core and TS SDK to interact, this interaction works robustly. We tested failure cases too (e.g., if orchestrator isn't running and TS SDK tries to connect, it handles it gracefully or with a clear error). The handshake of starting a workflow, exchanging messages, and completing is verified. There are no deadlocks or mismatches (like TS expecting a field name slightly different than what Python sends – those were ironed out).

Documentation & Developer Experience: The usage of each SDK is clearly documented with examples, and early feedback from team members indicates it's easy to use. For instance, someone other than the SDK author was able to write a short script using the SDK just by reading the docstring/README example, and it ran successfully. This is a good proxy for a positive developer experience which was a goal in the spec (to be developer-local and friendly).

No Regression in Core Behavior: The introduction of the SDKs did not break the core orchestrator logic or determinism. We ensure that when using the SDK, the determinism holds (if anything, the SDK should enforce it by controlling random seeds or execution order on the client side too). Also, telemetry from SDK-run workflows is as complete as before – e.g., no missing spans because of the abstraction. Essentially, the SDK is a thin layer that doesn't impede our core features.

Prepared for Integration: By the end of Phase 2, we have confidence that AgentMesh can be driven from different environments (Python, Node) without issues, which sets the stage for integrating with other systems. The acceptance is that there's no obvious blocker to, say, the internal SWE agent using these SDKs to connect external agents. If we identified any gap (like missing ability to stream intermediate results, etc.), we either implemented it or noted it for later, ensuring nothing fundamental is missing for integration scenarios.

Phase 3 — Budget & Cost Governance

Scope

Integrate cost monitoring and enforcement mechanisms to ensure AgentMesh runs are cost-governed. This includes:

Instrumenting all LLM API calls or tool invocations to capture usage metrics (token counts, API call counts, etc.).

Maintaining counters for each workflow (and possibly per agent) of cumulative tokens used and/or approximate cost in dollars.

Providing configuration for budget limits (per workflow or global) so that users can set a maximum allowed cost or token usage.

Implementing logic to proactively enforce these limits: e.g., halting or pausing agent execution when a budget is about to be exceeded, and cleanly terminating the workflow if the budget is hit.

Emitting telemetry about costs and usage (so that observability covers not just what agents did, but how much it cost and where resources went).

This phase ensures that a runaway agent or an inefficient plan cannot silently rack up unbounded costs – it will be contained and visible.

Deliverables

Usage Tracking Hooks: Modifications in the agent execution flow to measure resource usage. For example, wrap the LLM API client in a proxy that counts tokens (if using OpenAI API, use their response usage fields; if a different model, maybe count prompt/response tokens via a tokenizer library). Also track number of tool invocations if needed (some tools might have cost implications, like API calls).

Budget Configuration Interface: A config file or parameters for specifying budgets. E.g., an entry in a config: max_tokens_per_workflow: 10000 or max_cost_usd: 5.00. Alternatively, allow passing a budget parameter when starting a workflow via the SDK (e.g., mesh.run(plan, max_cost=5.0)). This deliverable is the mechanism by which a user or admin sets the limits.

Budget Enforcement Logic: Implementation in the orchestrator (or a dedicated BudgetManager component) that checks the current usage against the limits after each agent action. If an impending action would exceed the budget, it either prevents that action or stops the workflow. This could include sending a special event (like BudgetExceeded) to the log and gracefully shutting down agents. Possibly implement a "soft limit" warning threshold as well to log a warning when, say, 90% of budget is used.

Metrics and Telemetry for Cost: Extend the telemetry model to include cost metrics. For instance, each AgentResult event could carry tokens_used for that call, and perhaps a running total. The system might also log an overall summary at workflow end: "Workflow finished, total tokens: X (prompt Y, completion Z), est. cost $C". If using OpenTelemetry metrics, define counters/gauges for tokens and cost that could be scraped.

User Feedback & Documentation: Visible feedback when budgets are hit. E.g., if a budget is exceeded, the SDK should raise an exception or return an error status so the user knows the run stopped due to budget. Also produce documentation explaining how to configure budgets and what happens when limits are reached (with examples like "if max_tokens=1000 and you input a very large document, the agent will stop reading further or abort with BudgetExceeded error").

Test Scenarios: A set of test cases demonstrating that budget limits work. For instance:

A workflow with a very low token limit triggers a BudgetExceeded halfway through and stops further agent calls.

A workflow within limits completes normally and reports actual usage.

If applicable, multiple workflows in parallel each enforce their own budget without conflict (depending on single-track, likely sequential).

A scenario where an agent would exceed per-agent limits (if we set such) is prevented (like an agent tries to output 5000 tokens but we set max 1000 per agent, it's truncated or stopped).

Execution Steps

Implement Usage Counters: Identify where LLM calls occur (likely in agent implementations, e.g., CodeGenAgent calling OpenAI API). Modify these calls to record the number of tokens in prompt and response (the API typically returns counts). Sum them up in a workflow-scoped counter. If an agent uses tools that call external services, instrument those too (maybe log each call, count them if relevant). Encapsulate this in a BudgetManager class that orchestrator can query.

BudgetManager & Config: Create a BudgetManager (or integrate in orchestrator) that holds the budget limits and current usage. Parse budget settings from a config or function parameters at workflow start. For example, orchestrator might be initialized with budget={'tokens': 10000, 'usd': 5.0} which BudgetManager stores. Each time an agent finishes, call BudgetManager.update(consumed_tokens) and check status.

Enforcement Mechanism: Decide the strategy for enforcement. A straightforward approach: when usage exceeds limit, orchestrator stops scheduling further tasks and labels the workflow as terminated due to budget. Implement this: perhaps raise a controlled exception that unwinds the workflow loop, or set a flag that prevents new tasks from being dispatched. Also ensure any running tasks are either allowed to finish or are cancelled if possible (cancelling mid-LLM call might not be possible, but we can at least not use the result). Log an event such as {"event": "budget_exceeded", "limit": "...", "value": "..."} for audit.

Warning Thresholds: (Optional) Implement a warning at, say, 80% of budget usage. This can be a simple log or callback. For instance, log "Warning: 80% of budget used" which can help in debugging or user intervention if interactive. This aligns with suggestions like winding down when near limit.

Telemetry Augmentation: Update the metadata model to include cost metrics. For example, add tokensUsed and tokensRemaining fields to relevant events. If using OpenTelemetry, create a metric instrument (counter) for total tokens and increment it. Possibly tag metrics by agent type for insight (e.g., CodeGenAgent used 500 tokens, TestAgent used 200, etc.). Test that these metrics show expected values after a run.

SDK Integration: Ensure the SDKs (Python/TS) expose budget config to users. E.g., allow AgentMesh(..., max_tokens=1000) or an environment variable AGENTMESH_MAX_TOKENS. Also handle the case when budget is exceeded: the Python SDK could throw a BudgetExceededError, and the TS SDK might reject a promise with a similar error. Implement those error types and document them.

Testing Budget Enforcement: Write unit tests where the agent behavior is controlled such that we know how many tokens will be used. One approach: create a dummy agent that "consumes" a known amount (maybe a stub that on each call, increment a counter by N). Use it to simulate hitting the limit. Verify that after hitting limit, orchestrator stops scheduling further calls. Also test a scenario just below the limit to ensure it doesn't falsely trigger.

Review & Tuning: Have team members review whether the budget logic covers typical use cases. For example, if the spec or prior research mentioned "Max budget per workflow" explicitly, ensure we did exactly that. If they mention also limits like "max tokens per agent output", consider if we should implement that too here (maybe yes – e.g., truncate or disallow outputs beyond a length). Implement any such additional limits if easy (could be done by instructing the LLM to limit itself or post-processing).

User Documentation: Update the user docs (or README) with a section on Cost Management: explain how to configure budgets, what the runtime does when limits are hit, and how to interpret cost-related logs. Possibly include a tip like using OpenAI's own monthly limit features as backup, but within AgentMesh this is how we control it.

Acceptance

Accurate Usage Tracking: The system accurately counts tokens/cost for each agent call. This is confirmed by comparing our internal counts with external data. For example, if a conversation used 1000 input tokens and 800 output tokens (per OpenAI), our BudgetManager total for that agent is 1800. Tests with known token strings verify our counting logic (if we use our own tokenizer). If any approximation is used (for cost $), it is documented and reasonably close.

Budget Limit Enforcement: In test scenarios, workflows respect the set budgets. For instance, setting a low token limit causes the workflow to terminate once that many tokens have been processed. The termination is graceful: the orchestrator stops scheduling new tasks and logs an appropriate message/event. No uncontrolled continuing after limit was noticed. If a budget is hit mid-agent call, we handle it in the best possible way (maybe allow the call to finish but then not proceed further, logging that it went over by X).

User Notification: When a budget is exceeded, the user (developer) is clearly informed. For example, the Python SDK raises an exception BudgetExceededError: Token budget of 1000 exceeded (used 1100). The TS SDK promise is rejected with a similar message. The logs also contain an entry about budget exceeded for audit. Conversely, if a run completes without hitting budget, it maybe logs total usage but no error. This meets the spec's intent of being cost-governed: users are aware of costs in real time, not after the fact.

Configurable & Flexible: Users can easily adjust the budget settings. We demonstrate that by changing a config or parameter, the limit changes accordingly. Also, if no budget is set, system defaults to tracking usage without enforcing (or a reasonable default). We likely choose to have no default hard limit (so existing workflows won't break unexpectedly), but track usage and allow optional limits. This flexibility is documented. Essentially, the feature can be turned on or off or tuned as needed, which is important for different environments (dev vs prod).

Telemetry Includes Cost Info: The observability data now reflects resource usage. We can see in the log or trace attributes how many tokens each agent used and cumulative totals. A trace viewer or log analyzer could thus be used to pinpoint which step in a workflow was most expensive, etc. If integrated with monitoring, one could set up an alert on unusually high token usage. This satisfies enterprise observability needs for cost tracking.

No Large Performance Penalty: Adding budget checks did not significantly slow down normal operation. This is checked via benchmarking or at least reasoning (counting tokens is trivial compared to making an LLM API call, and our checks are after each agent which is fine). Memory overhead of counters is negligible. So the system remains efficient while adding this safety layer.

Edge Cases Handled: The system behaves sensibly at budget extremes. For example, if budget is zero, maybe it refuses to run any agent (immediate stop). If budget is extremely high, it never triggers and doesn't degrade performance. If an agent returns usage data unexpectedly (maybe none), we handle it by estimating or ignoring properly (document if needed). These edge cases tested and documented.

Aligns with Spec Guidance: The implementation aligns with any spec or blueprint notes on budgeting. The blueprint suggested having config for max tokens and cost and gracefully handling approach to limits – which we did. It also mentioned skipping optional steps if near limit – if we implemented warning thresholds, we partially address that (though actual step skipping might be more complex; at least we warn and/or stop). The key is we satisfied the requirement of not overrunning cost constraints in an uncontrolled way, fulfilling the "cost-governed" aspect of AgentMesh.

Phase 4 — Observability & Debugging Tools

Scope

Enhance observability features to full production quality and introduce advanced debugging capabilities such as trace visualization and time-travel replay. Focus areas:

Ensure complete tracing coverage of the system: every agent invocation, tool use, and important decision should emit a trace span with the correct parent-child relationships. This might involve adding spans for internal steps that were not covered in Phase 0/1 and refining attributes (ensuring they are low-cardinality and useful).

Implement trace sampling and filtering policies to manage high-volume traces. For example, allow configuration to sample only X% of runs or to only trace certain agents deeply (to avoid overhead if thousands of events occur).

Develop a time-travel debugging capability: the ability to replay a recorded execution trace step-by-step. This likely involves using the event log to simulate the orchestrator's decisions without calling external APIs (relying on logged responses). Possibly provide a CLI tool or interactive UI to traverse events.

Provide a way to visualize traces for easier understanding of multi-agent workflows. This could be integration with existing tools (like exporting to an OpenTelemetry backend such as Jaeger or Zipkin) or a custom lightweight visualization (maybe generating a sequence diagram or Gantt chart of agent actions).

Extend logging with structured logs for all events and ensure logs (and any UI) redact sensitive info (tying in Phase 5 policies) while still providing meaningful debug data.

Essentially, by end of this phase, a developer or operator should be able to observe what each agent did, in what order, how long it took, and be able to replay and inspect the state at each step for debugging.

Deliverables

Span Coverage Report: An audit (possibly automated or documented) of all components to confirm tracing is in place. E.g., a list of all major functions/steps with a note that a span is emitted. If anything was missing (like internal logic chunks), spans added. Update of trace instrumentation code where needed (maybe orchestrator loop now has spans for each phase).

Configurable Sampling Policy: A configuration (in code or file) allowing specification of trace sampling. E.g., an option sampling_ratio: 1.0 for all spans, or sampled_agents: [CriticalAgent] for detailed trace of some agents only. The deliverable is the implementation of these options in the tracing setup. Ensure by default we do full tracing (for dev) but can reduce for performance.

Replay Tool/Mode: A new tool or mode that can replay an execution. This could be a command-line interface (CLI) utility, e.g., agentmesh replay --trace-id=<id> which reads the event log for that trace and re-runs the orchestrator logic using the recorded events as inputs. Possibly interactive: stepping through each event with the ability to inspect the state (like the content of a message). If not interactive, at least produce a step-by-step log. This might also be delivered as a Jupyter notebook or script that uses the AgentMesh API to load a trace and iterate through it.

Trace Visualization Utility: Possibly integration with Jaeger or similar by exporting spans to it (since we use OTel, we can attach to a Jaeger exporter). Alternatively, provide a script to convert a trace log into a graphical format (like Mermaid.js sequence diagram). A simple deliverable could be a generated HTML or image for a given trace demonstrating the hierarchy of agent calls (or instructions for users to view traces in an off-the-shelf UI).

Enhanced Logging & Debug Info: Improvements to logging such as including timestamps, durations, and result summaries in structured logs. E.g., when an agent completes, log an event with {..., "duration_ms": 2500, "output_summary": "Test passed"}. Also ensure logs are properly rotated or segmented by workflow to avoid one giant log (maybe one log file per workflow/trace for easier analysis).

Documentation – Debugging Guide: A new section in documentation that guides a developer on using these observability and debugging features. It should explain how to enable sampling, how to run the replay tool, how to view traces (maybe "Point your Jaeger UI to this service" or "open the generated diagram"), and give an example of diagnosing an issue using these tools.

Execution Steps

Span Instrumentation Audit: Review each part of the code (orchestrator start, each agent execution, each tool usage, budget events, etc.) and ensure a span is created where appropriate. For nested operations, ensure parent context is passed (OpenTelemetry context propagation). Add missing spans, e.g., around a tool call inside an agent. Keep span names concise (agent.CodeGen.execute, tool.GitFetch.call, etc.) and use attributes from the metadata model for details.

Implement Sampling: Leverage OpenTelemetry's samplers if possible. For example, set up a parent-based sampler that samples X% of traces based on traceId. Or implement logic to drop spans of certain types if a flag is off. Provide config knobs (perhaps env vars or config file entries) for sampling rate and filters. Test sampling by configuring 0% and confirming no spans recorded, and 100% for all spans.

Develop Replay Mechanism: This is complex – likely involve running orchestrator in a mode where instead of calling the actual agent logic, it reads from a log of recorded results. One approach: create a subclass of orchestrator or a mode orchestrator.replay(log) that will iterate through the events. It would take each AgentTask event and instead of actually sending to agent, immediately fetch the corresponding AgentResult from the log and use that. Implement checks to ensure the sequence matches (if something expected is missing, warn). Also handle branching: if the orchestrator logic might normally branch based on content, in replay we already have what it did, so orchestrator should follow the same branch by using recorded decisions. Essentially, orchestrator uses recorded outputs to drive its state transitions identically. Ensure determinism by using exact same IDs and order. Provide output (via print or UI) at each step for user.

Interactive CLI/UI: Implement a simple CLI for replay. For instance, allow stepping: maybe pressing Enter moves to next event, printing out the event data and maybe any state changes (like "Agent X output Y, orchestrator now scheduling Z"). If building a small TUI (text UI) is feasible, could show a list of events and highlight current. Alternatively, implement replay in a Jupyter-friendly way (since developers might use that) – e.g., provide a replay(trace_id) function that prints out all steps.

Export/Visualization: If using Jaeger or Zipkin, configure the OTel exporter accordingly (maybe optional dependency). Test sending spans to a local Jaeger instance and verify the trace can be viewed with spans and timing. If not using Jaeger, implement a converter: read our event log and produce a .dot file or Mermaid markdown for the sequence. Could output something that shows agents on vertical axes and messages between them (like a sequence diagram). Provide a script or command for this, e.g., agentmesh visualize --trace-id XYZ.

Structured Log Enrichment: Add fields like duration_ms to AgentResult events using span timings or internal timers. Ensure each log entry has a timestamp (if not by default). Implement log rotation or separation: maybe open a new log file per workflow (include workflow ID in filename) to ease isolating a single run's events. Document log file naming convention and location.

Integrate Redaction (with Phase 5): Since we're improving logging, ensure that if any sensitive info (like a user prompt) is logged, it either passes through the policy engine or we apply redaction here too. Possibly tie into the policy engine's redaction functions when writing out logs. Test that replay and visualization show redacted content where appropriate (so we are not accidentally exposing PII in debug outputs).

Performance Consideration: Evaluate the overhead of full tracing. Perhaps measure how many spans per second we can emit, or ensure that heavy logs don't slow down execution too much. If overhead is high, that's why sampling is introduced – test that turning on sampling dramatically reduces overhead in a stress scenario.

Write Debugging Guide: Compile steps of how to use these tools into documentation. Maybe even create a small example scenario with a known bug (like an agent that does something wrong) and walk through diagnosing it with the replay. This will illustrate the value and usage of the features.

Demo & Feedback: Demonstrate the trace visualization and replay to the team or some users. Get feedback: e.g., is the output of replay understandable? Do we need to highlight certain info more? Iterate if necessary to improve usability (maybe adding color coding in CLI or more context around events, etc.).

Acceptance

Comprehensive Traceability: After this phase, it should be possible to trace the entire execution of a workflow from start to finish. In practice, this means if an issue occurred, one can find it in either the logs or the trace spans. We confirm this by running a complex workflow (multiple agents, perhaps parallel tasks if orchestrator allows by now) and observing that every agent action and internal decision has a corresponding log entry and/or span. The trace relationships (parent-child) in a trace viewer correctly represent the call hierarchy (e.g., Orchestrator -> AgentTask -> Agent internal tool calls, etc.).

Controlled Tracing Overhead: The sampling and filtering work as intended. For a trivial workflow, full tracing is fine (and default for dev). For a stress test with thousands of events, turning sampling down (or off certain spans) results in significant reduction of overhead and log size. We have documented recommended settings for production vs dev. The acceptance is that we can support high-volume scenarios without overwhelming the system by dialing down observability detail as needed, addressing potential "trace cardinality creep" risk with allowlists and sampling.

Replay Functionality: We can take a real log from a prior run and replay it such that the orchestrator's behavior is reproduced step-by-step. This is proven by, for example, replaying a run and comparing important state or outputs to the original – they match exactly. If randomness was present originally, the replay uses recorded outputs to bypass it, yielding the same final result. Developers are able to follow along each step, seeing what inputs an agent got and what it responded, satisfying a core need for debugging and compliance ("why did it do X?" can be answered by examining the replay).

User-Friendly Debug Tools: The CLI or interface for replay and visualization is reasonably easy to use. During internal testing, team members were able to invoke a replay and understand the output without needing to read through raw log files manually. The visualization (e.g., Jaeger UI or our diagram) clearly illustrates the agent interactions timeline, and was verified by inspecting a non-trivial trace. This aids understanding complex workflows quickly, as envisioned.

Rich Structured Logs: The logs now include timing and summary information that make them far more useful. For example, one can skim a workflow's log and see durations of each step and key results (maybe truncated to avoid giant outputs, but enough to get a sense). Sensitive data is not present in logs (thanks to redaction rules), so logs can be shared with others or stored without compliance issues. Logging is segmented so that analyzing a single workflow's log is straightforward (we can open workflow_<id>.log and get exactly that run's events).

Integration with Tools: If applicable, we demonstrate integration with an external trace tool (Jaeger/Zipkin). If we set it up and run AgentMesh with it, the traces appear correctly and can be navigated. This shows that our observability aligns with industry-standard tools, a plus for enterprise environments.

Debugging Guide & Documentation: The new documentation is thorough. A developer new to AgentMesh can read the debugging guide to learn how to turn on tracing in production (and the performance implications), how to replay a run if something went wrong, and how to interpret the outputs. This guide was reviewed and possibly tested by someone simulating a bug to see if they can follow it to find the cause.

No Regrets on Data: The observability enhancements do not inadvertently leak data or break determinism. For instance, injecting tracing does not change the execution order or outcome (thanks to careful use of no side-effect instrumentation). And any sensitive info captured (like agent prompts) are protected by policy (so e.g., if a prompt had a password, our policy engine from Phase 5 ensures it's not in plaintext in logs). We likely coordinate with Phase 5 implementation to verify this (since Phase 5 runs in parallel conceptually, but we'll consider its policies while finalizing logging).

Meets Enterprise Observability Needs: Overall, at this point AgentMesh provides a level of observability on par with or exceeding typical enterprise microservice tracing: unified tracing across agents, tools, and LLM calls. The unique addition is deterministic replay, which is rare and a competitive differentiator. The acceptance is that stakeholders concerned with debugging and monitoring are satisfied that the system won't be a black box in production; instead, it will be transparent and diagnosable, fulfilling one of the key goals.

Phase 5 — Policy Engine & Guardrails

Scope

Implement a policy enforcement layer to impose runtime guardrails on agent behaviors and content, ensuring safe and compliant operation. Key elements:

Content Moderation: Integrate content filtering for prompts and outputs. The policy engine should detect disallowed content (e.g., profanity, hate speech, sensitive data patterns) and take action (block or redact) before it reaches an external API or the user. This can use simple pattern matching or external moderation APIs.

Tool Usage Restrictions: Enforce which agents can use which tools and under what conditions. For example, prevent a CodeGenAgent from calling an Internet search tool if that's outside its scope. The orchestrator or a proxy should check a policy rule before executing any tool action and deny it if not permitted.

Automatic Redaction: Ensure that any sensitive information (PII, secrets) that agents might output is automatically redacted in logs and possibly in responses, according to configurable rules. E.g., credit card numbers replaced with "[REDACTED]". Tie this into content moderation (some detection patterns serve both blocking and redaction).

Audit Logging of Policy Events: Every time a policy rule triggers (content blocked, tool denied, etc.), log an audit event detailing what was blocked and why (without exposing the blocked content itself if sensitive). This provides an audit trail for compliance – one can review what the AI attempted that was against policy.

Policy Configuration: Provide a way to configure the rules without code changes. E.g., a JSON/YAML policy file listing forbidden content patterns, allowed tools per agent role, etc., that can be modified as needed. Make the policy engine data-driven to adapt to different org's needs.

By end of this phase, AgentMesh should have a robust safety layer: it's not just controlling cost, but also the nature of content and actions, reducing risk of harmful outputs or unauthorized operations.

Deliverables

Policy Rules Definition: A default policy file (e.g., policy.yaml) that defines initial rules:

Content rules: e.g., disallow content categories (could be simple like disallow offensive words, or using an official list). Also patterns for sensitive info like \d{16} for credit card or certain keywords.

Tool rules: mapping of agent roles to allowed tools (e.g., CodeGenAgent: ["WriteFile", "ReadFile"], WebAgent: ["HTTPGet"], etc.), possibly with context like time of day or environment if needed (maybe not yet).

Any global rules: e.g., "No agent should call external APIs that return financial data" (just hypothetical).

Policy Engine Module: Code that loads the above policy and checks each relevant event against it. This includes:

check_input(prompt), check_output(response) for content (returning OK or a violation type).

check_tool(agent, tool_action) for tools.

Redaction function redact(text) that masks sensitive substrings as per policy (this can be applied to logs or even agent outputs if we choose to sanitize before delivering).

Mechanisms to enforce decisions: e.g., if check_output flags disallowed content, the orchestrator might replace the output with an error or ask the agent for a different answer, etc. For now, simplest is to block and log.

Integrated Moderation (Optional): If an external API (like OpenAI Moderation or Perspective API) is available and policy desires, implement an interface to call it for more advanced content checks. Make it optional/configurable (e.g., only if API keys present).

Audit Log Entries: Extension of the logging system to include policy events. For example, when content is blocked, log an event: {"event": "policy_violation", "type": "content", "agent": "X", "reason": "prohibited phrase", "action": "blocked"}. Ensure these events do not contain the actual disallowed content (to avoid reintroducing it via logs).

Enforcement Pathways: Code paths in orchestrator or agent wrappers where the policy engine is invoked. E.g., before sending a prompt to LLM, run check_input; after getting LLM response, run check_output (and possibly modify or block). Before executing a tool, run check_tool. Ensure that if a rule triggers, the system either substitutes a safe value, raises an error to orchestrator, or skips the action, depending on rule configuration.

Tests for Guardrails: A set of test cases:

Content moderation: feed an agent a prompt with a known bad word or PII and verify the policy engine catches it and stops or alters it (and logs it). Similarly test agent outputs with disallowed content get filtered.

Tool restriction: try to invoke a tool from an agent that isn't allowed that tool, verify it's blocked and logged, and that the agent is informed (maybe gets an error result).

Redaction: simulate an agent output containing an email or SSN, ensure the log shows it redacted. Possibly test that the user-facing output is also redacted or replaced with a safe message, based on our decision.

Ensure false positives are minimal by constructing some borderline content and ensuring allowed content passes.

Documentation – Safety & Compliance: A doc section explaining the policy system, including how to customize the policy file, how the engine reacts to violations, and how to interpret audit logs. Also reassure how this helps with compliance (for instance, if using AI in a regulated environment, these audit logs and guardrails are necessary safeguards).

Execution Steps

Define Default Policies: Draft a basic policy file. Include a few obvious forbidden content patterns (maybe use widely-known bad word lists for testing, though not all inclusive). Also define allowed tools per agent from what we expect (we know from Phase 1 agent contracts what they should do, so align tools accordingly). Keep it simple and not too broad initially, to avoid excessive false triggers.

Implement Content Check: Develop content filtering function. Could use regex for patterns (like email regex, etc.) and substring match for profanity (or a simple word list). If external API is to be used, implement a stub or hook: e.g., a function moderate_via_api(text) that returns categories or scores – integrate if keys configured. Decide on actions: e.g., if minor policy violation (maybe just log), if major (block outright). For simplicity, likely treat any hit as block. Implement accordingly.

Implement Tool Check: Use Phase 1's agent-role definitions and Phase 2's metadata. For every tool invocation, intercept either in orchestrator or a centralized ToolManager. Compare agent's role to policy's allowlist. If not allowed, prevent execution: e.g., skip calling the actual function and instead return a failure to the agent (like "Tool not allowed" error). Ensure orchestrator can handle that (likely as an AgentResult error).

Integrate with Orchestrator/SDK: Insert hooks: before sending prompt to model -> if policy.check_input(prompt) not OK: abort/modify. After receiving output -> if check_output fails: either drop it or replace with "[REDACTED OUTPUT]" and mark it in result. For tool usage, in the code that dispatches tool calls (if our design uses a ToolRegistry to call actual tools), do if not check_tool: raise exception or skip. Make sure these exceptions get logged and don't crash the system but rather inform orchestrator to continue or stop gracefully.

Redaction Implementation: For any content that might go into logs or out to user that contains sensitive info as per policy (like detected PII), run the redaction function. The redaction function could, for example, replace digits in certain patterns with "X" or whole strings with "[REDACTED]". Use non-greedy patterns to avoid over-redacting. Test on known strings: ensure something like "Call me at 123-456-7890" becomes "Call me at [REDACTED]".

Audit Logging: Extend the logging mechanism from Phase 4: add a function to log audit events. Possibly use a separate logger or file for audit, or tag events as "audit": true. Log details like agent name, rule triggered (but not the content). E.g., "Content policy violation by Agent Debug: contained disallowed 'password' – output blocked." Ensure these logs have enough info for offline analysis but no sensitive data.

Testing Guardrails: Create scenarios to test each type of violation:

For content: maybe have a dummy agent whose prompt or output we can control in tests. Feed it bad content and verify orchestrator doesn't send it to the LLM (if check_input triggered) or doesn't propagate a bad output to logs/user (if check_output triggered).

For tools: create a dummy tool call from an agent that's not allowed. The orchestrator should intercept and respond with an error. Confirm the tool was not actually executed (perhaps by having a flag if it was called).

For redaction: directly call redact() on a sample text with PII and see the output string; also simulate that going through log and ensure the actual log file content is redacted.

Also, test that allowed content/tools go through unaffected to ensure normal operation isn't hampered.

Iterate on False Positives/Negatives: Fine-tune the patterns to minimize blocking of benign content. For example, if we block "secret" and an agent legitimately uses the word "secret" in a harmless context, do we want that? Maybe allow context or severity levels. Since comprehensive NLP filtering is complex, be conservative on content blocking to avoid hindering normal ops. Document these considerations.

Coordinate with Security Phase: Ensure that any PII detection here complements Phase 6's PII handling. Possibly the same regex list can be used for scanning logs in security. The policy engine's redaction is a primary line of defense for logs; Phase 6 might add encryption or more formal PII scanning. Make sure not to duplicate too much or conflict.

Documentation & Config Guide: Write up how an admin can edit the policy file. For instance, if a company wants to add "internal project codename ABC" to forbidden outputs (to prevent leaks), they can add it to a list of blocked terms. Explain how to reload policy (possibly need restart unless we implement dynamic reload). Emphasize that this provides an audit trail for all such events which can be reviewed for compliance.

Acceptance

Effective Content Moderation: Testing shows that obvious disallowed content is caught and not allowed through. For example, if we mark a certain slur as disallowed, an attempt by the AI to output it gets blocked or sanitized. If an agent tries to output an SSN-like number, the logs and any final answer have it redacted (we see ***-**-**** or [REDACTED] instead of the actual number). The system thus avoids unwittingly outputting clearly sensitive or policy-violating content. We also ensure this doesn't crash the agent but rather the agent either gets an error or the user sees a message like "[Content removed due to policy]".

Tool Use Constrained: Agents cannot perform actions outside their allowed scope. We demonstrate that by trying to force an agent to use a tool it shouldn't (maybe via a crafted prompt or a test harness) and observing that the orchestrator refuses it. The attempt is logged in audit. Meanwhile, normal allowed tool usage works as before. This fulfills part of the security requirement that agents are sandboxed not just in process but in capability (no unexpected side effects).

Audit Trail Completed: Every policy intervention generates an audit log entry with sufficient detail. We can take the audit log (or filtered main log) and see, for example: at 10:00, CodeGenAgent attempted to open URL not allowed -> blocked; at 10:05, DebugAgent output was redacted for PII. This provides transparency and accountability. The log is also free of sensitive content itself (we didn't log the actual secret that was blocked, just a description) in line with not leaking via logs.

Minimal Impact on Normal Operation: For interactions that do not violate any policy, the policy engine should have negligible overhead and not interfere. Our testing confirms that typical agent outputs and tool uses (which are within spec) pass through unchanged and with only microsecond delays for checks. No false positive blocks occur in our normal test suite or example runs. In edge cases where something was borderline, we made a conscious decision whether to allow or block, and documented it. Essentially, the policy doesn't make the system annoying to use; it only steps in when truly needed.

Configurability & Clarity: We can adjust rules easily and see the effect. For example, if we want to allow something previously blocked, we remove it from policy and it works on next run. The process for this (edit file, restart service or send SIGHUP if we implemented reload, etc.) is documented. The default policy is reasonable and can be used as a starting point for users. It's also clear in documentation how to expand it (e.g., add a regex, change a threshold). The acceptance is that an admin could realistically customize the policy for their needs following our guide.

Integrated with Other Features: The policy engine works in concert with earlier features. For instance, if a content moderation stops an output, that output is not counted towards budget (since not actually delivered) – or if it is counted, it's minimal but anyway the run stops so budget is saved (likely not a big issue but just conceptual consistency). Also, our observability tools in Phase 4 might show that an agent was blocked by policy (maybe as an event in trace or visible in logs), so developers understand a lack of output was due to policy, not a bug. We ensure some trace or log indicator (like a span attribute "blocked=true" on that agent call perhaps).

Safety & Compliance Goals Met: With these guardrails, stakeholders concerned with safety (like an ethical AI review or compliance officer) should be satisfied that AgentMesh has mitigations against obvious misuse. The system won't easily spew out disallowed content without at least logging it and stopping. It won't perform unauthorized external actions. And it logs everything for accountability. This addresses the spec's requirement for prompt/tool guards, redaction, and audit trail explicitly, creating an enterprise-ready trust framework around the AI agents.

No Unexpected Rigidness: While strict, the system should not become unusable. The acceptance test here is more subjective: run a variety of normal tasks (maybe from the earlier demo scenarios) and confirm none of them trigger the policy engine erroneously. If we see false triggers, adjust policy defaults. Achieve a balance where the user doesn't frequently have to fight the policy (which could cause them to disable it – undermining its purpose). The final accepted default policy is cautious but not overly restrictive for typical coding/dev tasks (assuming that context).

Foundation for Phase 6: The policy engine provides some of the functionality needed for security/privacy. Phase 6 will build on this (e.g., RBAC and tenant isolation), but we ensure nothing in Phase 5 conflicts. In fact, Phase 5's audit logs and redaction are essential for Phase 6's compliance story. So acceptance includes that Phase 5 deliverables have been communicated to the security design such that they complement each other (for example, not double-redacting the same thing, etc.). Overall, the system's safety net is now multi-layered and robust.

Phase 6 — Security & Multi-Tenancy Hardening

Scope

Finalize the security framework, focusing on authentication, authorization, multi-tenant data isolation, and privacy controls to prepare AgentMesh for secure use in multi-user or sensitive environments. Key components:

Authentication: Require and verify credentials or tokens for any user or system interfacing with AgentMesh (especially if there's a persistent service or API). This ensures only authorized clients can start workflows or retrieve data.

Role-Based Access Control (RBAC): Implement role definitions (e.g., Admin, User) and enforce permissions. For instance, only Admins can change policy or view all audit logs, regular Users can run workflows and see their own data but not others', etc.

Multi-Tenancy & Data Isolation: Ensure that if AgentMesh is used by multiple projects or teams (tenants), their data (prompts, context, logs) remains isolated. This may involve namespacing the event log and other storages by tenant ID, and always tagging events with a tenant and filtering by tenant on retrieval.

Secure Communication: If AgentMesh has components communicating over a network (e.g., SDKs to the Rust orchestrator service, or any future web UI), enforce TLS/mTLS so that data in transit is protected. Also, consider signing or integrity checks for logs if needed for tamper evidence.

TLS/Encryption Setup: Finalize TLS configuration so that if AGENT_TLS_ENABLE=1, the Rust orchestrator service uses the provided server.crt and server.key to serve HTTPS/gRPC-TLS, and clients trust the CA (configurable for local dev). Test that traffic is indeed encrypted. For logs at rest, document file permissions and optional encryption.

PII & Privacy Controls: Augment the policy engine's redaction with broader privacy measures. Possibly encryption at rest for logs containing sensitive data (or at least a mode to do so), and a thorough PII scanning mode to identify any personal data that may have been stored, supporting compliance like GDPR (for internal use, maybe just ensure ability to locate and remove data if needed).

Security Auditing: Conduct a security review or audit simulation. Identify any remaining vulnerabilities (like injection possibilities, etc.) and address them. Ensure that audit logs from Phase 5 plus RBAC logs (like login attempts, permission denied events) are comprehensive for forensic needs.

Deliverables

Auth Mechanism & Config: A system for authentication, e.g., an API key or token that must be provided by SDKs. Deliverable could be a shared secret in config (for dev usage) or integration with OS user accounts (for local scenario). If a service runs, endpoints should check for a valid token in requests. Provide a utility to generate a token and instructions to set it (e.g., AGENTMESH_API_KEY env var).

RBAC Implementation: Definition of roles (e.g., define in config something like:

roles:
  admin: 
    permissions: ["view_all_logs", "edit_policy", "run_workflows"]
  user:
    permissions: ["run_workflows", "view_own_logs"]


and assignment of roles to users or API keys).
Implement enforcement in the code:

When a user attempts an action (start workflow, fetch logs, change config), check their role's permissions.

E.g., if a user tries to fetch logs for a workflow not owned by them and they're not admin, deny.

Possibly integrate with identity from auth (like token contains role or user id).

Tenant Isolation Support: Modify data storage usage such that each workflow or event is labeled by a tenant. For example, event log entries now include tenantId. Ensure the SDKs or API require specifying tenant (or infer from auth token). Implement separation: e.g., use separate log files or database tables per tenant if that makes queries easier. At runtime, filter any data access by tenant (so a user from Tenant A can never see data from Tenant B because queries are scoped).

Security Testing Results: A document or checklist of potential issues tested:

Ensured no injection: e.g., if an agent name is malicious or if someone tries to craft log queries with SQL injection (if DB used) – not likely if we use file, but check any eval usage etc.

Ensured that the policy engine + RBAC covers any scenario of privilege escalation (e.g., an agent prompt can't be used to escalate privileges since all critical actions are internal and require proper roles).

Checked that on multi-tenant usage, data doesn't leak: e.g., run workflows under two different tenant IDs and confirm that querying the logs or results for one doesn't show up under the other.

Confirmed that audit logs include relevant security events like authentication failures, permission denials, etc., possibly implemented now.

Security Documentation: An updated security section outlining how to configure and use these features. This might include how to manage API keys, how to set up a multi-tenant config (maybe mapping tokens to tenant IDs), how RBAC roles are assigned and what each role can do, and recommendations for OS-level security (like file permissions or running the process under a service account with limited access).

Compliance Note: (If relevant) A brief note mapping these security features to compliance needs (like how we enable meeting certain standards by having audit trails, PII controls, etc.), which might be used internally to communicate readiness to security teams.

Execution Steps

Add Authentication Layer: If the orchestrator runs as a persistent service (perhaps it does to allow TS SDK connection), implement an auth check in the request handler: e.g., expect an Authorization: Bearer <token> header and compare token to a stored secret. If orchestrator is only run on-demand via SDK in same process, auth might be less relevant (since it's the same user running the code). But plan for a scenario where multiple users might connect to a central orchestrator – implement accordingly, maybe with tokens identifying user/tenant.

Implement RBAC: Decide how to identify users in the system. Perhaps simplest: each API token has an associated user identity and role (store this in a small config map). If orchestrator is single-user local, that user can be considered admin by default. For multi-user, when a request comes in with a token, look up the token's user and role. Then enforce permissions:

Before executing admin actions (like changing policy file or retrieving audit logs) check if role != admin: deny.

When returning data like logs, filter out any entries that user shouldn't see (e.g., other tenants' workflow IDs).

Ensure orchestrator internal methods know which user/tenant initiated a workflow (so they tag events accordingly).

Propagate Tenant IDs: From the point of user request to orchestrator, carry a tenantId and userId. For example, include these in the context passed to orchestrator when starting a workflow. Modify event logging to include tenantId on each event (maybe derived from user or provided explicitly). Ensure that any lookup of workflows or results must specify tenant and only returns matches for that tenant.

Data Partitioning: If using flat files for logs, consider splitting by tenant (e.g., store logs in logs/<tenant>/workflow_<id>.log). If using an in-memory store or DB, include tenant in keys or queries. Test by simulating concurrent or sequential workflows from different tenants and ensuring logs are separate and query APIs (if any) separate them.

Finish TLS Integration: Use Python's ssl library to wrap any HTTP server socket with provided certs. If TS SDK uses a fetch to http://, change it to https:// and ensure it either uses the CA or disables verification for dev (maybe allow a config to skip verify if using self-signed in test). Test that the TS SDK can still connect when TLS is on (might require adding the CA to Node's trust or an option).

File/Resource Security: Check the file system usage: ensure log files are created with restrictive permissions (in Python, use open(..., mode, opener=os.open with flags=0o600) to set perms, or rely on umask). Document if any sensitive file (like config with keys) should be chmod 600 by user. If containerizing, ensure container doesn't run as root ideally.

PII Sweep: Using the policy engine's detection, perform a scan of logs or data for PII markers to ensure redaction is working (like run a sample where agent outputs an email, then scan log for an @ – expecting none because it was redacted). If needed, implement a command or script for administrators to find and purge data for a specific user (to satisfy something like GDPR right-to-be-forgotten, if relevant internally).

Security Audit Simulation: Have someone not on the core dev team review the system for security. They might try actions like: using an API token with limited role to fetch data they shouldn't, altering client code to bypass an SDK check, etc. See if any holes are found. Also think of any known vectors: are we protecting against directory traversal if log file names come from user input? (Probably not applicable, but check if any file paths incorporate user data).

Penetration Test Light: Try to misuse the system: e.g., craft extremely long inputs to see if any buffer issues or DoS (maybe sending a million tokens input – though that would hit budget anyway). See if any uncaught exceptions occur when rules are violated repeatedly or if a user floods the system with requests (if it's a service, maybe mention rate limiting as future work if needed).

Finalize Documentation & Checks: Update the official docs to reflect the new requirement: e.g., "You must provide an API key to use AgentMesh server" etc. Write usage examples for multi-tenant: maybe demonstrate launching two workflows under different tenants and how to retrieve results scoped by tenant. Summarize how all the pieces (cost, policy, RBAC, audit) work together to create a secure environment.

Acceptance

Authentication Enabled: If AgentMesh is run as a service, no unauthorized client can use it without the proper token/credentials. We verify that by trying to call an API endpoint or use the TS SDK with no/invalid token and observe it's rejected (HTTP 401 or exception). For local one-user usage, this might not be applicable; in that case, it's acceptable because the user has OS-level access anyway. The presence of an auth layer means we can safely expose AgentMesh's interface if needed internally, knowing only intended users get in.

RBAC & Permissions Enforced: Users with different roles experience appropriate access. For example, create two dummy tokens: one admin, one user. Admin can retrieve any workflow's logs (test by cross-tenant query perhaps) and user cannot (they get a permission error or empty result). Admin can change config or shut down service (if we allow that) while user cannot. The system correctly identifies the requester and checks permissions on every sensitive action. If any attempt is made to bypass (like forging a different userId in a request), it fails because the system relies on token mapping which can't be arbitrarily altered.

Tenant Data Isolation: In a multi-tenant scenario, data does not leak between tenants. This is crucial: we demonstrate by running distinct workflows under two tenant IDs with similar content and showing that none of tenant A's data is visible when using tenant B's credentials or context. Also, if we intentionally try to mislabel a request (like manually calling an internal function with a wrong tenant ID), the system should either not allow it or it's an impossible scenario via exposed interfaces. Essentially, "walls" between tenants are solid.

Secure Communication & Storage: We confirm that communications can be secured via TLS – e.g., our integration test of TS SDK to Python orchestrator works over https with the provided self-signed cert (and we documented how to trust it). Also, we check that logs and config files created by AgentMesh are not world-readable on the file system (by default, they should be user-only given typical umask, but we ensure it in code/docs if needed). So if an unauthorized user on the same machine tries to read another user's AgentMesh logs, they cannot (assuming OS permissions).

Privacy Measures: All known channels for sensitive data are protected: the policy engine redacts logs for PII (tested in Phase 5), and any persistent storage of potentially sensitive info (like if we stored full prompts or vector embeddings somewhere) is either protected or documented so that operators can clear it. There's a clear procedure to remove user-specific data if required (e.g., an admin can delete all logs for a given user or tenant easily because they are isolated).

Comprehensive Audit & Logging: Now audit logs likely include authentication attempts (successful or failed) and major admin actions, in addition to policy events from Phase 5. Acceptance is that an admin can audit who did what: e.g., see that user X ran a workflow at time Y, it used Z tokens (from budget log), output was clean (no policy violation) or had a violation which was blocked (from policy audit), etc. If any security event occurred (invalid token use, forbidden data access attempt), it's logged. Thus, we have an auditable system state for security reviews.

Resilience to Attacks: While AgentMesh is primarily local, we consider basic resilience: e.g., the system doesn't crash or become unstable when facing intentional malformed inputs (policy already handles weird content by blocking, and budget prevents infinite loops consuming memory/cost). Perhaps an attacker could spam with many requests – our single-track design inherently queues them or processes one by one, so it's not easily overwhelmed (still, one could DoS by heavy usage, but that's an expected scenario mitigated by cost limits and possibly by OS if memory exhaust – acceptable for now).

No Regression of Functionality: Despite locking things down, normal usage by an authorized user remains smooth. We test regular flows with security on (auth token present, proper roles) and ensure everything still functions as in previous phases. If something requires admin now (like viewing all logs), we accept that as a design change but we updated docs to reflect it. The key is that new security checks are not blocking legitimate operations for properly authorized users.

Final Security Sign-off: Internal security evaluation (maybe by a security engineer or the team collectively) signs off that AgentMesh is safe to deploy internally. This means we addressed all major threat vectors identified at the start of the project: data leaks (solved by isolation and redaction), unauthorized use (solved by auth/RBAC), uncontrolled actions (solved by policy and sandboxing), and compliance logging (solved by audit). With this, the product meets the security/privacy requirements of the spec and is considered enterprise-ready on the trust dimension.

Documentation Clarity: The security and multi-tenancy features are well-documented such that someone setting up AgentMesh in an environment with multiple users or sensitive data can follow the guide to configure it correctly (set up keys, roles, etc.) and understand how to operate it securely (like rotating API keys, checking audit logs, etc.). The acceptance is that an informed user could use AgentMesh in a team setting without needing to dive into code to understand security implications.

Phase 7 — Integration & Release

Scope

The final phase focuses on polishing all aspects of AgentMesh for a production-grade release and verifying integration with external frameworks and tools. Key objectives:

Cross-Platform Support: Ensure AgentMesh runs smoothly on all target platforms. Specifically, extend support to Windows (if not fully covered yet) by fixing any OS-specific issues (path handling, process management, etc.). Re-run tests on Windows and address failures.

Performance Optimization: Profile the system under realistic loads and optimize any bottlenecks. Consider improving concurrency (within determinism limits) if possible, optimizing I/O (for log writing, etc.), and tuning configurations (like default sampling or batch writes) to meet performance targets (overhead, latency).

External Framework Integration: Validate that AgentMesh can integrate with external agent frameworks or standards via the internal agentic SWE agent or adapters. For example, test that an AgentMesh Orchestrator can coordinate with a Google A2A-compliant agent or that one of the agents can be a wrapper around a LangChain agent. This may involve building a small adapter or ensuring our message format is compatible (which we planned in prior phases).

Release Artifacts: Prepare final release artifacts for distribution. This includes packaging the Python SDK to PyPI, the TS SDK to npm, creating versioned releases, and perhaps container images. Also finalize version numbering (v1.0.0 for production release) and changelogs.

Documentation & Traceability: Compile and finalize documentation, including an end-to-end user guide, API references, and an explicit mapping of spec requirements to implemented features (to show nothing was missed). Put in place contribution guidelines and maintenance plans.

Deployment & Governance: If relevant, set up CI pipelines for future contributions, issue tracking, and any necessary governance processes (like code owners, security response process, etc.). Essentially, ensure the project can be maintained and scaled after initial release.

Deliverables

Windows Compatibility Fixes: Code adjustments or documentation that solve Windows-specific issues. For example, if our sandboxing used fork() (not available on Windows), replace with subprocess approach; handle Windows path separators in file paths; ensure that any shell commands or tool calls are either cross-platform or disabled on Windows with a note. Run the full test suite on Windows and attach results or a summary showing near-100% pass (some tests might be skipped if feature not supported on Win, but then mark those).

Performance Benchmark Results: A document or section in README listing performance metrics of AgentMesh in various scenarios (e.g., overhead on a single agent call ~50ms, ability to handle X agent calls per minute, memory footprint stable for Y workflows, etc.). If any optimizations were implemented (like asynchronous logging or caching), note their effect. Ideally show that we met or exceeded any performance goals from spec (the spec success criteria mentioned coordination overhead < 10%, etc., so confirm where we stand relative to that).

Integration Adapters/Demo: Provide either code or configuration for integrating with at least one external framework. For instance, an adapter class that can wrap a LangChain agent as an AgentMesh agent, or instructions for how to deploy an AgentMesh orchestrator that communicates with an A2A agent (since A2A is an open protocol)
solo.io
. Possibly include a small demo: e.g., AgentMesh orchestrates a conversation between two agents implemented in another framework via A2A messages – if realistic. At minimum, ensure nothing in our architecture prevents such integration (and document how it can be done).

Release Packaging:

PyPI package: the setup.py/pyproject.toml configured, tested by actually uploading to a test PyPI or building a wheel distribution.

npm package: package.json finalized, run npm pack to ensure it bundles correct files (type definitions, etc.).

Dockerfile (if we provide one) to run AgentMesh orchestrator in a container (with perhaps an embedded model or all dependencies).

Version tagging in git and changelog file enumerating changes from pre-release to now.

Comprehensive Documentation: All documentation files (or website if using one) updated. This includes:

User Guide: How to install and use AgentMesh end-to-end (including setup, running a basic workflow, using policy, etc.).

API Reference: For both Python and TS SDK (could be generated from docstrings/comments using Sphinx or TypeDoc).

Administration Guide: How to configure budgets, policies, roles, deploy in multi-tenant mode, enable TLS – basically summarizing Phases 3-6 features for admins.

Traceability Matrix: Possibly include in docs an explicit mapping from original spec bullet points to sections of docs or features, to illustrate completeness.

FAQ/Troubleshooting: If we identified common pitfalls or errors (like forgetting to set API key, or hitting OS limits on open files if many workflows run), mention solutions.

Maintenance and Handoff Materials: If this project will be maintained by others, provide things like:

CI configured to run tests on all OS and maybe to publish packages on tag.

A contribution guideline (coding style, how to run tests, how to add a new agent).

Identify any future work not done (maybe list ideas like more parallelism or richer moderation as backlog items).

Ensure all known issues are either resolved or documented (no surprise "TODO" in code without an issue filed).

Final Approval Checklist: A filled checklist (like the one in Phase 0 acceptance criteria but final) confirming all sections and features are present, spec references included, no placeholder text remains, etc. Essentially, an internal QA sign-off that the roadmap deliverables are completed.

Execution Steps

Cross-Platform Testing: Run automated tests on a Windows environment (and Mac if not done). Use continuous integration to do this systematically. Address failures: e.g., if path issues, use os.path.join properly; if certain libraries aren't available on Windows, include alternatives or conditionals. Particularly check things like the TLS path handling (file paths might need tweaks) and any subprocess calls. Ensure CLI tools work similarly on Windows (if we have any command-line scripts, maybe need .bat wrappers).

Optimize Performance: Profile using a tool (cProfile for Python, or measure time between log events for orchestrator overhead). Identify slow points:

Possibly logging could be synchronous and slow if writing to disk for each event. Consider batching or using an async logging thread. Implement if needed and safe (with flush on crash perhaps).

If orchestrator waits too synchronously for agents even when some could be parallel (e.g., if two unrelated tasks exist), consider introducing concurrency with threads or asyncio, ensuring order is deterministic by design (maybe schedule concurrently but join results in fixed order).

If serialization (JSON) is heavy, consider using ujson or orjson for speed, as long as determinism of formatting doesn't matter beyond logs.

Ensure no memory leaks: perhaps stress test with 100 sequential workflows, see memory usage stable.

Apply any low-hanging optimizations and measure improvements to ensure overhead is low. Document these in performance report.

Integration Validation with SWE Agent: Work with the team/person who built the agentic SWE agent that is supposed to integrate external frameworks. Possibly they have a blueprint for how to call AgentMesh or vice versa. Do a dry run: e.g., if the SWE agent can feed tasks to AgentMesh in A2A format, try mapping our AgentTask to A2A JSON (we had lines in blueprint about mapping to A2A). Or test that we can embed a LangChain tool use inside one of our agents by calling LangChain's API from within an AgentMesh agent – verifying nothing prevents that (like possibly increase some context limits or allow LangChain to run externally). If integration not fully built, at least create a stub demonstrating concept and no blockers (maybe a short write-up for future devs on how to complete the integration).

Finalize Packaging and Versioning: Bump version numbers to 1.0.0. Build distribution files: python setup.py sdist bdist_wheel, npm run build etc. Test installing those packages in a fresh environment and running a basic usage to ensure no missing files. Prepare release notes listing key features and improvements since any earlier version (if applicable).

CI/CD: Configure GitHub Actions or another CI to on release:

run tests on all OS (matrix of ubuntu, windows, mac),

perhaps automatically publish the package to PyPI/npm when a tag is pushed (if appropriate for internal distribution, maybe not needed, but at least have steps).

If open-sourced, ensure repository is tidy (LICENSE file present, no internal sensitive info in history, etc.).

Documentation Proofreading: Go through each doc section, ensure it's up-to-date with final feature set (for instance, we added RBAC, make sure user guide mentions needing to set up a token and how). Possibly have a colleague follow the quickstart guide from scratch to see if anything is unclear.

Traceability Audit: Use the traceability matrix (Section 4) to manually verify each spec item has been addressed in code/docs. For any that were partially addressed or tricky, note how or plan for future. By now it should all be done, so ideally tick them all off.

Final Demo: Put together an end-to-end demonstration scenario that showcases AgentMesh's capabilities: e.g., have a sample "project" where multiple agents collaborate to solve a task (maybe a coding task). Run it live (or in a recorded script) showing:

Deterministic execution (maybe run twice to show same results),

Observability (open Jaeger UI or show the structured logs),

Budget enforcement (maybe set a low budget to show it stops appropriately),

Policy guardrails (maybe slip a disallowed content to show it blocked),

Multi-tenant (run two parallel tasks under different tenants, show isolation),

External integration (maybe one agent's functionality is actually provided by an external API call integrated).
This might be ambitious to show all in one flow, but even a narrative could suffice. The idea is to prove the system works as intended for stakeholders (principal engineers, etc.). Collect any final feedback.

Address Final Feedback: If any minor adjustments or clarifications needed from the final demo or reviews (maybe someone suggests an extra default policy rule, or found a small bug in a corner case), address them quickly as part of release polish.

Acceptance

Cross-Platform Availability: AgentMesh passes its test suite on macOS, Linux, and Windows. We have resolved any platform-specific issues or clearly documented if a minor feature is not available on a certain OS (with plans to fix in future if needed). For example, if on Windows we can't fully sandbox a subprocess with memory limits like ulimit, we note that limitation but maybe implement a partial workaround (like just timeouts). Importantly, core functionality (running workflows, logging, etc.) works on Windows. This meets the initial target of macOS/Linux and shows progress toward Windows support (even if final Windows support is experimental, we plan it in a later minor release).

Performance Goals Achieved: Based on our benchmark results, AgentMesh meets the performance SLOs outlined (if spec gave specific numbers, check them). For example, if coordination overhead target was <10%, our testing shows maybe ~5% overhead in typical scenario. If throughput or concurrency was a concern, we demonstrate we can handle a reasonable load (like running 10 workflows sequentially with minimal slowdown, or a few concurrently if allowed). The system is efficient in log writing and doesn't exhibit undue latency. Stakeholders find performance satisfactory for the intended use (developer-local – which usually tolerates some overhead – and possibly small team server).

Integration-Ready: We have either integrated or at least proven that integration with external agent systems is feasible. For instance, we confirm that our message format can wrap around an A2A message and our orchestrator can serve as an A2A "router" if needed. Or we show that our CodeGenAgent could call out to an external tool library easily. This addresses any question that "are we stuck in our ecosystem or can we talk to others?" – answer is yes, we can interoperate. Possibly even demonstrate a simple LangChain or Microsoft Semantic Kernel interop scenario as a validation. The internal agentic SWE agent team acknowledges that AgentMesh provides the hooks they need to integrate external frameworks.

Release Artifacts Complete: The 1.0 packages are built and tested. We performed a dry-run of publishing them (maybe to a test index or local npm) and then installing, to ensure nothing is missing. The CLI (if any provided, e.g., an agentmesh command for replay maybe) is installed and works. All dependency licenses are accounted for (if open source distribution is planned). Essentially, if we gave this to another team or open-sourced it, it's in a deliverable state. This is typically confirmed by having a fresh dev (not using dev environment with repo) do an pip install agentmesh, npm install agentmesh-sdk and run through example – it works as documented.

Full Documentation & Training Materials: The documentation is praised for its clarity and completeness. Any internal beta users or new joiners can rely on it to understand and use AgentMesh without constantly asking the original developers. The traceability matrix shows that all original requirements from the spec (foundational architecture, SDKs, budget, observability, policy, security) are implemented and documented. There are no loose ends or "TBD" in docs. If the project is handed over to another team or open source maintainers, they have everything needed (design rationale in architecture doc, usage docs, etc.).

Stakeholder Sign-off: The principal engineer or project sponsor reviews the entire roadmap implementation and agrees it meets the scope and rigor defined. The quality-first approach is evident: no major bugs, high test coverage, and all acceptance criteria in each phase were met. Security team signs off that it's compliant with internal guidelines. Observability team is happy that it uses standard OTel and logs. Essentially, the product is considered production-ready to deploy for internal use (and perhaps to eventually integrate into developer workflows product).

Post-Release Plan: We have addressed how AgentMesh will be maintained. For instance, assigned code ownership to a team or arranged for the repository to be monitored. CI will catch future issues (with tests on all platforms). A backlog of nice-to-have features (if any remained, e.g., deeper concurrency, richer UI) is documented for subsequent versions, but none of those are blockers for the 1.0 release.

Spec Goals Realized: The overarching goal was "a developer-local and self-hosted runtime for deterministic, observable, cost-governed multi-agent execution". We confirm, at release:

Deterministic execution is achieved via event sourcing and seeds (replay tests confirm this).

The system is highly observable (traces, logs, metrics integrated).

Costs are governed (budgets enforceable, metrics visible).

Multi-agent workflows are orchestrated as intended, with integration possible to external ecosystems.

It runs self-hosted (no cloud requirement) and on developer machines (tested on common OS).

Security and guardrails make it enterprise-grade (suitable for internal use by a dev team with sensitive code).

With all criteria met, AgentMesh can be confidently released as version 1.0.0 for internal use and further iteration.