pub mod cancel;
pub mod execute;
pub mod stage_file;
pub mod status;
pub mod watch;

pub use cancel::cancel_bulk_operation;
pub use execute::{execute_bulk_operation, ExecuteBulkOptions};
pub use stage_file::{
    resolve_mutation_jsonl, staged_upload_path_from_response, upload_staged_jsonl,
};
pub use status::{
    extract_bulk_operation_id, format_bulk_operation_cancellation_result,
    format_bulk_operation_status, format_bulk_operation_user_errors, get_bulk_operation_status,
    list_bulk_operations, normalize_bulk_operation_id, parse_bulk_operation_status,
    BulkOperationCancellationResult, BulkOperationStatus,
};
pub use watch::watch_bulk_operation;

pub const BULK_OPERATIONS_MIN_API_VERSION: &str = "2026-01";
