//! DNS の応答 (公開鍵と DMARC レコード) を、有効期限のあいだメモリに覚えておく。
//!
//! 同じ差出人のメールを続けて開いたときに、毎回 DNS を引き直さないためのもの。
//! テストでは、DNS の代わりに答えを仕込む場所としても使う。

use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Mutex;
use std::time::Instant;

use mail_auth::{ResolverCache, Txt};

#[derive(Default)]
pub struct DnsCache {
    records: Mutex<HashMap<Box<str>, (Txt, Instant)>>,
}

impl ResolverCache<Box<str>, Txt> for DnsCache {
    fn get<Q>(&self, name: &Q) -> Option<Txt>
    where
        Box<str>: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let mut records = self.records.lock().unwrap();
        match records.get(name) {
            Some((record, valid_until)) if *valid_until > Instant::now() => Some(record.clone()),
            Some(_) => {
                records.remove(name);
                None
            }
            None => None,
        }
    }

    fn remove<Q>(&self, name: &Q) -> Option<Txt>
    where
        Box<str>: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let (record, _) = self.records.lock().unwrap().remove(name)?;
        Some(record)
    }

    fn insert(&self, name: Box<str>, record: Txt, valid_until: Instant) {
        self.records
            .lock()
            .unwrap()
            .insert(name, (record, valid_until));
    }
}
