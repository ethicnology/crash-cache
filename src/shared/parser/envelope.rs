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
        let header_end = data.iter().position(|&byte| byte == b'\n').unwrap_or(data.len());
        let header = serde_json::from_slice(&data[..header_end]).ok()?;
        let mut offset = (header_end + 1).min(data.len());
        let mut items = Vec::new();
        while offset < data.len() {
            let header_length = data[offset..].iter().position(|&byte| byte == b'\n')?;
            let payload_start = offset + header_length + 1;
            let item_header: ItemHeader = serde_json::from_slice(&data[offset..payload_start - 1]).ok()?;
            let length = item_header.length.unwrap_or_else(|| {
                data[payload_start..].iter().position(|&byte| byte == b'\n').unwrap_or(data.len() - payload_start)
            });
            let payload_end = payload_start.checked_add(length)?;
            let payload = data.get(payload_start..payload_end)?.to_vec();
            if payload_end < data.len() && data[payload_end] != b'\n' {
                return None;
            }
            items.push(EnvelopeItem { header: item_header, payload });
            offset = payload_end.saturating_add(1);
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

#[cfg(test)]
mod protocol_tests {
    use super::Envelope;

    #[test]
    fn rejects_truncated_explicit_payload() {
        assert!(Envelope::parse(b"{}\n{\"type\":\"attachment\",\"length\":4}\nabc").is_none());
    }

    #[test]
    fn rejects_non_newline_after_explicit_payload() {
        assert!(Envelope::parse(b"{}\n{\"type\":\"event\",\"length\":2}\n{}garbage").is_none());
    }

    #[test]
    fn rejects_malformed_item_header_instead_of_silently_skipping() {
        assert!(Envelope::parse(b"{}\nnot-json\n{}\n").is_none());
    }

    #[test]
    fn preserves_binary_unknown_items_and_following_event() {
        let data = b"{}\n{\"type\":\"future\",\"length\":4}\n\xff\n\x00x\n{\"type\":\"event\"}\n{}";
        let envelope = Envelope::parse(data).unwrap();
        assert_eq!(envelope.items.len(), 2);
        assert_eq!(envelope.items[0].payload, b"\xff\n\x00x");
        assert_eq!(envelope.find_event_payload(), Some(b"{}".as_slice()));
    }
}
