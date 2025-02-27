#[cfg(feature = "io_print")]
mod print;

#[cfg(feature = "io_json")]
mod json;

#[cfg(feature = "io_json")]
mod ndjson;

#[cfg(feature = "io_ipc")]
mod ipc;

#[cfg(feature = "io_parquet")]
mod parquet;

#[cfg(feature = "io_avro")]
mod avro;

#[cfg(any(
    feature = "io_csv_read",
    feature = "io_csv_write",
    feature = "io_csv_read_async"
))]
mod csv;
