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
// LOOKUP TABLES
// ============================================

diesel::table! {
    lookup_platform (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    lookup_environment (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    lookup_connection_type (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    lookup_orientation (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    lookup_os_name (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    lookup_os_version (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    lookup_manufacturer (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    lookup_brand (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    lookup_model (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    lookup_chipset (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    lookup_locale_code (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    lookup_timezone (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    lookup_app_name (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    lookup_app_version (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    lookup_app_build (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    lookup_user (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    lookup_exception_type (id) {
        id -> Integer,
        value -> Text,
    }
}

diesel::table! {
    lookup_device_specs (id) {
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
    lookup_exception_message (id) {
        id -> Integer,
        hash -> Text,
        value -> Text,
    }
}

diesel::table! {
    lookup_stacktrace (id) {
        id -> Integer,
        hash -> Text,
        fingerprint_hash -> Nullable<Text>,
        frames_json -> Binary,
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
    }
}

// ============================================
// JOINABLE RELATIONS
// ============================================

diesel::joinable!(queue -> archive (archive_hash));
diesel::joinable!(queue_error -> archive (archive_hash));
diesel::joinable!(report -> archive (archive_hash));
diesel::joinable!(report -> project (project_id));
diesel::joinable!(report -> lookup_platform (platform_id));
diesel::joinable!(report -> lookup_environment (environment_id));
diesel::joinable!(report -> lookup_os_name (os_name_id));
diesel::joinable!(report -> lookup_os_version (os_version_id));
diesel::joinable!(report -> lookup_manufacturer (manufacturer_id));
diesel::joinable!(report -> lookup_brand (brand_id));
diesel::joinable!(report -> lookup_model (model_id));
diesel::joinable!(report -> lookup_chipset (chipset_id));
diesel::joinable!(report -> lookup_device_specs (device_specs_id));
diesel::joinable!(report -> lookup_locale_code (locale_code_id));
diesel::joinable!(report -> lookup_timezone (timezone_id));
diesel::joinable!(report -> lookup_connection_type (connection_type_id));
diesel::joinable!(report -> lookup_orientation (orientation_id));
diesel::joinable!(report -> lookup_app_name (app_name_id));
diesel::joinable!(report -> lookup_app_version (app_version_id));
diesel::joinable!(report -> lookup_app_build (app_build_id));
diesel::joinable!(report -> lookup_user (user_id));
diesel::joinable!(report -> lookup_exception_type (exception_type_id));
diesel::joinable!(report -> lookup_exception_message (exception_message_id));
diesel::joinable!(report -> lookup_stacktrace (stacktrace_id));
diesel::joinable!(report -> issue (issue_id));
diesel::joinable!(issue -> lookup_exception_type (exception_type_id));

diesel::allow_tables_to_appear_in_same_query!(
    project,
    archive,
    queue,
    queue_error,
    lookup_platform,
    lookup_environment,
    lookup_connection_type,
    lookup_orientation,
    lookup_os_name,
    lookup_os_version,
    lookup_manufacturer,
    lookup_brand,
    lookup_model,
    lookup_chipset,
    lookup_locale_code,
    lookup_timezone,
    lookup_app_name,
    lookup_app_version,
    lookup_app_build,
    lookup_user,
    lookup_exception_type,
    lookup_device_specs,
    lookup_exception_message,
    lookup_stacktrace,
    issue,
    report,
);
