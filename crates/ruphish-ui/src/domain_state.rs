//! Domain detail reads and mutations cannot restore stale verification results.
use ruphish_contracts::domains::Domain;

/// Last confirmed domain and in-flight request ownership.
#[derive(Clone, Default)]
pub struct Details {
    /// Last confirmed server snapshot.
    pub domain: Option<Domain>,
    /// Refresh or mutation failure; never clears the confirmed snapshot.
    pub error: Option<String>,
    /// Active read ticket.
    pub reading: Option<u64>,
    /// Active mutation ticket.
    pub mutation: Option<u64>,
    generation: u64,
}

impl Details {
    /// Admit one refresh without overlapping a mutation.
    pub fn refresh(&mut self) -> Option<u64> {
        if self.reading.is_some() || self.mutation.is_some() {
            return None;
        }
        self.generation += 1;
        self.reading = Some(self.generation);
        self.reading
    }
    /// Admit verification only when no server attempt is active.
    pub fn verify(&mut self) -> Option<u64> {
        if self.mutation.is_some()
            || self
                .domain
                .as_ref()
                .is_none_or(|d| !d.enabled || d.active_check.is_some())
        {
            return None;
        }
        self.begin_mutation()
    }
    /// Invalidate earlier reads before a mutation.
    pub fn begin_mutation(&mut self) -> Option<u64> {
        if self.mutation.is_some() {
            return None;
        }
        self.generation += 1;
        self.reading = None;
        self.error = None;
        self.mutation = Some(self.generation);
        self.mutation
    }
    /// Apply only the response that still owns the current request ticket.
    pub fn received(&mut self, request: u64, result: Result<Domain, String>, mutation: bool) {
        let active = if mutation {
            self.mutation
        } else {
            self.reading
        };
        if active != Some(request) || request != self.generation {
            return;
        }
        if mutation {
            self.mutation = None;
        } else {
            self.reading = None;
        }
        match result {
            Ok(domain) => {
                self.domain = Some(domain);
                self.error = None;
            }
            Err(error) => {
                self.error = Some(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "Functional state workflow fixtures")]
    use super::*;
    fn domain(active: Option<&str>, status: &str) -> Domain {
        serde_json::from_value(serde_json::json!({"id":7,"name":"Training","origin":"https://training.example.test","hostname":"training.example.test","enabled":true,"revision":1,"txt_name":"_ruphish-verify.training.example.test","txt_value":"synthetic","challenge_expires":2000,"checked_at":100,"verified_until":2000,"control":true,"dns":true,"routing":true,"tls":true,"code":"verified","status":status,"eligible":status=="verified","checking":active==Some("running"),"campaigns":0,"history":[],"next_check_at":1000,"active_check":active.map(|state|serde_json::json!({"id":18,"state":state,"scheduled_at":100,"started_at":if state=="running" {Some(100)}else{None},"finished_at":null,"code":""}))})).unwrap()
    }
    #[test]
    fn refresh_mutation_and_completion_do_not_restore_an_old_attempt_or_hide_failures() {
        let mut s = Details::default();
        let initial = s.refresh().unwrap();
        s.received(initial, Ok(domain(None, "verified")), false);
        let stale = s.refresh().unwrap();
        let verify = s.verify().unwrap();
        assert!(s.verify().is_none());
        assert!(s.refresh().is_none());
        s.received(verify, Ok(domain(Some("queued"), "verified")), true);
        s.received(stale, Ok(domain(None, "verified")), false);
        assert_eq!(
            s.domain
                .as_ref()
                .unwrap()
                .active_check
                .as_ref()
                .unwrap()
                .state,
            "queued"
        );
        assert!(s.verify().is_none());
        let poll = s.refresh().unwrap();
        s.received(poll, Err("network unavailable".into()), false);
        assert!(s.error.is_some());
        assert!(s.domain.as_ref().unwrap().active_check.is_some());
        let poll = s.refresh().unwrap();
        s.received(poll, Ok(domain(Some("running"), "verified")), false);
        assert!(s.error.is_none());
        let poll = s.refresh().unwrap();
        s.received(poll, Ok(domain(None, "failed")), false);
        assert!(s.domain.as_ref().unwrap().active_check.is_none());
        assert!(!s.domain.as_ref().unwrap().eligible);
        assert!(s.verify().is_some());
    }
    #[test]
    fn reopening_restores_active_work_and_an_ambiguous_request_is_resolved_by_refresh() {
        for state in ["queued", "running"] {
            let mut s = Details::default();
            let read = s.refresh().unwrap();
            s.received(read, Ok(domain(Some(state), "not_verified")), false);
            assert!(s.verify().is_none());
            assert_eq!(s.domain.unwrap().active_check.unwrap().state, state);
        }
        let mut s = Details::default();
        let read = s.refresh().unwrap();
        s.received(read, Ok(domain(None, "failed")), false);
        let request = s.verify().unwrap();
        s.received(request, Err("connection lost".into()), true);
        assert!(s.error.is_some());
        let read = s.refresh().unwrap();
        s.received(read, Ok(domain(Some("running"), "failed")), false);
        assert!(s.error.is_none());
        assert!(s.verify().is_none());
        let read = s.refresh().unwrap();
        s.received(read, Ok(domain(None, "verified")), false);
        assert!(s.domain.as_ref().unwrap().eligible);
        s.domain.as_mut().unwrap().enabled = false;
        assert!(s.verify().is_none());
    }
}
