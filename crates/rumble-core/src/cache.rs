//! 開いたメールのメモリ内キャッシュ。
//!
//! 同じメールを開き直したり添付を保存したりするたびにサーバーへ取りに行くのは
//! 無駄なので、直近に開いたぶんだけメモリに持つ。**ディスクには一切書かない**ので、
//! アプリを終了すれば何も残らない。上限 (バイト数) を超えたら古いものから捨てる。

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub account_id: String,
    pub mailbox: String,
    pub uid: u32,
}

pub struct MessageCache {
    capacity_bytes: usize,
    used_bytes: usize,
    entries: HashMap<CacheKey, Arc<Vec<u8>>>,
    /// 先頭が最も長く使われていないキー
    recency: VecDeque<CacheKey>,
}

impl MessageCache {
    pub fn new(capacity_bytes: usize) -> Self {
        Self {
            capacity_bytes,
            used_bytes: 0,
            entries: HashMap::new(),
            recency: VecDeque::new(),
        }
    }

    pub fn get(&mut self, key: &CacheKey) -> Option<Arc<Vec<u8>>> {
        let raw = self.entries.get(key)?.clone();
        self.touch(key);
        Some(raw)
    }

    pub fn insert(&mut self, key: CacheKey, raw: Arc<Vec<u8>>) {
        // 上限より大きい 1 通は、入れた瞬間に他を全部追い出すだけなので持たない
        if raw.len() > self.capacity_bytes {
            return;
        }
        self.remove(&key);
        self.used_bytes += raw.len();
        self.entries.insert(key.clone(), raw);
        self.recency.push_back(key);

        while self.used_bytes > self.capacity_bytes {
            let Some(oldest) = self.recency.front().cloned() else {
                break;
            };
            self.remove(&oldest);
        }
    }

    pub fn remove(&mut self, key: &CacheKey) {
        if let Some(raw) = self.entries.remove(key) {
            self.used_bytes -= raw.len();
            self.recency.retain(|k| k != key);
        }
    }

    /// アカウント削除時に、そのアカウントのメールをメモリからも消す。
    pub fn remove_account(&mut self, account_id: &str) {
        let keys: Vec<CacheKey> = self
            .entries
            .keys()
            .filter(|k| k.account_id == account_id)
            .cloned()
            .collect();
        for key in keys {
            self.remove(&key);
        }
    }

    fn touch(&mut self, key: &CacheKey) {
        self.recency.retain(|k| k != key);
        self.recency.push_back(key.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(uid: u32) -> CacheKey {
        CacheKey {
            account_id: "a".into(),
            mailbox: "INBOX".into(),
            uid,
        }
    }

    fn bytes(len: usize) -> Arc<Vec<u8>> {
        Arc::new(vec![0; len])
    }

    #[test]
    fn evicts_least_recently_used_when_over_capacity() {
        let mut cache = MessageCache::new(100);
        cache.insert(key(1), bytes(40));
        cache.insert(key(2), bytes(40));
        cache.get(&key(1)); // 1 を使ったので、次に追い出されるのは 2
        cache.insert(key(3), bytes(40));

        assert!(cache.get(&key(1)).is_some());
        assert!(cache.get(&key(2)).is_none());
        assert!(cache.get(&key(3)).is_some());
    }

    #[test]
    fn oversized_message_is_not_cached() {
        let mut cache = MessageCache::new(100);
        cache.insert(key(1), bytes(40));
        cache.insert(key(2), bytes(101));

        assert!(cache.get(&key(1)).is_some());
        assert!(cache.get(&key(2)).is_none());
    }

    #[test]
    fn reinserting_same_key_does_not_double_count() {
        let mut cache = MessageCache::new(100);
        cache.insert(key(1), bytes(60));
        cache.insert(key(1), bytes(60));
        cache.insert(key(2), bytes(40));

        assert!(cache.get(&key(1)).is_some());
        assert!(cache.get(&key(2)).is_some());
    }

    #[test]
    fn remove_account_drops_only_that_account() {
        let mut cache = MessageCache::new(100);
        cache.insert(key(1), bytes(10));
        let other = CacheKey {
            account_id: "b".into(),
            ..key(1)
        };
        cache.insert(other.clone(), bytes(10));
        cache.remove_account("a");

        assert!(cache.get(&key(1)).is_none());
        assert!(cache.get(&other).is_some());
    }
}
