//! Derived metrics over page-turn events.
//!
//! Every function here is pure (no database access): the `db` layer
//! produces event slices, this layer turns them into the numbers
//! `spec.md` defines. Timezone-sensitive functions are generic over
//! `chrono::TimeZone`; pass `chrono::Local` in the app, a fixed offset in
//! tests.

pub mod completion;
pub mod days;
pub mod progress;
pub mod sessions;
pub mod speed;

pub use completion::{CompletionConfig, completions};
pub use days::{daily_totals, local_date, streaks};
pub use progress::{
    avg_seconds_per_page, capped_seconds, coverage, uncapped_seconds, unique_pages_read,
};
pub use sessions::sessions;
pub use speed::{Bucket, speed_series};
