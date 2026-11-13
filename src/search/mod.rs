pub mod controller;
pub mod index;
pub mod query;
pub mod result;
pub mod scanner;
pub mod ui;
pub mod worker;

pub use controller::SearchController;
pub use index::SearchIndex;
pub use query::{QueryError, SearchQuery};
pub use result::{FileSearchResult, MatchLocation};
pub use scanner::VaultScanner;
pub use ui::SearchUi;
pub use worker::SearchWorker;

#[cfg(test)]
mod tests;
