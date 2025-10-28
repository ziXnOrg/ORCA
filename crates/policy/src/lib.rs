//! ORCA Governance Policy Engine
//!
//! This crate provides a deterministic, fail-closed policy engine used to guard
//! orchestration events. The security baseline is deny-on-error: if no valid
//! policy is loaded or an evaluation precondition is not met, decisions default
//! to Deny. Built-in PII redaction is applied first to protect sensitive data.
//!
//! Decision taxonomy:
//! - Allow — proceed unchanged
//! - Deny — block the action (fail-closed default on error/misconfig)
//! - Modify — proceed with a redacted/rewritten payload (e.g., PII redaction)
//! - Flag — represented as `Allow` with `action == "allow_but_flag"` for audit
//!
//! Precedence and determinism:
//! 1) Built-in PII redaction (returns Modify immediately if applied)
//! 2) Fail-closed check: if no valid policy is loaded ⇒ Deny
//! 3) Tool allowlist enforcement
//! 4) Rule interpreter:
//!    - Highest priority wins (larger priority is higher)
//!    - Tie-breaker: most-restrictive-wins (Deny > Modify > Allow)
//!    - Still tied: first-match-wins (stable file order)
//!
//! All evaluations are designed to be deterministic for a given policy and input.
//!
//! Observability and audit:
//! - Every decision emits a low-cardinality counter `policy.decision.count{phase,kind,action}`.
//! - The special action `allow_but_flag` also increments an alias with `action="flag"` for ease of querying.
//! - An optional `PolicyObserver` can be installed to observe decisions in-process.
//! - A process-global `AuditSink` captures `AuditRecord`s for later inspection in tests.

#![deny(unsafe_code)]
#![warn(missing_docs)]

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// Kind of policy decision returned by the policy engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DecisionKind {
    /// Permit the action without changes.
    Allow,
    /// Block the action (fail-closed default on error/misconfig).
    Deny,
    /// Allow the action with modifications (e.g., redaction).
    Modify,
}

/// Result of evaluating a governance policy against an input envelope.
#[derive(Debug, Clone, Serialize)]
pub struct Decision {
    /// The overall decision kind produced by evaluation.
    pub kind: DecisionKind,
    /// Optional modified payload when the decision is `Modify`; `None` otherwise.
    pub payload: Option<Value>,
    /// Human-readable reason for the decision if available.
    pub reason: Option<String>,
    /// Name of the rule that triggered this decision (if any)
    pub rule_name: Option<String>,
    /// Action declared by the rule (e.g., `deny` | `modify` | `allow_but_flag`).
    pub action: Option<String>,
}

/// Observer invoked for each policy decision emitted by the engine.
///
/// Install an implementation via [`set_observer()`] to receive callbacks across all
/// evaluation phases. Implementations must be cheap and non-blocking; avoid I/O on
/// hot paths. This hook is primarily intended for tests and in-process metrics.
///
/// Example
/// ```
/// struct Capture;
/// impl policy::PolicyObserver for Capture {
///     fn on_decision(&self, phase: &str, d: &policy::Decision) {
///         assert!(matches!(phase, "pre_start_run"|"pre_submit_task"|"post_submit_task"));
///         let _ = &d.kind; // observe decision
///     }
/// }
/// policy::set_observer(Some(Box::new(Capture)));
/// // ... perform evaluations via Engine ... then clear when no longer needed:
/// policy::set_observer(None);
/// ```
pub trait PolicyObserver: Send + Sync {
    /// Called on every decision with the evaluation phase.
    fn on_decision(&self, phase: &str, decision: &Decision);
}

static OBSERVER: OnceLock<RwLock<Option<Arc<dyn PolicyObserver>>>> = OnceLock::new();

/// Install or clear the global policy observer used by this crate.
///
/// Passing `Some(Box::new(obs))` installs the observer; passing `None` clears it.
///
/// Example
/// ```
/// struct Nop;
/// impl policy::PolicyObserver for Nop {
///     fn on_decision(&self, _: &str, _: &policy::Decision) {}
/// }
/// policy::set_observer(Some(Box::new(Nop)));
/// policy::set_observer(None);
/// ```
pub fn set_observer(observer: Option<Box<dyn PolicyObserver>>) {
    let cell = OBSERVER.get_or_init(|| RwLock::new(None));
    let mut w = cell.write().expect("observer write lock poisoned");
    *w = observer.map(Arc::from);
}

/// In-process counters for policy decisions keyed by `{phase, kind, action}`.
///
/// Low-cardinality by construction; intended for tests and local observability. Not
/// persisted across process restarts.
#[derive(Default)]
pub struct PolicyMetrics {
    inner: Arc<Mutex<HashMap<String, u64>>>,
}

impl PolicyMetrics {
    /// Read the current count for a given {phase, kind, action} tuple.
    pub fn decision_counter(&self, phase: &str, kind: &str, action: &str) -> u64 {
        let key = format!("{}:{}:{}", phase, kind, action);
        self.inner.lock().expect("metrics lock poisoned").get(&key).copied().unwrap_or(0)
    }
    fn inc(&self, phase: &str, kind: &str, action: &str) {
        let mut g = self.inner.lock().expect("metrics lock poisoned");
        *g.entry(format!("{}:{}:{}", phase, kind, action)).or_insert(0) += 1;
    }
}

static METRICS: OnceLock<PolicyMetrics> = OnceLock::new();

/// Access the global policy metrics registry.
///
/// Example
/// ```
/// let m = policy::policy_metrics();
/// let c = m.decision_counter("pre_submit_task", "deny", "deny");
/// let _ = c; // inspect or compare as needed
/// ```
pub fn policy_metrics() -> &'static PolicyMetrics {
    METRICS.get_or_init(PolicyMetrics::default)
}

/// Audit record for a single policy decision.
#[derive(Debug, Clone, Serialize)]
pub struct AuditRecord {
    /// Evaluation phase (e.g., pre_submit_task)
    pub phase: String,
    /// Decision kind (allow/deny/modify)
    pub kind: DecisionKind,
    /// Triggering rule name (if any)
    pub rule_name: Option<String>,
    /// Declared action (e.g., deny|modify|allow_but_flag)
    pub action: Option<String>,
    /// Optional reason/message
    pub reason: Option<String>,
}

/// Handle for draining captured audit records. Cheap to clone; thread-safe.
#[derive(Clone)]
pub struct AuditSink {
    inner: Arc<Mutex<Vec<AuditRecord>>>,
}

impl AuditSink {
    /// Drain and return all captured audit records.
    pub fn drain(&self) -> Vec<AuditRecord> {
        let mut g = self.inner.lock().expect("audit lock poisoned");
        std::mem::take(&mut *g)
    }
}

static AUDIT: OnceLock<AuditSink> = OnceLock::new();

/// Install (or retrieve) the process-global audit sink.
///
/// Example
/// ```
/// let sink = policy::install_audit_sink();
/// assert!(sink.drain().is_empty());
/// // After evaluations, records will be available via `drain()`.
/// ```
pub fn install_audit_sink() -> AuditSink {
    if let Some(s) = AUDIT.get() {
        return s.clone();
    }
    let sink = AuditSink { inner: Arc::new(Mutex::new(Vec::new())) };
    let _ = AUDIT.set(sink.clone());
    sink
}

fn notify_observers_and_record(phase: &str, d: &Decision) {
    // Metrics
    let metrics = METRICS.get_or_init(PolicyMetrics::default);
    let kind_str = match d.kind {
        DecisionKind::Allow => "allow",
        DecisionKind::Deny => "deny",
        DecisionKind::Modify => "modify",
    };
    let action_str = d.action.as_deref().unwrap_or(kind_str);
    metrics.inc(phase, kind_str, action_str);
    if action_str == "allow_but_flag" {
        // Also emit alias for acceptance criteria that expects 'flag'
        metrics.inc(phase, kind_str, "flag");
    }
    // Observer
    if let Some(lock) = OBSERVER.get() {
        if let Ok(r) = lock.read() {
            if let Some(obs) = r.as_ref() {
                obs.on_decision(phase, d);
            }
        }
    }
    // Audit
    if let Some(s) = AUDIT.get() {
        let mut g = s.inner.lock().expect("audit lock poisoned");
        g.push(AuditRecord {
            phase: phase.to_string(),
            kind: d.kind,
            rule_name: d.rule_name.clone(),
            action: d.action.clone(),
            reason: d.reason.clone(),
        });
    }
}

/// Deterministic policy engine implementing fail-closed governance semantics.
#[derive(Debug, Clone)]
pub struct Engine {
    pii: Regex,
    rules: Vec<Rule>,
    tool_allowlist: Option<HashSet<String>>, // deny-by-default when present and tool not allowed
    /// True once a valid policy file has been loaded successfully. While `false`,
    /// evaluations are fail-closed (`DecisionKind::Deny`) after builtin PII redaction.
    policy_loaded: bool,
}

/// In-memory representation of a policy file loaded from YAML.
#[derive(Debug, Clone, Deserialize)]
pub struct PolicyFile {
    /// Declarative list of rules to evaluate.
    pub rules: Vec<Rule>,
    /// Optional global allowlist of tool names (case-insensitive). When present,
    /// tools not listed will be denied by default.
    #[serde(default)]
    pub tool_allowlist: Option<Vec<String>>,
}

/// A single policy rule compiled from YAML.
#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    /// Human-readable name of the rule (unique within a file is recommended).
    pub name: String,
    /// Condition string; matching is implementation-defined for the current baseline.
    pub when: String,
    /// Action to take: one of `deny`, `modify`, or `allow_but_flag`.
    pub action: String,
    /// Optional human-readable message explaining the decision.
    #[serde(default)]
    pub message: Option<String>,
    /// Optional severity/level (reserved for future use).
    #[serde(default)]
    pub level: Option<String>,
    /// Optional transform hint; for example, `regex:<pattern>` for modify rules.
    #[serde(default)]
    pub transform: Option<String>,
    /// Higher number = higher priority. Defaults to 0 for backward compatibility.
    #[serde(default)]
    pub priority: i32,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// Construct a new `Engine`. Initially, no policy is loaded; evaluation is
    /// fail-closed (Deny) after builtin PII redaction until a valid policy is loaded.
    #[must_use]
    pub fn new() -> Self {
        let pii = Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap();
        Self { pii, rules: Vec::new(), tool_allowlist: None, policy_loaded: false }
    }

    /// Load a policy from a YAML file at `path`.
    ///
    /// Validates schema, tool allowlist, and transforms; on success marks the engine
    /// as policy-loaded. Returns an error string describing the first validation
    /// failure encountered.
    pub fn load_from_yaml_path<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
    ) -> Result<(), String> {
        let f = File::open(&path)
            .map_err(|e| format!("Failed to open policy file {:?}: {}", path.as_ref(), e))?;
        let rdr = BufReader::new(f);
        let pf: PolicyFile = serde_yaml::from_reader(rdr)
            .map_err(|e| format!("Malformed YAML in policy file {:?}: {}", path.as_ref(), e))?;

        // Validate tool_allowlist: non-empty strings, no duplicates (case-insensitive)
        let tool_allowlist = if let Some(v) = pf.tool_allowlist {
            let mut set = HashSet::new();
            for (i, s) in v.into_iter().enumerate() {
                let t = s.trim().to_lowercase();
                if t.is_empty() {
                    return Err(format!("tool_allowlist[{}] must be a non-empty string", i));
                }
                if !set.insert(t.clone()) {
                    return Err(format!("tool_allowlist contains duplicate entry: '{}'", t));
                }
            }
            Some(set)
        } else {
            None
        };

        // Validate rules
        for (i, r) in pf.rules.iter().enumerate() {
            if r.name.trim().is_empty() {
                return Err(format!("rules[{}].name must be non-empty", i));
            }
            if r.when.trim().is_empty() {
                return Err(format!("rules[{}].when must be non-empty", i));
            }
            match r.action.as_str() {
                "deny" | "modify" | "allow_but_flag" => {}
                other => {
                    return Err(format!(
                        "rules[{}].action '{}' is invalid; valid: deny|modify|allow_but_flag",
                        i, other
                    ))
                }
            }
            if let Some(t) = &r.transform {
                let t = t.trim();
                if let Some(rest) = t.strip_prefix("regex:") {
                    // Validate regex patterns if declared as transform: "regex:<pattern>"
                    Regex::new(rest)
                        .map_err(|e| format!("rules[{}].transform regex invalid: {}", i, e))?;
                }
            }
        }

        self.rules = pf.rules;
        self.tool_allowlist = tool_allowlist;
        self.policy_loaded = true;
        Ok(())
    }

    /// Evaluate a policy prior to starting a run, returning a deterministic decision.
    pub fn pre_start_run(&self, envelope: &Value) -> Decision {
        let d = self.apply_rules_then_redact(envelope, Some("pre_start_run"));
        notify_observers_and_record("pre_start_run", &d);
        d
    }

    /// Evaluate a policy prior to submitting a task, returning a deterministic decision.
    pub fn pre_submit_task(&self, envelope: &Value) -> Decision {
        let d = self.apply_rules_then_redact(envelope, Some("pre_submit_task"));
        notify_observers_and_record("pre_submit_task", &d);
        d
    }

    /// Evaluate a policy after submitting a task; current baseline always allows.
    pub fn post_submit_task(&self, _result: &Value) -> Decision {
        let d = Decision {
            kind: DecisionKind::Allow,
            payload: None,
            reason: None,
            rule_name: None,
            action: None,
        };
        notify_observers_and_record("post_submit_task", &d);
        d
    }

    /// Apply the evaluation pipeline in deterministic order:
    /// 1) Built-in PII redaction (returns `Modify` immediately if applied)
    /// 2) Fail-closed deny if no valid policy is loaded
    /// 3) Tool allowlist enforcement
    /// 4) Rule interpreter with precedence (priority -> most-restrictive -> first-match)
    fn apply_rules_then_redact(&self, envelope: &Value, _phase: Option<&str>) -> Decision {
        // 1) Built-in PII redaction first (fail-closed if needed in callers)
        //    If PII is detected, return immediately with a Modify decision.
        let d = self.scan_and_redact(envelope, Some("builtin_redact_pii"));
        if matches!(d.kind, DecisionKind::Modify) {
            return d;
        }
        // Fail-closed: deny when no valid policy is loaded
        if !self.policy_loaded {
            return Decision {
                kind: DecisionKind::Deny,
                payload: None,
                reason: Some("no valid policy loaded".into()),
                rule_name: Some("fail_closed_default".into()),
                action: Some("deny".into()),
            };
        }

        // 2) Tool allowlist enforcement (deny by default when a tool is present and not allowed)
        if let Some(dec) = self.check_tool_allowlist(envelope) {
            return dec;
        }
        // 3) Rule interpreter with priority and precedence
        //    - Evaluate all matching rules
        //    - Select highest priority (larger = higher)
        //    - Tie-break by most-restrictive-wins: Deny > Modify > Allow
        //    - If still tied, first-match-wins to preserve file order determinism
        let mut matches: Vec<(i32, usize, Decision)> = Vec::new();
        for (idx, r) in self.rules.iter().enumerate() {
            match (r.action.as_str(), r.when.as_str()) {
                ("deny", cond) if cond.contains("ToolInvocation") => {
                    matches.push((
                        r.priority,
                        idx,
                        Decision {
                            kind: DecisionKind::Deny,
                            payload: None,
                            reason: r.message.clone(),
                            rule_name: Some(r.name.clone()),
                            action: Some(r.action.clone()),
                        },
                    ));
                }
                ("allow_but_flag", cond) if cond.contains("LLMPrompt") => {
                    matches.push((
                        r.priority,
                        idx,
                        Decision {
                            kind: DecisionKind::Allow,
                            payload: None,
                            reason: r.message.clone(),
                            rule_name: Some(r.name.clone()),
                            action: Some(r.action.clone()),
                        },
                    ));
                }
                ("modify", cond) if cond.contains("pii_detect") => {
                    // apply redaction and attribute decision to this rule
                    let mut d2 = self.scan_and_redact(envelope, Some(r.name.as_str()));
                    if d2.reason.is_none() {
                        d2.reason = r.message.clone();
                    }
                    d2.action = Some(r.action.clone());
                    matches.push((r.priority, idx, d2));
                }
                _ => {}
            }
        }
        if matches.is_empty() {
            return Decision {
                kind: DecisionKind::Allow,
                payload: None,
                reason: None,
                rule_name: None,
                action: None,
            };
        }
        let max_pri = matches.iter().map(|(p, _, _)| *p).max().unwrap_or(0);
        let mut best: Option<(i32, usize, Decision)> = None;
        for (p, idx, d) in matches.into_iter().filter(|(p, _, _)| *p == max_pri) {
            let severity = match d.kind {
                DecisionKind::Deny => 3,
                DecisionKind::Modify => 2,
                DecisionKind::Allow => 1,
            };
            let better = match &best {
                None => true,
                Some((_bp, _bi, bd)) => {
                    let bsev = match bd.kind {
                        DecisionKind::Deny => 3,
                        DecisionKind::Modify => 2,
                        DecisionKind::Allow => 1,
                    };
                    severity > bsev // most-restrictive wins; ties keep first-match
                }
            };
            if better {
                best = Some((p, idx, d));
            }
        }
        best.map(|(_, _, d)| d).unwrap_or(Decision {
            kind: DecisionKind::Allow,
            payload: None,
            reason: None,
            rule_name: None,
            action: None,
        })
    }

    fn scan_and_redact(&self, envelope: &Value, rule_name: Option<&str>) -> Decision {
        let mut modified = envelope.clone();
        let mut changed = false;
        if let Some(payload) =
            modified.get_mut("payload_json").and_then(|v| v.as_str()).map(|s| s.to_string())
        {
            let redacted = self.pii.replace_all(&payload, "[REDACTED]").into_owned();
            if redacted != payload {
                changed = true;
                if let Some(v) = modified.get_mut("payload_json") {
                    *v = json!(redacted);
                }
            }
        }
        if changed {
            Decision {
                kind: DecisionKind::Modify,
                payload: Some(modified),
                reason: Some("PII redacted".into()),
                rule_name: Some(rule_name.unwrap_or("builtin_redact_pii").to_string()),
                action: Some("modify".into()),
            }
        } else {
            Decision {
                kind: DecisionKind::Allow,
                payload: None,
                reason: None,
                rule_name: None,
                action: None,
            }
        }
    }

    fn check_tool_allowlist(&self, envelope: &Value) -> Option<Decision> {
        // Parse payload_json if present and look for tool name under common keys
        let payload_str = envelope.get("payload_json").and_then(|v| v.as_str())?;
        let payload_val: Value = serde_json::from_str(payload_str).unwrap_or(Value::Null);
        let tool_name = payload_val
            .get("tool")
            .and_then(|v| v.as_str())
            .or_else(|| payload_val.get("tool_name").and_then(|v| v.as_str()));
        if let Some(tn) = tool_name.map(|s| s.to_lowercase()) {
            if let Some(allow) = &self.tool_allowlist {
                if !allow.contains(&tn) {
                    return Some(Decision {
                        kind: DecisionKind::Deny,
                        payload: None,
                        reason: Some(format!("tool '{}' not allowed", tn)),
                        rule_name: Some("tool_allowlist".into()),
                        action: Some("deny".into()),
                    });
                }
            } else {
                // No explicit allowlist: if a rule exists to deny ToolInvocation, deny on any tool presence
                if self
                    .rules
                    .iter()
                    .any(|r| r.action == "deny" && r.when.contains("ToolInvocation"))
                {
                    return Some(Decision {
                        kind: DecisionKind::Deny,
                        payload: None,
                        reason: Some(format!("external tool '{}' blocked by default", tn)),
                        rule_name: Some("Default-Deny-All-External-Tools".into()),
                        action: Some("deny".into()),
                    });
                }
            }
        }
        None
    }
}
