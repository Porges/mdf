use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Gender {
    #[serde(rename = "type", with = "http_serde::uri")]
    pub type_uri: http::Uri,
}

#[derive(Serialize, Deserialize)]
pub struct Date {
    pub original: String,
    // pub formal: crate::date::v1::Date,
}

#[derive(Serialize, Deserialize)]
pub struct Name {
    #[serde(rename = "type", with = "http_serde::uri")]
    type_uri: http::Uri,
}

#[derive(Serialize, Deserialize)]
pub struct Person {
    pub private: bool,
    pub gender: Gender,
    pub names: Vec<Name>,
    // pub facts: Vec<Fact>,
}
