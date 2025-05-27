//! A lightweight client for querying [sunxdcc.com](https://sunxdcc.com) and parsing XDCC bot listings.
//!
//! This crate provides an asynchronous `Engine` to search for XDCC pack listings,
//! returning decoded metadata as structured `Entry` items.
//!
//! # Example
//!
//! ```no_run
//! # use xdcc_search::sunxdcc::{Engine, Entry};
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let engine = Engine::default();
//! let results: Vec<Entry> = engine.search("ubuntu", 1).await?;
//! for entry in results {
//!     println!("Found pack: {} ({} bytes)", entry.filename, entry.filesize);
//! }
//! # Ok(())
//! # }
//! ```

use std::borrow::Cow;
use std::num::{ParseFloatError, ParseIntError};
use std::sync::Arc;

#[derive(Debug)]
struct InnerEngine {
    client: reqwest::Client,
    url: Cow<'static, str>,
}

impl Default for InnerEngine {
    fn default() -> Self {
        Self {
            client: reqwest::Client::default(),
            url: Cow::Borrowed("https://sunxdcc.com/deliver.php"),
        }
    }
}

#[derive(Debug, serde::Serialize)]
struct QueryParams<'a> {
    sterm: &'a str,
    page: u8,
}

/// The main entry point for querying the XDCC engine.
///
/// `Engine` is a lightweight, cloneable wrapper around an internal HTTP client.
/// It provides a `search` method that sends a request to the XDCC listing service
/// and returns a parsed list of results.
#[derive(Clone, Debug, Default)]
pub struct Engine(Arc<InnerEngine>);

impl Engine {
    /// Queries the XDCC engine for packs matching the given search term and page number.
    ///
    /// # Arguments
    ///
    /// * `query` - The search term (e.g., a keyword or filename).
    /// * `page` - The page number to fetch (starting from 1).
    ///
    /// # Returns
    ///
    /// A `Vec<Entry>` containing the parsed pack information.
    ///
    /// # Errors
    ///
    /// Returns a `reqwest::Error` if the request fails or the response is malformed.
    pub async fn search(&self, query: &str, page: u8) -> reqwest::Result<Vec<Entry>> {
        let res = self
            .0
            .client
            .get(self.0.url.as_ref())
            .query(&QueryParams { sterm: query, page })
            .send()
            .await?;
        res.error_for_status_ref()?;
        let body: Response = res.json().await?;
        Ok(body.into())
    }
}

#[derive(Debug, serde::Deserialize)]
struct Response {
    botrec: Vec<String>,
    network: Vec<String>,
    bot: Vec<String>,
    channel: Vec<String>,
    packnum: Vec<String>,
    gets: Vec<String>,
    fsize: Vec<String>,
    fname: Vec<String>,
}

impl Response {
    fn into(self) -> Vec<Entry> {
        self.fname
            .into_iter()
            .zip(self.fsize)
            .zip(self.gets)
            .zip(self.packnum)
            .zip(self.channel)
            .zip(self.network)
            .zip(self.bot)
            .zip(self.botrec)
            .enumerate()
            .filter_map(
                |(
                    index,
                    (
                        ((((((fname, fsize), downloads), packnum), channel), network), bot_name),
                        bot_speed,
                    ),
                )| {
                    Entry::try_decode(
                        fname, fsize, downloads, packnum, channel, network, bot_name, bot_speed,
                    )
                    .inspect_err(|err| {
                        tracing::debug!("unable to decode entry {index}: {err:?}");
                    })
                    .ok()
                },
            )
            .collect::<Vec<_>>()
    }
}

/// A single XDCC listing entry returned from the search.
///
/// Contains all relevant metadata parsed from the server response.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize, serde::Serialize)]
pub struct Entry {
    /// The name of the file being shared.
    pub filename: String,
    /// The size of the file in bytes.
    pub filesize: u64,
    /// Number of times the pack has been downloaded.
    pub downloads: u64,
    /// The XDCC pack number (used to request the pack).
    pub packnum: u64,
    /// The IRC channel where the bot is located.
    pub channel: String,
    /// The IRC network hosting the bot.
    pub network: String,
    /// The name of the bot sharing the file.
    pub bot_name: String,
    /// The reported upload speed of the bot, in bytes per second.
    pub bot_speed: u64,
}

impl Entry {
    /// Attempts to decode a set of string values from the server into a structured `Entry`.
    ///
    /// Each field is parsed individually, with validation and conversion applied.
    ///
    /// # Errors
    ///
    /// Returns `DecodingError` if any field fails to parse or is malformed.
    #[allow(clippy::too_many_arguments)]
    fn try_decode(
        fname: String,
        fsize: String,
        downloads: String,
        packnum: String,
        channel: String,
        network: String,
        bot_name: String,
        bot_speed: String,
    ) -> Result<Self, DecodingError> {
        Ok(Self {
            filename: fname,
            filesize: decode_filesize(fsize)?,
            downloads: decode_downloads(downloads)?,
            packnum: decode_packnum(packnum)?,
            channel,
            network,
            bot_name,
            bot_speed: decode_speed(bot_speed)?,
        })
    }
}

/// Represents an error that occurred while parsing or decoding a field from the response.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum DecodingError {
    /// Field had an invalid format (e.g., missing prefix or suffix).
    #[error("invalid {field:?} format, expected {expected:?}, received {value:?}")]
    InvalidFormat {
        field: &'static str,
        value: String,
        expected: &'static str,
    },
    /// Field could not be parsed as a float.
    #[error("invalid number in field {field:?}, expected a float, received {value:?}")]
    InvalidFloat {
        field: &'static str,
        value: String,
        error: ParseFloatError,
    },
    /// Field could not be parsed as an integer.
    #[error("invalid number in field {field:?}, expected a int, received {value:?}")]
    InvalidInt {
        field: &'static str,
        value: String,
        error: ParseIntError,
    },
}

const FILESIZE_FIELD: &str = "filesize";
const FILESIZE_FORMAT: &str = "[1.1M]";

fn decode_filesize(value: String) -> Result<u64, DecodingError> {
    let Some(stripped) = value
        .as_str()
        .strip_prefix("[")
        .and_then(|v| v.strip_suffix("]"))
    else {
        return Err(DecodingError::InvalidFormat {
            field: FILESIZE_FIELD,
            value,
            expected: FILESIZE_FORMAT,
        });
    };
    let Some(last_char) = stripped.chars().last() else {
        return Err(DecodingError::InvalidFormat {
            field: FILESIZE_FIELD,
            value,
            expected: FILESIZE_FORMAT,
        });
    };
    let factor = match last_char.to_ascii_lowercase() {
        'k' => 1024.0,
        'm' => 1024.0 * 1024.0,
        'g' => 1024.0 * 1024.0 * 1024.0,
        't' => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        'p' => 1024.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0,
        '0'..='9' => 1.0,
        _ => {
            return Err(DecodingError::InvalidFormat {
                field: FILESIZE_FIELD,
                value,
                expected: FILESIZE_FORMAT,
            });
        }
    };
    let number = if last_char.is_numeric() {
        stripped
    } else {
        &stripped[..stripped.len() - 1]
    };
    let number = number
        .trim()
        .parse::<f64>()
        .map_err(|error| DecodingError::InvalidFloat {
            field: FILESIZE_FIELD,
            value,
            error,
        })?;
    let number = (number * factor) as u64;
    Ok(number)
}

const GETS_FIELD: &str = "gets";
const GETS_FORMAT: &str = "42x";

fn decode_downloads(value: String) -> Result<u64, DecodingError> {
    let Some(stripped) = value.strip_suffix('x') else {
        return Err(DecodingError::InvalidFormat {
            field: GETS_FIELD,
            value,
            expected: GETS_FORMAT,
        });
    };
    stripped
        .parse::<u64>()
        .map_err(|error| DecodingError::InvalidInt {
            field: GETS_FIELD,
            value,
            error,
        })
}

const SPEED_FIELD: &str = "botrec";
const SPEED_FORMAT: &str = "123.4kB/s";

fn decode_speed(value: String) -> Result<u64, DecodingError> {
    let number_size = value
        .chars()
        .take_while(|c| c.is_numeric() || *c == '.')
        .count();
    let Some((number, unit)) = value.split_at_checked(number_size) else {
        return Err(DecodingError::InvalidFormat {
            field: SPEED_FIELD,
            value,
            expected: SPEED_FORMAT,
        });
    };
    let factor = match unit {
        "B/s" => 1.0,
        "kB/s" => 1024.0,
        "MB/s" => 1024.0 * 1024.0,
        "GB/s" => 1024.0 * 1024.0 * 1024.0,
        "TB/s" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        "PB/s" => 1024.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => {
            return Err(DecodingError::InvalidFormat {
                field: SPEED_FIELD,
                value,
                expected: SPEED_FORMAT,
            });
        }
    };
    let number = number
        .parse::<f64>()
        .map_err(|error| DecodingError::InvalidFloat {
            field: SPEED_FIELD,
            value,
            error,
        })?;
    Ok((number * factor) as u64)
}

const PACKNUM_FIELD: &str = "packnum";
const PACKNUM_FORMAT: &str = "#42";

fn decode_packnum(value: String) -> Result<u64, DecodingError> {
    let Some(number) = value.strip_prefix("#") else {
        return Err(DecodingError::InvalidFormat {
            field: PACKNUM_FIELD,
            value,
            expected: PACKNUM_FORMAT,
        });
    };
    number
        .parse::<u64>()
        .map_err(|error| DecodingError::InvalidInt {
            field: PACKNUM_FIELD,
            value,
            error,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_search_for_ubuntu() {
        let mut src = mockito::Server::new_async().await;
        let engine = Engine(Arc::new(InnerEngine {
            client: Default::default(),
            url: Cow::Owned(format!("{}/deliver.php", src.url())),
        }));
        let mock = src
            .mock("GET", "/deliver.php?sterm=ubuntu&page=0")
            .expect(1)
            .with_body(include_str!("../resources/ubuntu.json"))
            .create_async()
            .await;
        let list = engine.search("ubuntu", 0).await.unwrap();
        assert_eq!(list.len(), 38);
        assert!(list[0].filename.contains("Ubuntu"));
        assert_eq!(list[0].filesize, 1503238553);
        mock.assert_async().await;
    }

    #[test_case::test_case("[ 112]", 112; "without letter")]
    #[test_case::test_case("[  1k]", 1024; "simple kilo with dot")]
    #[test_case::test_case("[  1M]", 1024 * 1024; "simple mega without dot")]
    #[test_case::test_case("[1.2M]", 1258291; "simple mega with dot")]
    #[test_case::test_case("[1.2G]", 1288490188; "simple giga with dot")]
    #[test_case::test_case("[1.2T]", 1319413953331; "simple tera with dot")]
    fn should_decode_filesize(input: &str, expected: u64) {
        assert_eq!(decode_filesize(input.into()).unwrap(), expected);
    }

    #[test_case::test_case("[ 12R]"; "invalid factor")]
    fn shouldnt_decode_filesize(input: &str) {
        assert!(decode_filesize(input.into()).is_err());
    }

    #[test_case::test_case("0x", 0; "zero")]
    #[test_case::test_case("42x", 42; "2 digits")]
    fn should_decode_downloads(input: &str, expected: u64) {
        assert_eq!(decode_downloads(input.into()).unwrap(), expected);
    }

    #[test_case::test_case("12B/s", 12; "B/s")]
    #[test_case::test_case("114012.3kB/s", 116748595; "kB/s")]
    fn should_decode_speed(input: &str, expected: u64) {
        assert_eq!(decode_speed(input.into()).unwrap(), expected);
    }

    #[test_case::test_case("#1", 1; "single digit")]
    #[test_case::test_case("#1234", 1234; "multiple digits")]
    fn should_decode_packnum(input: &str, expected: u64) {
        assert_eq!(decode_packnum(input.into()).unwrap(), expected);
    }
}
