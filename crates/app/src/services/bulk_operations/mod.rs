pub mod cancel;
pub mod client;
pub mod download;
pub mod execute;
pub mod stage_file;
pub mod status;
pub mod watch;

pub use cancel::cancel_bulk_operation;
pub use client::{
    extract_created_operation_id, extract_list_nodes, extract_operation_node, graphql_user_errors,
    is_graphql_mutation, list_created_at_filter, BulkAdminClient, HttpBulkAdminClient,
    MockBulkAdminClient,
};
pub use download::{download_bulk_operation_results, results_contain_user_errors};
pub use execute::{execute_bulk_operation, ExecuteBulkOptions, ExecuteBulkResult};
pub use stage_file::{
    resolve_mutation_jsonl, staged_upload_path_from_response, upload_staged_jsonl,
};
pub use status::{
    extract_bulk_operation_id, format_bulk_operation_cancellation_result,
    format_bulk_operation_list_row, format_bulk_operation_status, format_bulk_operation_user_errors,
    get_bulk_operation_status, list_bulk_operations, normalize_bulk_operation_id,
    parse_bulk_operation_status, BulkOperationCancellationResult, BulkOperationStatus,
};
pub use watch::{
    short_bulk_operation_poll, watch_bulk_operation, WatchOptions, QUICK_WATCH_POLL_INTERVAL,
    QUICK_WATCH_TIMEOUT,
};

pub const BULK_OPERATIONS_MIN_API_VERSION: &str = "2026-01";
