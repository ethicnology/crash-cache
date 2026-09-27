// ============================================
// CORE TABLES
// ============================================

diesel::table! {
    project (id) {
        id -> Integer,
        public_key -> Nullable<Text>,
        name -> Nullable<Text>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    archive (hash) {
        hash -> Text,
        project_id -> Integer,
        compressed_payload -> Binary,
        original_size -> Nullable<Integer>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    queue (id) {
        id -> Integer,
        archive_hash -> Text,
        created_at -> Timestamp,
    }
}

diesel::table! {
    queue_error (id) {
        id -> Integer,
        archive_hash -> Text,
        error -> Text,
        created_at -> Timestamp,
    }
}

// ============================================
// ANALYTICS BUCKET TABLES
// ============================================

diesel::table! {
    bucket_rate_limit_global (id) {
        id -> Integer,
        bucket_start -> Timestamp,
        hit_count -> Integer,
    }
}

diesel::table! {
    bucket_rate_limit_dsn (id) {
        id -> Integer,
        dsn -> Text,
        project_id -> Nullable<Integer>,
        bucket_start -> Timestamp,
        hit_count -> Integer,
    }
}

diesel::table! {
    bucket_rate_limit_subnet (id) {
        id -> Integer,
        subnet -> Text,
        bucket_start -> Timestamp,
        hit_count -> Integer,
    }
}

diesel::table! {
    bucket_request_latency (id) {
        id -> Integer,
        endpoint -> Text,
        bucket_start -> Timestamp,
        request_count -> Integer,
        total_ms -> Integer,
        min_ms -> Nullable<Integer>,
        max_ms -> Nullable<Integer>,
    }
}

// ============================================
// SESSION TABLES
// ============================================

diesel::table! {
    unwrap_session_status (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_session_release (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_session_environment (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    session (id) {
        id -> Integer,
        project_id -> Integer,
        sid -> Text,
        init -> Integer,
        started_at -> Text,
        timestamp -> Text,
        errors -> Integer,
        status_id -> Integer,
        release_id -> Nullable<Integer>,
        environment_id -> Nullable<Integer>,
        distinct_id -> Nullable<Text>,
        sequence -> Text,
        duration -> Nullable<Double>,
        abnormal_mechanism -> Nullable<Text>,
    }
}

// ============================================
// UNWRAP TABLES
// ============================================

diesel::table! {
    unwrap_platform (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_environment (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_connection_type (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_orientation (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_os_name (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_os_version (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_manufacturer (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_brand (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_model (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_chipset (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_locale_code (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_timezone (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_app_name (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_app_version (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_app_build (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_user (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_exception_type (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_tag_key (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_tag_value (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_context_key (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_context_value (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_device_specs (id) {
        id -> Integer,
        screen_width -> Nullable<Integer>,
        screen_height -> Nullable<Integer>,
        screen_density -> Nullable<Float>,
        screen_dpi -> Nullable<Integer>,
        processor_count -> Nullable<Integer>,
        memory_size -> Nullable<BigInt>,
        archs -> Nullable<Text>,
    }
}

diesel::table! {
    unwrap_exception_message (id) {
        id -> Integer,
        hash -> Text,
        value -> Text,
    }
}

diesel::table! {
    unwrap_stacktrace (id) {
        id -> Integer,
        hash -> Text,
        fingerprint_hash -> Nullable<Text>,
        frames -> Jsonb,
    }
}

diesel::table! {
    unwrap_breadcrumb_category (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_breadcrumb_type (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_breadcrumb_level (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    unwrap_breadcrumb (id) {
        id -> Integer,
        hash -> Text,
        timestamp -> Nullable<BigInt>,
        category_id -> Nullable<Integer>,
        type_id -> Nullable<Integer>,
        level_id -> Nullable<Integer>,
        data -> Nullable<Jsonb>,
    }
}

// ============================================
// ISSUE TABLE
// ============================================

diesel::table! {
    issue (id) {
        id -> Integer,
        fingerprint_hash -> Text,
        exception_type_id -> Nullable<Integer>,
        title -> Nullable<Text>,
        first_seen -> Timestamp,
        last_seen -> Timestamp,
        event_count -> Integer,
    }
}

// ============================================
// REPORT TABLE
// ============================================

diesel::table! {
    report (id) {
        id -> Integer,
        event_id -> Text,
        archive_hash -> Text,
        timestamp -> BigInt,
        received_at -> Timestamp,

        project_id -> Integer,
        platform_id -> Nullable<Integer>,
        environment_id -> Nullable<Integer>,

        os_name_id -> Nullable<Integer>,
        os_version_id -> Nullable<Integer>,

        manufacturer_id -> Nullable<Integer>,
        brand_id -> Nullable<Integer>,
        model_id -> Nullable<Integer>,
        chipset_id -> Nullable<Integer>,
        device_specs_id -> Nullable<Integer>,

        locale_code_id -> Nullable<Integer>,
        timezone_id -> Nullable<Integer>,
        connection_type_id -> Nullable<Integer>,
        orientation_id -> Nullable<Integer>,

        app_name_id -> Nullable<Integer>,
        app_version_id -> Nullable<Integer>,
        app_build_id -> Nullable<Integer>,

        user_id -> Nullable<Integer>,

        exception_type_id -> Nullable<Integer>,
        exception_message_id -> Nullable<Integer>,
        stacktrace_id -> Nullable<Integer>,
        issue_id -> Nullable<Integer>,
        session_id -> Nullable<Integer>,
    }
}

// ============================================
// REPORT JOIN TABLES (tags + contexts)
// ============================================

diesel::table! {
    report_tag (report_id, key_id) {
        report_id -> Integer,
        key_id -> Integer,
        value_id -> Integer,
    }
}

diesel::table! {
    report_context (report_id, key_id) {
        report_id -> Integer,
        key_id -> Integer,
        value_id -> Integer,
    }
}

diesel::table! {
    report_breadcrumb (report_id, seq) {
        report_id -> Integer,
        seq -> Integer,
        breadcrumb_id -> Integer,
    }
}

// ============================================
// JOINABLE RELATIONS
// ============================================

diesel::joinable!(queue -> archive (archive_hash));
diesel::joinable!(queue_error -> archive (archive_hash));
diesel::joinable!(report -> archive (archive_hash));
diesel::joinable!(report -> project (project_id));
diesel::joinable!(report -> unwrap_platform (platform_id));
diesel::joinable!(report -> unwrap_environment (environment_id));
diesel::joinable!(report -> unwrap_os_name (os_name_id));
diesel::joinable!(report -> unwrap_os_version (os_version_id));
diesel::joinable!(report -> unwrap_manufacturer (manufacturer_id));
diesel::joinable!(report -> unwrap_brand (brand_id));
diesel::joinable!(report -> unwrap_model (model_id));
diesel::joinable!(report -> unwrap_chipset (chipset_id));
diesel::joinable!(report -> unwrap_device_specs (device_specs_id));
diesel::joinable!(report -> unwrap_locale_code (locale_code_id));
diesel::joinable!(report -> unwrap_timezone (timezone_id));
diesel::joinable!(report -> unwrap_connection_type (connection_type_id));
diesel::joinable!(report -> unwrap_orientation (orientation_id));
diesel::joinable!(report -> unwrap_app_name (app_name_id));
diesel::joinable!(report -> unwrap_app_version (app_version_id));
diesel::joinable!(report -> unwrap_app_build (app_build_id));
diesel::joinable!(report -> unwrap_user (user_id));
diesel::joinable!(report -> unwrap_exception_type (exception_type_id));
diesel::joinable!(report -> unwrap_exception_message (exception_message_id));
diesel::joinable!(report -> unwrap_stacktrace (stacktrace_id));
diesel::joinable!(report -> issue (issue_id));
diesel::joinable!(issue -> unwrap_exception_type (exception_type_id));
diesel::joinable!(session -> project (project_id));
diesel::joinable!(session -> unwrap_session_status (status_id));
diesel::joinable!(session -> unwrap_session_release (release_id));
diesel::joinable!(session -> unwrap_session_environment (environment_id));
diesel::joinable!(report -> session (session_id));
diesel::joinable!(report_tag -> report (report_id));
diesel::joinable!(report_tag -> unwrap_tag_key (key_id));
diesel::joinable!(report_tag -> unwrap_tag_value (value_id));
diesel::joinable!(report_context -> report (report_id));
diesel::joinable!(report_context -> unwrap_context_key (key_id));
diesel::joinable!(report_context -> unwrap_context_value (value_id));
diesel::joinable!(report_breadcrumb -> report (report_id));
diesel::joinable!(report_breadcrumb -> unwrap_breadcrumb (breadcrumb_id));
diesel::joinable!(unwrap_breadcrumb -> unwrap_breadcrumb_category (category_id));
diesel::joinable!(unwrap_breadcrumb -> unwrap_breadcrumb_type (type_id));
diesel::joinable!(unwrap_breadcrumb -> unwrap_breadcrumb_level (level_id));

diesel::allow_tables_to_appear_in_same_query!(
    project,
    archive,
    queue,
    queue_error,
    session,
    unwrap_session_status,
    unwrap_session_release,
    unwrap_session_environment,
    unwrap_platform,
    unwrap_environment,
    unwrap_connection_type,
    unwrap_orientation,
    unwrap_os_name,
    unwrap_os_version,
    unwrap_manufacturer,
    unwrap_brand,
    unwrap_model,
    unwrap_chipset,
    unwrap_locale_code,
    unwrap_timezone,
    unwrap_app_name,
    unwrap_app_version,
    unwrap_app_build,
    unwrap_user,
    unwrap_exception_type,
    unwrap_device_specs,
    unwrap_exception_message,
    unwrap_stacktrace,
    issue,
    report,
    bucket_rate_limit_global,
    bucket_rate_limit_dsn,
    bucket_rate_limit_subnet,
    bucket_request_latency,
    unwrap_tag_key,
    unwrap_tag_value,
    unwrap_context_key,
    unwrap_context_value,
    unwrap_breadcrumb_category,
    unwrap_breadcrumb_type,
    unwrap_breadcrumb_level,
    unwrap_breadcrumb,
    report_tag,
    report_context,
    report_breadcrumb,
);
