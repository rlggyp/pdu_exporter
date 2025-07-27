pub mod client;
pub mod handler;
pub mod metrics;

const RAW_DATA_LENGTH: usize = 2016;
const METRIC_STEP: usize = 63;
const TEMP_INDEX_OFFSET: usize = 15;
