//! Short-lived memory of finished searches. The suggestions panel searches while the user types, and pressing Enter asks the
//! same question again; answering that from memory makes the final search instant and spends no quota.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::request::SearchRequest;
use super::response::{GroupStatus, SearchResponse, SkipReason};
use crate::core::domain::ProviderId;

const TTL: Duration = Duration::from_secs(180);
const CAPACITY: usize = 48;

#[derive(Default)]
pub struct ResponseCache {
    entries: HashMap<String, (Instant, SearchResponse)>,
}

impl ResponseCache {
    /// Same words in any case or spacing, same providers and limit, give the same key.
    pub fn key(req: &SearchRequest, providers: &[ProviderId]) -> String {
        let words: Vec<String> = req
            .query
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let ids: Vec<&str> = providers.iter().map(ProviderId::as_str).collect();
        format!("{}|{}|{}", ids.join(","), req.limit, words.join(" "))
    }

    pub fn get(&mut self, key: &str, now: Instant) -> Option<SearchResponse> {
        match self.entries.get(key) {
            Some((at, response)) if now.saturating_duration_since(*at) < TTL => {
                Some(response.clone())
            }
            Some(_) => {
                self.entries.remove(key);
                None
            }
            None => None,
        }
    }

    /// Keeps only complete answers: a failed, cancelled or over-quota group must be asked again.
    pub fn put(&mut self, key: String, response: &SearchResponse, now: Instant) {
        let complete = response.groups.iter().all(|g| {
            matches!(
                g.status,
                GroupStatus::Ok(_) | GroupStatus::Skipped(SkipReason::CapabilityNotSupported)
            )
        });
        if !complete {
            return;
        }
        if self.entries.len() >= CAPACITY
            && let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(k, _)| k.clone())
        {
            self.entries.remove(&oldest);
        }
        self.entries.insert(key, (now, response.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::errors::CoreError;
    use crate::core::search::response::SourceGroup;

    fn response(status: GroupStatus) -> SearchResponse {
        SearchResponse::new(
            "q".into(),
            vec![SourceGroup::new(ProviderId::YouTube, status)],
        )
    }

    fn req(query: &str, limit: u8) -> SearchRequest {
        SearchRequest::new(query, vec![], limit).unwrap()
    }

    #[test]
    fn key_ignores_case_and_spacing_but_not_limit() {
        let p = [ProviderId::YouTube];
        assert_eq!(
            ResponseCache::key(&req("Fame  is a GUN", 10), &p),
            ResponseCache::key(&req("fame is a gun", 10), &p)
        );
        assert_ne!(
            ResponseCache::key(&req("fame", 10), &p),
            ResponseCache::key(&req("fame", 5), &p)
        );
    }

    #[test]
    fn hit_until_ttl_then_miss() {
        let mut cache = ResponseCache::default();
        let t0 = Instant::now();
        cache.put("k".into(), &response(GroupStatus::Ok(vec![])), t0);
        assert!(cache.get("k", t0 + Duration::from_secs(10)).is_some());
        assert!(cache.get("k", t0 + TTL + Duration::from_secs(1)).is_none());
        assert!(
            cache.get("k", t0 + Duration::from_secs(10)).is_none(),
            "expired entries are dropped"
        );
    }

    #[test]
    fn incomplete_answers_are_not_kept() {
        let mut cache = ResponseCache::default();
        let t0 = Instant::now();
        cache.put(
            "failed".into(),
            &response(GroupStatus::Failed(CoreError::ProviderUnavailable)),
            t0,
        );
        cache.put(
            "cancelled".into(),
            &response(GroupStatus::Skipped(SkipReason::Cancelled)),
            t0,
        );
        cache.put(
            "quota".into(),
            &response(GroupStatus::Skipped(SkipReason::QuotaExhausted)),
            t0,
        );
        for key in ["failed", "cancelled", "quota"] {
            assert!(cache.get(key, t0).is_none(), "{key}");
        }
    }

    #[test]
    fn oldest_entry_goes_when_full() {
        let mut cache = ResponseCache::default();
        let t0 = Instant::now();
        for i in 0..CAPACITY {
            cache.put(
                format!("k{i}"),
                &response(GroupStatus::Ok(vec![])),
                t0 + Duration::from_millis(i as u64),
            );
        }
        cache.put(
            "new".into(),
            &response(GroupStatus::Ok(vec![])),
            t0 + Duration::from_secs(1),
        );
        assert!(cache.get("k0", t0 + Duration::from_secs(1)).is_none());
        assert!(cache.get("new", t0 + Duration::from_secs(1)).is_some());
    }
}
