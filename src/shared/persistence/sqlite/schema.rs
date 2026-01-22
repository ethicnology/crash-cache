diesel::table! {
    archive (hash) {
        hash -> Text,
        compressed_payload -> Binary,
        original_size -> Integer,
        created_at -> Timestamp,
    }
}

diesel::table! {
    event (id) {
        id -> Integer,
        archive_hash -> Text,
        received_at -> Timestamp,
        processed -> Bool,
    }
}

diesel::table! {
    processing_queue (id) {
        id -> Integer,
        event_id -> Integer,
        created_at -> Timestamp,
        retry_count -> Integer,
        last_error -> Nullable<Text>,
        next_retry_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    crash_metadata (id) {
        id -> Integer,
        event_id -> Integer,
        app_version -> Nullable<Text>,
        platform -> Nullable<Text>,
        environment -> Nullable<Text>,
        error_type -> Nullable<Text>,
        error_message -> Nullable<Text>,
        sdk_name -> Nullable<Text>,
        sdk_version -> Nullable<Text>,
        processed_at -> Timestamp,
    }
}

diesel::joinable!(event -> archive (archive_hash));
diesel::joinable!(processing_queue -> event (event_id));
diesel::joinable!(crash_metadata -> event (event_id));

diesel::allow_tables_to_appear_in_same_query!(archive, event, processing_queue, crash_metadata,);
