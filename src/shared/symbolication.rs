//! Native crash processing uses an operator-configured Symbolicator service.
//! Original envelopes remain immutable in the archive, including binary dumps.
use std::io::Read;
use std::time::{Duration, Instant};

use reqwest::blocking::{Client, Response, multipart};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::shared::domain::{DomainError, SentryReport};
use crate::shared::parser::Envelope;

const RESPONSE_LIMIT: u64 = 8 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const JOB_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Deserialize)]
struct SymbolicationResponse {
    status: String,
    request_id: Option<String>,
    #[serde(default)]
    stacktraces: Vec<Value>,
    #[serde(default)]
    modules: Vec<Value>,
    signal: Option<i64>,
    system_info: Option<Value>,
    crash_reason: Option<String>,
    crashed: Option<bool>,
}

fn failure(message: &str) -> DomainError {
    DomainError::Processing(format!("Native symbolication: {message}"))
}

fn decode_response(response: Response) -> Result<SymbolicationResponse, DomainError> {
    if !response.status().is_success() {
        return Err(failure(&format!("HTTP {}", response.status().as_u16())));
    }
    let mut bytes = Vec::new();
    response.take(RESPONSE_LIMIT + 1).read_to_end(&mut bytes)
        .map_err(|_| failure("response read failed"))?;
    if bytes.len() as u64 > RESPONSE_LIMIT {
        return Err(failure("response exceeds limit"));
    }
    serde_json::from_slice(&bytes).map_err(|_| failure("invalid response"))
}

/// Called on the blocking digest worker, before opening its database transaction.
pub fn process_payload(data: &[u8], project_id: i32) -> Result<Option<SentryReport>, DomainError> {
    process_payload_with_config(data, project_id, || {
        let endpoint = std::env::var("SYMBOLICATOR_URL").map_err(|_| failure("SYMBOLICATOR_URL is not configured"))?;
        // This file is supplied by the operator, never by an event or attachment.
        let sources_path = std::env::var("SYMBOLICATOR_SOURCES_PATH").map_err(|_| failure("symbol sources are not configured"))?;
        let sources_data = std::fs::read(sources_path).map_err(|_| failure("cannot read symbol sources"))?;
        let sources: Vec<Value> = serde_json::from_slice(&sources_data).map_err(|_| failure("invalid symbol sources"))?;
        Ok((endpoint, sources))
    })
}

fn process_payload_with_config(
    data: &[u8],
    project_id: i32,
    configuration: impl FnOnce() -> Result<(String, Vec<Value>), DomainError>,
) -> Result<Option<SentryReport>, DomainError> {
    let envelope = Envelope::parse(data);
    let dump = envelope.as_ref().and_then(|envelope| envelope.items.iter().find(|item| {
        item.header.item_type == "attachment" && item.header.extra.get("attachment_type").and_then(Value::as_str) == Some("event.minidump")
    }));
    let mut event: Value = match envelope.as_ref().and_then(Envelope::find_event_payload) {
        Some(payload) => serde_json::from_slice(payload).map_err(|_| failure("invalid event JSON"))?,
        None if dump.is_some() => json!({}),
        None => match serde_json::from_slice(data) { Ok(event) => event, Err(_) => return Ok(None) },
    };
    let native_threads = event.pointer("/exception/values").and_then(Value::as_array).map(|exceptions| {
        exceptions.iter().filter_map(|exception| exception.get("stacktrace")).map(native_thread).collect::<Vec<Value>>()
    }).unwrap_or_default();
    let native_json = event.get("platform").and_then(Value::as_str) == Some("native")
        && event.pointer("/debug_meta/images").and_then(Value::as_array).is_some_and(|images| !images.is_empty())
        && native_threads.iter().any(|trace| trace.get("frames").and_then(Value::as_array).is_some_and(|frames| frames.iter().any(|frame| frame.get("instruction_addr").is_some())));
    if dump.is_none() && !native_json { return Ok(None); }
    let (endpoint, sources) = configuration()?;
    let endpoint = endpoint.trim_end_matches('/');
    let object = event.as_object_mut().ok_or_else(|| failure("event must be an object"))?;
    if !object.contains_key("event_id") {
        if let Some(id) = envelope.as_ref().and_then(|envelope| envelope.header.event_id.as_ref()) {
            object.insert("event_id".to_owned(), json!(id));
        }
    }
    let client = Client::builder().timeout(REQUEST_TIMEOUT).connect_timeout(Duration::from_secs(3))
        .redirect(reqwest::redirect::Policy::none()).build().map_err(|_| failure("client initialization failed"))?;
    let deadline = Instant::now() + JOB_TIMEOUT;
    let query = [("scope", project_id.to_string()), ("timeout", "5".to_owned())];
    let response = if let Some(dump) = dump {
        let form = multipart::Form::new()
            .part("upload_file_minidump", multipart::Part::bytes(dump.payload.clone()).file_name("crash.dmp"))
            .text("sources", serde_json::to_string(&sources).map_err(|_| failure("invalid symbol sources"))?)
            .text("platform", "\"native\"");
        client.post(format!("{endpoint}/minidump")).query(&query).multipart(form)
    } else {
        client.post(format!("{endpoint}/symbolicate")).query(&query).json(&json!({
            "platform":"native", "sources":sources, "modules":event.pointer("/debug_meta/images"),
            "stacktraces": native_threads, "options":{"frame_order":"caller_first"}
        }))
    }.send().map_err(|_| failure("service unavailable"))?;
    let mut response = decode_response(response)?;
    loop {
        match response.status.as_str() {
            "completed" => return if dump.is_some() { normalize_result(event, response) } else { normalize_native_json(event, response) }.map(Some),
            "pending" => {
                if Instant::now() >= deadline { return Err(failure("job timed out")); }
                let request_id = response.request_id.as_deref().ok_or_else(|| failure("pending response has no request ID"))?;
                if request_id.is_empty() || !request_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                    return Err(failure("invalid request ID"));
                }
                std::thread::sleep(Duration::from_millis(100));
                response = decode_response(client.get(format!("{endpoint}/requests/{request_id}"))
                    .query(&[("timeout", "5")]).send().map_err(|_| failure("poll failed"))?)?;
            }
            _ => return Err(failure("service rejected native crash")),
        }
    }
}

/// Android's tombstone unwinder may already adjust every program counter.
/// Symbolicator accepts that instruction on individual frames, not on the trace.
fn native_thread(stacktrace: &Value) -> Value {
    let mut trace = stacktrace.clone();
    let adjustment = match trace.get("instruction_addr_adjustment").and_then(Value::as_str) {
        Some("none") => Some(false),
        Some("all") => Some(true),
        _ => None,
    };
    if let Some(adjustment) = adjustment {
        if let Some(frames) = trace.get_mut("frames").and_then(Value::as_array_mut) {
            for frame in frames {
                if let Some(frame) = frame.as_object_mut() {
                    frame.insert("adjust_instruction_addr".to_owned(), json!(adjustment));
                }
            }
        }
    }
    trace
}

fn normalize_native_json(mut event: Value, response: SymbolicationResponse) -> Result<SentryReport, DomainError> {
    let exceptions = event.pointer_mut("/exception/values").and_then(Value::as_array_mut).ok_or_else(|| failure("missing native exceptions"))?;
    let expected = exceptions.iter().filter(|exception| exception.get("stacktrace").is_some()).count();
    if response.stacktraces.len() != expected { return Err(failure("native stacktrace count changed")); }
    let mut traces = response.stacktraces.into_iter();
    let mut total = 0;
    let mut resolved = 0;
    for exception in exceptions {
        let Some(original) = exception.get_mut("stacktrace") else { continue; };
        let mut trace = traces.next().ok_or_else(|| failure("missing native stacktrace"))?;
        let frames = trace.get_mut("frames").and_then(Value::as_array_mut).ok_or_else(|| failure("missing native frames"))?;
        let original_frames = original.get("frames").and_then(Value::as_array);
        for frame in frames {
            total += 1;
            if frame.get("status").and_then(Value::as_str) == Some("symbolicated") { resolved += 1; }
            let index = frame.get("original_index").and_then(Value::as_u64).and_then(|index| usize::try_from(index).ok());
            if let Some(old_frame) = index.and_then(|index| original_frames.and_then(|frames| frames.get(index))).and_then(Value::as_object) {
                let object = frame.as_object_mut().ok_or_else(|| failure("invalid native frame"))?;
                for (key, value) in old_frame { object.entry(key.clone()).or_insert_with(|| value.clone()); }
            }
        }
        *original = trace;
    }
    event["debug_meta"] = json!({"images":response.modules});
    let contexts = event.as_object_mut().ok_or_else(|| failure("invalid event"))?.entry("contexts").or_insert_with(|| json!({})).as_object_mut().ok_or_else(|| failure("invalid contexts"))?;
    contexts.insert("symbolication".to_owned(), json!({"status":if resolved == total && total > 0 {"complete"} else if resolved > 0 {"partial"} else {"missing_symbols"},"resolved_frames":resolved}));
    serde_json::from_value(event).map_err(|_| failure("invalid normalized native event"))
}

fn normalize_result(mut event: Value, response: SymbolicationResponse) -> Result<SentryReport, DomainError> {
    if response.crashed == Some(false) {
        return Err(failure("dump does not describe a crashed process"));
    }
    let trace = response.stacktraces.iter().find(|trace| trace.get("is_requesting").and_then(Value::as_bool) == Some(true))
        .or_else(|| response.stacktraces.first()).ok_or_else(|| failure("no stacktrace returned"))?;
    let mut frames = trace.get("frames").and_then(Value::as_array).cloned().ok_or_else(|| failure("no frames returned"))?;
    if frames.is_empty() { return Err(failure("empty native stacktrace")); }
    // Minidump stackwalking returns callee-first; the Sentry event protocol is caller-first.
    frames.reverse();
    let resolved = frames.iter().filter(|frame| frame.get("status").and_then(Value::as_str) == Some("symbolicated")).count();
    let resolution_status = if resolved == frames.len() { "complete" } else if resolved > 0 { "partial" } else { "missing_symbols" };
    let object = event.as_object_mut().ok_or_else(|| failure("event must be an object"))?;
    object.insert("platform".to_owned(), json!("native"));
    object.insert("level".to_owned(), json!("fatal"));
    object.insert("exception".to_owned(), json!({"values":[{
        "type": response.crash_reason.unwrap_or_else(|| "NativeCrash".to_owned()), "value": response.signal.map(|signal| format!("Signal {signal}")).unwrap_or_else(|| "Native process crash".to_owned()),
        "stacktrace": {"frames": frames}, "mechanism": {"type":"minidump", "handled":false}
    }]}));
    object.insert("debug_meta".to_owned(), json!({"images":response.modules}));
    let contexts = object.entry("contexts").or_insert_with(|| json!({})).as_object_mut().ok_or_else(|| failure("invalid event contexts"))?;
    contexts.insert("symbolication".to_owned(), json!({"status": resolution_status, "resolved_frames":resolved}));
    if let Some(system) = response.system_info {
        contexts.entry("os").or_insert_with(|| json!({"name":system.get("os_name"),"version":system.get("os_version"),"build":system.get("os_build")}));
        contexts.entry("device").or_insert_with(|| json!({"model":system.get("device_model"),"arch":system.get("cpu_arch")}));
    }
    serde_json::from_value(event).map_err(|_| failure("invalid normalized event"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbolication_preserves_identity_and_reverses_frames() {
        let response = serde_json::from_value(json!({"status":"completed","signal":11,"stacktraces":[{"frames":[{"function":"crash","status":"symbolicated"},{"function":"main","status":"symbolicated"}]}]})).unwrap();
        let report = normalize_result(json!({"event_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","tags":{"layer":"native"}}), response).unwrap();
        assert_eq!(report.event_id.as_deref(), Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let frames = report.exception.unwrap().values.unwrap().remove(0).stacktrace.unwrap().frames.unwrap();
        assert_eq!(frames[0].function.as_deref(), Some("main"));
        assert_eq!(frames[1].function.as_deref(), Some("crash"));
        assert_eq!(report.tags.unwrap()["layer"], "native");
    }

    #[test]
    fn symbolication_missing_stack_is_not_success() {
        let response = serde_json::from_value(json!({"status":"completed"})).unwrap();
        assert!(normalize_result(json!({}), response).is_err());
    }
    #[test]
    fn symbolication_native_json_preserves_exception_and_application_context() {
        let event = json!({
            "event_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "platform":"native",
            "user":{"id":"installation"}, "tags":{"layer":"godot"},
            "exception":{"values":[{"type":"SIGSEGV","value":"native fault","stacktrace":{"frames":[{"instruction_addr":"0x42","in_app":true}]}}]}
        });
        let response = serde_json::from_value(json!({"status":"completed","modules":[],"stacktraces":[{"frames":[{"original_index":0,"instruction_addr":"0x42","function":"crash_probe","status":"symbolicated"}]}]})).unwrap();
        let report = normalize_native_json(event, response).unwrap();
        let exception = report.exception.unwrap().values.unwrap().remove(0);
        assert_eq!(exception.exception_type.as_deref(), Some("SIGSEGV"));
        let frame = exception.stacktrace.unwrap().frames.unwrap().remove(0);
        assert_eq!(frame.in_app, Some(true));
        assert_eq!(frame.function.as_deref(), Some("crash_probe"));
        assert_eq!(report.user.unwrap().id.as_deref(), Some("installation"));
        assert_eq!(report.tags.unwrap()["layer"], "godot");
    }

    /// Run explicitly against the pinned service, without external symbol sources.
    #[test]
    #[ignore = "requires SYMBOLICATOR_URL pointing to a running Symbolicator 26.9.0 service"]
    fn symbolicator_http_preserves_native_event_with_missing_symbols() {
        let endpoint = std::env::var("SYMBOLICATOR_URL").expect("SYMBOLICATOR_URL is required");
        let event = json!({
            "event_id":"c9cc622201424caebfc5e6e360152021", "platform":"native", "level":"fatal",
            "user":{"id":"synthetic-installation"}, "tags":{"layer":"native","component":"contract-test"},
            "debug_meta":{"images":[{"type":"elf","debug_id":"00000000-0000-4000-8000-000000000001","code_id":"00000000000040008000000000000001","debug_file":"synthetic-missing.so","code_file":"synthetic-missing.so","image_addr":"0x1000","image_size":4096}]},
            "exception":{"values":[{"type":"SIGSEGV","value":"synthetic contract fixture","stacktrace":{"frames":[{"instruction_addr":"0x1042","in_app":true,"adjust_instruction_addr":false}]}}]}
        });
        let payload = serde_json::to_vec(&event).unwrap();
        let report = process_payload_with_config(&payload, 4242, || Ok((endpoint, vec![]))).unwrap().expect("native event must be processed");
        assert_eq!(report.event_id.as_deref(), Some("c9cc622201424caebfc5e6e360152021"));
        assert_eq!(report.user.unwrap().id.as_deref(), Some("synthetic-installation"));
        assert_eq!(report.tags.unwrap()["component"], "contract-test");
        let context = report.contexts.unwrap().extra.remove("symbolication").unwrap();
        assert_eq!(context["status"], "missing_symbols");
        assert_eq!(context["resolved_frames"], 0);
        let exception = report.exception.unwrap().values.unwrap().remove(0);
        assert_eq!(exception.exception_type.as_deref(), Some("SIGSEGV"));
        let frames = exception.stacktrace.unwrap().frames.unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].extra["instruction_addr"], "0x1042");
    }

    #[test]
    #[ignore = "requires SYMBOLICATOR_URL pointing to a running Symbolicator 26.9.0 service"]
    fn symbolicator_http_rejects_malformed_minidump_explicitly() {
        let endpoint = std::env::var("SYMBOLICATOR_URL").expect("SYMBOLICATOR_URL is required");
        let data = b"{\"event_id\":\"c9cc622201424caebfc5e6e360152021\"}\n{\"type\":\"attachment\",\"attachment_type\":\"event.minidump\",\"length\":4}\nMDMP";
        let error = process_payload_with_config(data, 4242, || Ok((endpoint, vec![]))).unwrap_err();
        assert!(matches!(error, DomainError::Processing(ref message) if message == "Native symbolication: service rejected native crash"), "unexpected error: {error}");
    }

    #[test]
    fn native_thread_preserves_pre_adjusted_tombstone_addresses() {
        let trace = json!({"instruction_addr_adjustment":"none","frames":[{"instruction_addr":"0x1042"},{"instruction_addr":"0x2042"}]});
        let prepared = native_thread(&trace);
        assert_eq!(prepared["frames"][0]["adjust_instruction_addr"], false);
        assert_eq!(prepared["frames"][1]["adjust_instruction_addr"], false);
        assert_eq!(prepared["frames"][0]["instruction_addr"], "0x1042");
        assert!(trace["frames"][0].get("adjust_instruction_addr").is_none());
        let auto = native_thread(&json!({"frames":[{"instruction_addr":"0x1042"}]}));
        assert!(auto["frames"][0].get("adjust_instruction_addr").is_none());
    }

}
