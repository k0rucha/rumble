//! メールの自動分類。
//!
//! 判断は TypeSafe の分類モデル Jev に任せる。Jev は文章を生成せず、
//! 「選択肢のどれか」とその確からしさを返すモデルで、1 通につき 1 リクエストで済む。
//!
//! 役割分担:
//!
//! - **何を送るか** ([`MailSignals`]) : 差出人・宛先・件名・メーリングリストかどうか。
//!   **本文は送らない**。一覧表示で取得済みの範囲に留めることで、分類のために
//!   本文を取得する必要がなく、外部に出る情報も最小限になる。
//! - **何を尋ねるか** ([`request_body`]) : 区分の定義 (英語。Jev の主な学習言語)。
//! - **答えをどう使うか** ([`decide`]) : 確信度が低ければ「その他」にする、という
//!   方針はモデルではなくこのコードが決める。

use std::time::Duration;

use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::domain::{Address, Category};
use crate::{Error, Result};

const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
const MODEL: &str = "jev-latest";
const QUESTION: &str = "category";

/// これ未満の確信度なら、誤った区分を付けるより「その他」にしておく。
/// 根拠のある値ではなく出発点。実際のメールで見ながら調整する。
const MIN_CONFIDENCE: f64 = 0.4;
/// 同時に投げるリクエスト数。
const MAX_PARALLEL: usize = 8;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// 混雑 (429 / 529) のときに待ってやり直す回数。
const MAX_RETRIES: u32 = 2;

/// 分類の手がかりとしてモデルに渡す情報。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MailSignals {
    pub from: Option<Address>,
    pub to: Vec<Address>,
    pub subject: String,
    /// List-Unsubscribe / List-Id ヘッダーがあるか (一斉配信の目印)
    pub is_mailing_list: bool,
}

pub struct Jev {
    http: reqwest::Client,
    api_key: String,
}

impl Jev {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: api_key.into(),
        }
    }

    /// 複数のメールを並行して分類する。
    ///
    /// 一部だけ失敗した場合は、成功したぶんを返す (残りは次の機会にやり直される)。
    /// 1 通も成功しなければ、その原因をエラーとして返す。
    pub async fn classify_all(
        &self,
        mails: Vec<(u32, MailSignals)>,
    ) -> Result<Vec<(u32, Category)>> {
        let outcomes: Vec<(u32, Result<Category>)> = stream::iter(mails)
            .map(|(uid, signals)| async move { (uid, self.classify(&signals).await) })
            .buffer_unordered(MAX_PARALLEL)
            .collect()
            .await;

        let mut classified = Vec::new();
        let mut first_error = None;
        for (uid, outcome) in outcomes {
            match outcome {
                Ok(category) => classified.push((uid, category)),
                Err(error) => first_error = first_error.or(Some(error)),
            }
        }
        match first_error {
            Some(error) if classified.is_empty() => Err(error),
            _ => Ok(classified),
        }
    }

    pub async fn classify(&self, signals: &MailSignals) -> Result<Category> {
        let body = request_body(signals);
        let mut attempt = 0;
        loop {
            let response = self
                .http
                .post(ENDPOINT)
                .bearer_auth(&self.api_key)
                .timeout(REQUEST_TIMEOUT)
                .json(&body)
                .send()
                .await
                .map_err(|e| Error::Classifier(format!("TypeSafe に接続できません ({e})")))?;

            let status = response.status().as_u16();
            match status {
                200 => {
                    let response: Response = response
                        .json()
                        .await
                        .map_err(|e| Error::Classifier(format!("応答を解釈できません ({e})")))?;
                    return Ok(decide(&response));
                }
                429 | 529 if attempt < MAX_RETRIES => {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
                }
                401 | 403 => {
                    return Err(Error::Classifier(
                        "TypeSafe の API キーが正しくありません".into(),
                    ))
                }
                429 | 529 => {
                    return Err(Error::Classifier(
                        "TypeSafe が混雑しています。少し待ってください".into(),
                    ))
                }
                _ => {
                    let detail = response.text().await.unwrap_or_default();
                    return Err(Error::Classifier(format!(
                        "TypeSafe がエラーを返しました ({status}) {detail}"
                    )));
                }
            }
        }
    }
}

/// Jev へのリクエスト本文。メール 1 通を state に、区分の選択を 1 つの Choice として尋ねる。
fn request_body(signals: &MailSignals) -> Value {
    json!({
        "model": MODEL,
        "state": { "email": signals },
        "questions": {
            QUESTION: {
                "type": "choice",
                "instructions": "Which kind of email is `email`, judging from its sender, recipients and subject? \
                    The subject may be written in Japanese or any other language. \
                    `email.is_mailing_list` is true when the message carries bulk-mail list headers.",
                "criteria": {
                    "personal": "Written by an individual person to the recipient: a friend, family member, \
                        colleague, client or other human correspondent, including replies in a conversation.",
                    "notification": "Automated notice from a service about the recipient's own account or activity: \
                        security alerts, sign-in codes, shipping updates, reminders, calendar invitations, \
                        social network or code hosting notifications.",
                    "receipt": "A record of a transaction: receipt, invoice, order or payment confirmation, \
                        billing statement, booking or ticket.",
                    "newsletter": "Bulk content sent to many subscribers: newsletter, marketing, promotion, \
                        sales offer, product announcement or digest.",
                    "other": "None of the above, or the available information is not enough to tell."
                }
            }
        }
    })
}

#[derive(Debug, Deserialize)]
struct Response {
    answers: std::collections::HashMap<String, Answer>,
}

#[derive(Debug, Deserialize)]
struct Answer {
    #[serde(default)]
    choice: String,
    #[serde(default)]
    confidence: f64,
}

/// モデルの答えから区分を決める。
fn decide(response: &Response) -> Category {
    match response.answers.get(QUESTION) {
        Some(answer) if answer.confidence >= MIN_CONFIDENCE => {
            Category::from_id(&answer.choice).unwrap_or(Category::Other)
        }
        _ => Category::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signals() -> MailSignals {
        MailSignals {
            from: Some(Address {
                name: Some("Shop".into()),
                email: "news@shop.example".into(),
            }),
            to: vec![Address {
                name: None,
                email: "me@example.com".into(),
            }],
            subject: "秋の大セール開催中".into(),
            is_mailing_list: true,
        }
    }

    fn answer(json: &str) -> Response {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn request_offers_exactly_the_known_categories() {
        let body = request_body(&signals());
        let question = &body["questions"][QUESTION];

        assert_eq!(body["model"], "jev-latest");
        assert_eq!(question["type"], "choice");
        let mut offered: Vec<&str> = question["criteria"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let mut known: Vec<&str> = Category::ALL.iter().map(|c| c.id()).collect();
        offered.sort_unstable();
        known.sort_unstable();
        assert_eq!(offered, known);
    }

    #[test]
    fn request_sends_headers_but_never_a_body() {
        let body = request_body(&signals());
        let email = body["state"]["email"].as_object().unwrap();

        assert_eq!(email["subject"], "秋の大セール開催中");
        assert_eq!(email["from"]["email"], "news@shop.example");
        assert_eq!(email["is_mailing_list"], true);
        let mut fields: Vec<&str> = email.keys().map(String::as_str).collect();
        fields.sort_unstable();
        assert_eq!(fields, ["from", "is_mailing_list", "subject", "to"]);
    }

    #[test]
    fn confident_answer_becomes_the_category() {
        // 公式ドキュメントの応答例と同じ形
        let response = answer(
            r#"{"model":"jev-1.13.0","answers":{"category":{"type":"choice","choice":"newsletter",
            "probabilities":{"newsletter":0.88,"receipt":0.12},"confidence":0.81}},
            "usage":{"input_tokens":318,"output_tokens":34}}"#,
        );
        assert_eq!(decide(&response), Category::Newsletter);
    }

    #[test]
    fn unsure_or_unexpected_answers_fall_back_to_other() {
        let unsure =
            r#"{"answers":{"category":{"type":"choice","choice":"receipt","confidence":0.2}}}"#;
        let unknown =
            r#"{"answers":{"category":{"type":"choice","choice":"spam","confidence":0.9}}}"#;
        let missing = r#"{"answers":{}}"#;

        assert_eq!(decide(&answer(unsure)), Category::Other);
        assert_eq!(decide(&answer(unknown)), Category::Other);
        assert_eq!(decide(&answer(missing)), Category::Other);
    }
}
