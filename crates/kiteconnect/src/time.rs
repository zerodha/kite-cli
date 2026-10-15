use chrono::{DateTime, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Deserializer, Serializer, de::Error};

const DATETIME: &str = "%Y-%m-%d %H:%M:%S%.f";
const DATE: &str = "%Y-%m-%d";

fn parse_datetime(value: &str) -> Result<NaiveDateTime, chrono::ParseError> {
    for format in [DATETIME, "%Y-%m-%dT%H:%M:%S%.f"] {
        if let Ok(value) = NaiveDateTime::parse_from_str(value, format) {
            return Ok(value);
        }
    }
    if let Ok(value) = NaiveDate::parse_from_str(value, DATE) {
        return Ok(value.and_time(chrono::NaiveTime::MIN));
    }
    DateTime::parse_from_rfc3339(value)
        .or_else(|_| DateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f%z"))
        .map(|value| value.naive_local())
}

pub(crate) mod datetime {
    use super::{
        DATETIME, Deserialize, Deserializer, Error, NaiveDateTime, Serializer, parse_datetime,
    };

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<NaiveDateTime, D::Error> {
        parse_datetime(&String::deserialize(deserializer)?).map_err(D::Error::custom)
    }

    pub(crate) fn serialize<S: Serializer>(
        value: &NaiveDateTime,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&value.format(DATETIME))
    }
}

pub(crate) mod optional_datetime {
    use super::{
        DATETIME, Deserialize, Deserializer, Error, NaiveDateTime, Serializer, parse_datetime,
    };

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<NaiveDateTime>, D::Error> {
        Option::<String>::deserialize(deserializer)?
            .filter(|value| !value.is_empty())
            .map(|value| parse_datetime(&value).map_err(D::Error::custom))
            .transpose()
    }

    #[expect(
        clippy::ref_option,
        reason = "serde serialize_with requires a reference to the field type"
    )]
    pub(crate) fn serialize<S: Serializer>(
        value: &Option<NaiveDateTime>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer.serialize_some(&value.format(DATETIME).to_string()),
            None => serializer.serialize_none(),
        }
    }
}

pub(crate) mod date {
    use super::{DATE, Deserialize, Deserializer, Error, NaiveDate, Serializer};

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<NaiveDate, D::Error> {
        NaiveDate::parse_from_str(&String::deserialize(deserializer)?, DATE)
            .map_err(D::Error::custom)
    }

    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde serialize_with requires a reference to the field type"
    )]
    pub(crate) fn serialize<S: Serializer>(
        value: &NaiveDate,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&value.format(DATE))
    }
}

pub(crate) mod optional_date {
    use super::{DATE, Deserialize, Deserializer, Error, NaiveDate, Serializer};

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<NaiveDate>, D::Error> {
        Option::<String>::deserialize(deserializer)?
            .filter(|value| !value.is_empty())
            .map(|value| NaiveDate::parse_from_str(&value, DATE).map_err(D::Error::custom))
            .transpose()
    }

    #[expect(
        clippy::ref_option,
        clippy::trivially_copy_pass_by_ref,
        reason = "serde serialize_with requires a reference to the field type"
    )]
    pub(crate) fn serialize<S: Serializer>(
        value: &Option<NaiveDate>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer.serialize_some(&value.format(DATE).to_string()),
            None => serializer.serialize_none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use serde_json::{Value, json};

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Times {
        #[serde(with = "datetime")]
        timestamp: NaiveDateTime,
        #[serde(with = "optional_datetime")]
        optional_timestamp: Option<NaiveDateTime>,
        #[serde(with = "date")]
        date: NaiveDate,
        #[serde(with = "optional_date")]
        optional_date: Option<NaiveDate>,
    }

    fn payload() -> Value {
        json!({
            "timestamp": "2021-05-31 09:16:39",
            "optional_timestamp": "2021-07-05T10:41:27+05:30",
            "date": "2021-05-12",
            "optional_date": "2018-01-25"
        })
    }

    #[test]
    fn fixture_dates_decode_and_round_trip() {
        let trades: Value =
            serde_json::from_str(include_str!("../../../mock_responses/trades.json")).unwrap();
        let ticker: Value =
            serde_json::from_str(include_str!("../../../mock_responses/ticker_full.json")).unwrap();
        let sips: Value =
            serde_json::from_str(include_str!("../../../mock_responses/mf_sips.json")).unwrap();
        assert!(
            include_str!("../../../mock_responses/instruments_all.csv").contains(",2018-01-25,")
        );
        let mut input = payload();
        input["timestamp"] = trades["data"][0]["fill_timestamp"].clone();
        input["optional_timestamp"] = ticker["timestamp"].clone();
        input["date"] = sips["data"][0]["next_instalment"].clone();
        let value: Times = serde_json::from_value(input).unwrap();
        assert_eq!(
            value.timestamp,
            NaiveDate::from_ymd_opt(2021, 5, 31)
                .unwrap()
                .and_hms_opt(9, 16, 39)
                .unwrap()
        );
        assert_eq!(
            value.optional_timestamp,
            Some(
                NaiveDate::from_ymd_opt(2021, 7, 5)
                    .unwrap()
                    .and_hms_opt(10, 41, 27)
                    .unwrap()
            )
        );
        assert_eq!(value.date, NaiveDate::from_ymd_opt(2021, 5, 12).unwrap());
        assert_eq!(value.optional_date, NaiveDate::from_ymd_opt(2018, 1, 25));
        let serialized = serde_json::to_value(&value).unwrap();
        assert_eq!(serialized["timestamp"], "2021-05-31 09:16:39");
        assert_eq!(serialized["optional_timestamp"], "2021-07-05 10:41:27");
        assert_eq!(serialized["date"], "2021-05-12");
        assert_eq!(serialized["optional_date"], "2018-01-25");
        assert_eq!(serde_json::from_value::<Times>(serialized).unwrap(), value);
    }

    #[test]
    fn optional_dates_accept_empty_and_null() {
        for empty in [json!(""), Value::Null] {
            let mut input = payload();
            input["optional_timestamp"] = empty.clone();
            input["optional_date"] = empty;
            let value: Times = serde_json::from_value(input).unwrap();
            assert_eq!(value.optional_timestamp, None);
            assert_eq!(value.optional_date, None);
            let serialized = serde_json::to_value(value).unwrap();
            assert_eq!(serialized["optional_timestamp"], Value::Null);
            assert_eq!(serialized["optional_date"], Value::Null);
        }
    }

    #[test]
    fn invalid_and_required_empty_dates_fail() {
        for field in ["timestamp", "optional_timestamp", "date", "optional_date"] {
            for invalid in [json!("not-a-date"), json!("2021-02-30"), json!(42)] {
                let mut input = payload();
                input[field] = invalid;
                assert!(serde_json::from_value::<Times>(input).is_err());
            }
        }
        for field in ["timestamp", "date"] {
            for empty in [json!(""), Value::Null] {
                let mut input = payload();
                input[field] = empty;
                assert!(serde_json::from_value::<Times>(input).is_err());
            }
        }
    }

    #[test]
    fn datetime_accepts_go_iso_formats_and_date_only() {
        for timestamp in [
            "2021-07-05T10:41:27+0530",
            "2021-07-05T10:41:27Z",
            "2021-07-05T10:41:27",
        ] {
            let mut input = payload();
            input["timestamp"] = json!(timestamp);
            assert_eq!(
                serde_json::from_value::<Times>(input).unwrap().timestamp,
                NaiveDate::from_ymd_opt(2021, 7, 5)
                    .unwrap()
                    .and_hms_opt(10, 41, 27)
                    .unwrap()
            );
        }
        let mut input = payload();
        input["timestamp"] = json!("2021-05-12");
        assert_eq!(
            serde_json::from_value::<Times>(input).unwrap().timestamp,
            NaiveDate::from_ymd_opt(2021, 5, 12)
                .unwrap()
                .and_time(chrono::NaiveTime::MIN)
        );
    }
}
