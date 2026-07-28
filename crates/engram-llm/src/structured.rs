use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StructuredExtraction<T> {
    pub model: String,
    pub data: T,
}
