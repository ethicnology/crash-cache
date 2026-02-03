use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Envelope {
    pub header: EnvelopeHeader,
    pub items: Vec<EnvelopeItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopeHeader {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dsn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent_at: Option<String>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct EnvelopeItem {
    pub header: ItemHeader,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemHeader {
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, Value>,
}

impl Envelope {
    pub fn parse(data: &[u8]) -> Option<Self> {
        let mut lines = data.split(|&b| b == b'\n');

        let header_line = lines.next()?;
        let header: EnvelopeHeader = serde_json::from_slice(header_line).ok()?;

        let mut items = Vec::new();
        while let Some(item_header_line) = lines.next() {
            if item_header_line.is_empty() {
                continue;
            }

            let item_header: ItemHeader = match serde_json::from_slice(item_header_line) {
                Ok(h) => h,
                Err(_) => continue,
            };

            let payload = if let Some(length) = item_header.length {
                let remaining: Vec<u8> = lines
                    .clone()
                    .flat_map(|l| {
                        let mut v = l.to_vec();
                        v.push(b'\n');
                        v
                    })
                    .collect();

                let payload = remaining.get(..length)?.to_vec();

                let mut consumed = 0;
                while consumed < length {
                    if let Some(line) = lines.next() {
                        consumed += line.len() + 1;
                    } else {
                        break;
                    }
                }
                payload
            } else {
                let next_line = lines.next().unwrap_or(&[]);
                next_line.to_vec()
            };

            items.push(EnvelopeItem {
                header: item_header,
                payload,
            });
        }

        Some(Envelope { header, items })
    }

    pub fn find_event_payload(&self) -> Option<&[u8]> {
        self.items
            .iter()
            .find(|item| item.header.item_type == "event")
            .map(|item| item.payload.as_slice())
    }

    pub fn find_transaction_payload(&self) -> Option<&[u8]> {
        self.items
            .iter()
            .find(|item| item.header.item_type == "transaction")
            .map(|item| item.payload.as_slice())
    }

    pub fn find_session_payloads(&self) -> Vec<&[u8]> {
        self.items
            .iter()
            .filter(|item| item.header.item_type == "session")
            .map(|item| item.payload.as_slice())
            .collect()
    }
}
