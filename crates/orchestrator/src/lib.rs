//! Deterministic orchestrator gRPC service skeleton.

#![deny(unsafe_code)]

use budget::{BudgetConfig, BudgetState, Manager as BudgetManager};
use dashmap::{DashMap, DashSet};
use event_log::{EventLogError, EventRecord, JsonlEventLog};
use orca_core::envelope::Envelope;
use policy::{DecisionKind, Engine as PolicyEngine};
use serde_json::{json, Value as JsonValue};
#[cfg(feature = "otel")]
use telemetry::metrics::init_budget_instruments;
use telemetry::BudgetMetrics;
use tokio::time::{sleep, timeout, Duration};
use tonic::{Request, Response, Status};
use tracing::{info, info_span, instrument, warn, Instrument};

pub mod orca_v1 {
    tonic::include_proto!("orca.v1");
}

use orca_v1::{
    orchestrator_server::{Orchestrator, OrchestratorServer},
    *,
};

/// Minimal in-memory run index rebuilt from WAL on start.
#[derive(Default, Clone)]
pub struct RunIndex {
    pub last_event_id_by_run: std::sync::Arc<DashMap<String, u64>>,
    pub usage_by_run: std::sync::Arc<DashMap<String, (u64, u64)>>,
    pub usage_by_run_agent: std::sync::Arc<DashMap<(String, String), (u64, u64)>>,
    pub run_start_ts_by_run: std::sync::Arc<DashMap<String, u64>>,
}

/// Service state.
#[derive(Clone)]
pub struct OrchestratorService {
    log: JsonlEventLog,
    seen_ids: std::sync::Arc<DashSet<String>>, // idempotency: seen message ids
    pub index: RunIndex,
    policy: PolicyEngine,
    budget: BudgetManager,
    budgets_by_run: std::sync::Arc<DashMap<String, BudgetManager>>, // per-run budgets
    metrics: BudgetMetrics,
}

#[allow(clippy::result_large_err)]
impl OrchestratorService {
    pub fn new(log: JsonlEventLog) -> Self {
        Self {
            log,
            seen_ids: std::sync::Arc::new(DashSet::new()),
            index: RunIndex {
                last_event_id_by_run: std::sync::Arc::new(DashMap::new()),
                usage_by_run: std::sync::Arc::new(DashMap::new()),
                usage_by_run_agent: std::sync::Arc::new(DashMap::new()),
                run_start_ts_by_run: std::sync::Arc::new(DashMap::new()),
            },
            policy: PolicyEngine::new(),
            budget: BudgetManager::new(BudgetConfig::default()),
            budgets_by_run: std::sync::Arc::new(DashMap::new()),
            metrics: BudgetMetrics::new(),
        }
    }
    pub fn with_budget(mut self, cfg: BudgetConfig) -> Self {
        self.budget = BudgetManager::new(cfg);
        self
    }
    pub fn into_server(self) -> OrchestratorServer<Self> {
        OrchestratorServer::new(self)
    }

    pub fn replay_on_start(&self) -> Result<(), Status> {
        let recs: Vec<EventRecord<JsonValue>> =
            self.log.read_range(0, u64::MAX).map_err(internal_io)?;
        for rec in recs {
            let p = rec.payload;
            if let Some(run) =
                p.get("run_id").and_then(|v| v.as_str()).map(|s| s.to_string()).or_else(|| {
                    p.get("workflow_id").and_then(|v| v.as_str()).map(|s| s.to_string())
                })
            {
                self.index.last_event_id_by_run.insert(run.clone(), rec.id);
                if p.get("event").and_then(|v| v.as_str()) == Some("start_run") {
                    self.index.run_start_ts_by_run.insert(run, rec.ts_ms);
                }
            }
            if let Some(env) = p.get("envelope").and_then(|v| v.get("id")).and_then(|v| v.as_str())
            {
                self.seen_ids.insert(env.to_string());
            }
        }
        Ok(())
    }

    async fn retry<F, Fut, T>(&self, mut f: F, attempts: u32, delay_ms: u64) -> Result<T, Status>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, Status>>,
    {
        let mut rem = attempts;
        loop {
            match f().await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    rem -= 1;
                    if rem == 0 {
                        return Err(e);
                    }
                    sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }

    fn reject_if_expired_or_version(&self, env: &orca_v1::Envelope) -> Result<(), Status> {
        if env.timeout_ms > 0 {
            let now = orca_core::ids::now_ms();
            if now.saturating_sub(env.ts_ms) > env.timeout_ms {
                return Err(Status::deadline_exceeded("ttl expired"));
            }
        }
        if env.protocol_version != 1 {
            return Err(Status::failed_precondition("unsupported protocol_version"));
        }
        Ok(())
    }

    fn check_auth(md: &tonic::metadata::MetadataMap) -> Result<(), Status> {
        if let Ok(Some(required)) =
            std::env::var("AGENT_AUTH_TOKEN").map(|s| if s.is_empty() { None } else { Some(s) })
        {
            match md.get("authorization").and_then(|v| v.to_str().ok()) {
                Some(got) if got == required => Ok(()),
                _ => Err(Status::unauthenticated("invalid authorization")),
            }
        } else {
            Ok(())
        }
    }
}

#[allow(clippy::result_large_err, clippy::single_match)]
#[tonic::async_trait]
impl Orchestrator for OrchestratorService {
    #[instrument(skip_all)]
    async fn start_run(
        &self,
        req: Request<StartRunRequest>,
    ) -> Result<Response<StartRunResponse>, Status> {
        Self::check_auth(req.metadata())?;
        let mut r = req.into_inner();
        if let Some(ref env) = r.initial_task {
            self.reject_if_expired_or_version(env)?;
        }
        // Pre-policy: allow/deny/modify (redaction)
        if let Some(ref env) = r.initial_task {
            let _span = info_span!("agent.policy.check", run=%r.workflow_id, phase="pre_start_run", agent=%env.agent).entered();
            let env_json = serde_json::to_value(env).map_err(internal_serde)?;
            match self.policy.pre_start_run(&env_json).kind {
                DecisionKind::Deny => return Err(Status::permission_denied("policy deny")),
                DecisionKind::Modify => {
                    // replace initial_task with redacted json->proto
                    r.initial_task =
                        Some(serde_json::from_value(env_json).map_err(internal_serde)?);
                }
                DecisionKind::Allow => {}
            }
        }
        // Optional per-run budget from request or environment defaults
        if let Some(b) = r.budget.as_ref() {
            let cfg = BudgetConfig {
                max_tokens: if b.max_tokens == 0 { None } else { Some(b.max_tokens) },
                max_cost_micros: if b.max_cost_micros == 0 {
                    None
                } else {
                    Some(b.max_cost_micros)
                },
            };
            self.budgets_by_run.insert(r.workflow_id.clone(), BudgetManager::new(cfg));
        } else {
            let max_tokens =
                std::env::var("ORCA_MAX_TOKENS").ok().and_then(|s| s.parse::<u64>().ok());
            let max_cost =
                std::env::var("ORCA_MAX_COST_MICROS").ok().and_then(|s| s.parse::<u64>().ok());
            if max_tokens.is_some() || max_cost.is_some() {
                self.budgets_by_run.insert(
                    r.workflow_id.clone(),
                    BudgetManager::new(BudgetConfig { max_tokens, max_cost_micros: max_cost }),
                );
            }
        }
        let wf = r.workflow_id.clone();
        self.retry(
            || async {
                let _span = info_span!("wal.append", event="start_run", workflow=%wf).entered();
                let now_ts = orca_core::ids::now_ms();
                self.index.run_start_ts_by_run.insert(wf.clone(), now_ts);
                self.log
                    .append(
                        orca_core::ids::next_monotonic_id(),
                        now_ts,
                        &json!({
                            "event":"start_run", "workflow_id": wf, "envelope": r.initial_task
                        }),
                    )
                    .map_err(internal_io)
            },
            3,
            50,
        )
        .await?;
        info!(workflow=%r.workflow_id, "StartRun accepted");
        Ok(Response::new(StartRunResponse { run_id: r.workflow_id }))
    }

    #[instrument(skip_all)]
    async fn submit_task(
        &self,
        req: Request<SubmitTaskRequest>,
    ) -> Result<Response<SubmitTaskResponse>, Status> {
        Self::check_auth(req.metadata())?;
        let mut r = req.into_inner();
        {
            let env =
                r.task.as_ref().ok_or_else(|| Status::invalid_argument("missing envelope"))?;
            self.reject_if_expired_or_version(env)?;
            if self.seen_ids.contains(&env.id) {
                return Ok(Response::new(SubmitTaskResponse { accepted: true }));
            }
        }

        // Pre-policy
        let env_json = {
            let env =
                r.task.as_ref().ok_or_else(|| Status::invalid_argument("missing envelope"))?;
            let _span = info_span!("agent.policy.check", run=%r.run_id, phase="pre_submit_task", agent=%env.agent).entered();
            serde_json::to_value(env).map_err(internal_serde)?
        };
        match self.policy.pre_submit_task(&env_json).kind {
            DecisionKind::Deny => return Err(Status::permission_denied("policy deny")),
            DecisionKind::Modify => {
                r.task = Some(serde_json::from_value(env_json).map_err(internal_serde)?);
            }
            DecisionKind::Allow => {}
        }

        // Budget usage/update and thresholds (per-run if configured)
        let env = r.task.as_ref().ok_or_else(|| Status::invalid_argument("missing envelope"))?;
        let mut tokens_inc: u64 = 1; // default minimal increment
        let mut cost_inc: u64 = 0;
        if let Some(h) = env.usage.as_ref() {
            if h.tokens > 0 {
                tokens_inc = h.tokens;
            }
            if h.cost_micros > 0 {
                cost_inc = h.cost_micros;
            }
        }
        if let Some(mgr) = self.budgets_by_run.get(&r.run_id) {
            mgr.add_usage(tokens_inc, cost_inc);
            self.metrics.add(tokens_inc, cost_inc);
            #[cfg(feature = "otel")]
            {
                let inst = init_budget_instruments();
                inst.tokens().add(tokens_inc, &[]);
                inst.cost_micros().add(cost_inc, &[]);
            }
            let status = mgr.status();
            let _span = info_span!("agent.budget.check", run=%r.run_id, tokens=%tokens_inc, cost_micros=%cost_inc, status=?status).entered();
            match status {
                BudgetState::Exceeded => {
                    let _ = self
                        .log
                        .append(
                            orca_core::ids::next_monotonic_id(),
                            orca_core::ids::now_ms(),
                            &json!({
                                "event":"budget_exceeded", "run_id": r.run_id
                            }),
                        )
                        .map_err(internal_io)?;
                    return Err(Status::resource_exhausted("budget exceeded"));
                }
                BudgetState::Warning90 => {
                    let _ = self
                        .log
                        .append(
                            orca_core::ids::next_monotonic_id(),
                            orca_core::ids::now_ms(),
                            &json!({
                                "event":"budget_warning", "run_id": r.run_id, "level":"90"
                            }),
                        )
                        .map_err(internal_io)?;
                    warn!(run=%r.run_id, "budget >=90%")
                }
                BudgetState::Warning80 => {
                    let _ = self
                        .log
                        .append(
                            orca_core::ids::next_monotonic_id(),
                            orca_core::ids::now_ms(),
                            &json!({
                                "event":"budget_warning", "run_id": r.run_id, "level":"80"
                            }),
                        )
                        .map_err(internal_io)?;
                    warn!(run=%r.run_id, "budget >=80%")
                }
                BudgetState::Within => {}
            }
        } else {
            self.budget.add_usage(tokens_inc, cost_inc);
            self.metrics.add(tokens_inc, cost_inc);
            #[cfg(feature = "otel")]
            {
                let inst = init_budget_instruments();
                inst.tokens().add(tokens_inc, &[]);
                inst.cost_micros().add(cost_inc, &[]);
            }
            let status = self.budget.status();
            let _span = info_span!("agent.budget.check", run=%r.run_id, tokens=%tokens_inc, cost_micros=%cost_inc, status=?status).entered();
            match status {
                BudgetState::Exceeded => {
                    let _ = self
                        .log
                        .append(
                            orca_core::ids::next_monotonic_id(),
                            orca_core::ids::now_ms(),
                            &json!({
                                "event":"budget_exceeded", "run_id": r.run_id
                            }),
                        )
                        .map_err(internal_io)?;
                    return Err(Status::resource_exhausted("budget exceeded"));
                }
                BudgetState::Warning90 => {
                    let _ = self
                        .log
                        .append(
                            orca_core::ids::next_monotonic_id(),
                            orca_core::ids::now_ms(),
                            &json!({
                                "event":"budget_warning", "run_id": r.run_id, "level":"90"
                            }),
                        )
                        .map_err(internal_io)?;
                    warn!(run=%r.run_id, "budget >=90%")
                }
                BudgetState::Warning80 => {
                    let _ = self
                        .log
                        .append(
                            orca_core::ids::next_monotonic_id(),
                            orca_core::ids::now_ms(),
                            &json!({
                                "event":"budget_warning", "run_id": r.run_id, "level":"80"
                            }),
                        )
                        .map_err(internal_io)?;
                    warn!(run=%r.run_id, "budget >=80%")
                }
                BudgetState::Within => {}
            }
        }

        // Update per-run usage totals and emit usage_update event
        {
            let mut entry = self.index.usage_by_run.entry(r.run_id.clone()).or_insert((0, 0));
            let (ref mut t, ref mut c) = *entry;
            *t = t.saturating_add(tokens_inc);
            *c = c.saturating_add(cost_inc);
            // Per-agent aggregation
            let agent_key = (r.run_id.clone(), env.agent.clone());
            let mut aentry = self.index.usage_by_run_agent.entry(agent_key).or_insert((0, 0));
            let (ref mut at, ref mut ac) = *aentry;
            *at = at.saturating_add(tokens_inc);
            *ac = ac.saturating_add(cost_inc);
            let _ = self
                .log
                .append(
                    orca_core::ids::next_monotonic_id(),
                    orca_core::ids::now_ms(),
                    &json!({
                        "event":"usage_update", "run_id": r.run_id, "tokens": *t, "cost_micros": *c,
                        "elapsed_ms": self.index.run_start_ts_by_run.get(&r.run_id).map(|v| orca_core::ids::now_ms().saturating_sub(*v.value())).unwrap_or(0)
                    }),
                )
                .map_err(internal_io)?;
        }

        let env = r.task.as_ref().unwrap();
        self.seen_ids.insert(env.id.clone());
        let env_json2 = serde_json::to_value(env).map_err(internal_serde)?;
        let run_id = r.run_id.clone();
        self.retry(
            || async {
                let _span = info_span!("wal.append", event="task_enqueued", run=%run_id).entered();
                self.log
                    .append(
                        orca_core::ids::next_monotonic_id(),
                        orca_core::ids::now_ms(),
                        &json!({
                            "event":"task_enqueued", "run_id": run_id, "envelope": env_json2
                        }),
                    )
                    .map_err(internal_io)
            },
            3,
            50,
        )
        .await?;

        if env.timeout_ms > 0 {
            let dur = Duration::from_millis(env.timeout_ms);
            let res = timeout(dur, async {}).await;
            let post = self.policy.post_submit_task(&json!({"result":"stub"}));
            match res {
                Ok(_) => info!("task completed"),
                Err(_) => warn!("task timeout"),
            }
            match post.kind {
                DecisionKind::Deny => return Err(Status::permission_denied("policy deny")),
                _ => {}
            }
        }

        // End-of-run summary heuristic: if this is an agent_result, emit summary
        if env.kind == "agent_result" {
            if let Some((t, c)) = self.index.usage_by_run.get(&r.run_id).map(|v| *v.value()) {
                // Build per-agent breakdown
                let mut breakdown: Vec<JsonValue> = Vec::new();
                for kv in self.index.usage_by_run_agent.iter() {
                    let ((run, agent), (at, ac)) = kv.pair();
                    if *run == r.run_id {
                        breakdown.push(json!({"agent": agent, "tokens": at, "cost_micros": ac }));
                    }
                }
                let _ = self.log.append(orca_core::ids::next_monotonic_id(), orca_core::ids::now_ms(), &json!({
                    "event":"run_summary", "run_id": r.run_id, "tokens": t, "cost_micros": c, "by_agent": breakdown,
                    "duration_ms": self.index.run_start_ts_by_run.get(&r.run_id).map(|v| orca_core::ids::now_ms().saturating_sub(*v.value())).unwrap_or(0)
                })).map_err(internal_io)?;
            }
        }
        Ok(Response::new(SubmitTaskResponse { accepted: true }))
    }

    type StreamEventsStream =
        tokio_stream::wrappers::ReceiverStream<Result<StreamEventsResponse, Status>>;
    #[instrument(skip_all)]
    async fn stream_events(
        &self,
        req: Request<StreamEventsRequest>,
    ) -> Result<Response<Self::StreamEventsStream>, Status> {
        Self::check_auth(req.metadata())?;
        let r = req.into_inner();
        let run_id = r.run_id.clone();
        let start_event_id = r.start_event_id;
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let log = self.log.clone();
        tokio::spawn(
            async move {
                let start_id = r.start_event_id;
                let recs: Result<Vec<EventRecord<JsonValue>>, _> =
                    log.read_range(start_id, u64::MAX);
                let mut sent = 0u32;
                match recs {
                    Ok(recs) => {
                        for rec in recs {
                            if r.since_ts_ms > 0 && rec.ts_ms < r.since_ts_ms {
                                continue;
                            }
                            if r.max_events > 0 && sent >= r.max_events {
                                break;
                            }
                            let p = rec.payload;
                            let run_match =
                                p.get("run_id").and_then(|v| v.as_str()) == Some(r.run_id.as_str());
                            let wf_match = p.get("workflow_id").and_then(|v| v.as_str())
                                == Some(r.run_id.as_str());
                            if !(run_match || wf_match) {
                                continue;
                            }
                            let kind = p
                                .get("event")
                                .and_then(|v| v.as_str())
                                .unwrap_or("event")
                                .to_string();
                            let env = orca_v1::Envelope {
                                id: String::new(),
                                parent_id: String::new(),
                                trace_id: String::new(),
                                agent: String::new(),
                                kind,
                                payload_json: p.to_string(),
                                timeout_ms: 0,
                                protocol_version: 1,
                                ts_ms: rec.ts_ms,
                                usage: None,
                            };
                            if tx.send(Ok(StreamEventsResponse { event: Some(env) })).await.is_err()
                            {
                                break;
                            }
                            sent += 1;
                        }
                    }
                    Err(e) => {
                        let _ = tx
                            .send(Err(Status::internal(format!("stream read failed: {}", e))))
                            .await;
                    }
                }
            }
            .instrument(info_span!("agent.core.stream", run=%run_id, start_id=%start_event_id)),
        );
        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }

    #[instrument(skip_all)]
    async fn fetch_result(
        &self,
        req: Request<FetchResultRequest>,
    ) -> Result<Response<FetchResultResponse>, Status> {
        Self::check_auth(req.metadata())?;
        let empty = Envelope::new_result("", "", "", json!({"status":"stub"}));
        Ok(Response::new(FetchResultResponse { result: Some(convert_envelope(empty)) }))
    }
}

fn internal_io(e: EventLogError) -> Status {
    Status::internal(format!("io error: {}", e))
}
fn internal_serde(e: serde_json::Error) -> Status {
    Status::internal(format!("serde error: {}", e))
}

fn convert_envelope(e: Envelope) -> orca_v1::Envelope {
    orca_v1::Envelope {
        id: e.id,
        parent_id: e.parent_id.unwrap_or_default(),
        trace_id: e.trace_id,
        agent: e.agent,
        kind: format!("{:?}", e.kind).to_lowercase(),
        payload_json: serde_json::to_string(&e.payload).unwrap_or_default(),
        timeout_ms: e.timeout_ms.unwrap_or_default(),
        protocol_version: e.protocol_version,
        ts_ms: e.ts_ms,
        usage: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ttl_rejection() {
        let dir = tempfile::tempdir().unwrap();
        let log = JsonlEventLog::open(dir.path().join("x.jsonl")).unwrap();
        let svc = OrchestratorService::new(log);
        let env = orca_v1::Envelope {
            id: "m1".into(),
            parent_id: "".into(),
            trace_id: "t".into(),
            agent: "A".into(),
            kind: "agent_task".into(),
            payload_json: "{}".into(),
            timeout_ms: 1,
            protocol_version: 1,
            ts_ms: orca_core::ids::now_ms().saturating_sub(10_000),
            usage: None,
        };
        let req = SubmitTaskRequest { run_id: "r".into(), task: Some(env) };
        let res = svc.submit_task(Request::new(req)).await;
        assert!(matches!(res, Err(Status { .. })))
    }

    #[tokio::test]
    async fn idempotency_skips_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let log = JsonlEventLog::open(dir.path().join("x.jsonl")).unwrap();
        let svc = OrchestratorService::new(log);
        let env = orca_v1::Envelope {
            id: "dup".into(),
            parent_id: "".into(),
            trace_id: "t".into(),
            agent: "A".into(),
            kind: "agent_task".into(),
            payload_json: "{}".into(),
            timeout_ms: 0,
            protocol_version: 1,
            ts_ms: orca_core::ids::now_ms(),
            usage: None,
        };
        let req1 = SubmitTaskRequest { run_id: "r".into(), task: Some(env.clone()) };
        let r1 = svc.submit_task(Request::new(req1)).await.unwrap();
        assert!(r1.into_inner().accepted);
        let req2 = SubmitTaskRequest { run_id: "r".into(), task: Some(env.clone()) };
        let r2 = svc.submit_task(Request::new(req2)).await.unwrap();
        assert!(r2.into_inner().accepted);
    }
}
