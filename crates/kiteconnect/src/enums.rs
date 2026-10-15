use serde::{Deserialize, Serialize};
use std::fmt;

/// Exchange for Kite order and market endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "UPPERCASE")]
pub enum Exchange {
    /// `NSE` exchange.
    Nse,
    /// `BSE` exchange.
    Bse,
    /// `MCX` exchange.
    Mcx,
    /// `NFO` exchange.
    Nfo,
    /// `BFO` exchange.
    Bfo,
    /// `CDS` exchange.
    Cds,
    /// `BCD` exchange.
    Bcd,
    /// `NCO` exchange.
    Nco,
}

impl fmt::Display for Exchange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Nse => "NSE",
            Self::Bse => "BSE",
            Self::Mcx => "MCX",
            Self::Nfo => "NFO",
            Self::Bfo => "BFO",
            Self::Cds => "CDS",
            Self::Bcd => "BCD",
            Self::Nco => "NCO",
        })
    }
}

/// Transaction side for Kite order endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "UPPERCASE")]
pub enum TransactionType {
    /// `BUY` transaction.
    Buy,
    /// `SELL` transaction.
    Sell,
}

impl fmt::Display for TransactionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
        })
    }
}

/// Product for Kite order and portfolio endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "UPPERCASE")]
pub enum Product {
    /// `BO` (bracket order) product.
    Bo,
    /// `CO` (cover order) product.
    Co,
    /// `MIS` (intraday) product.
    Mis,
    /// `CNC` (delivery) product.
    Cnc,
    /// `NRML` (normal margin) product.
    Nrml,
    /// `MTF` (margin trading facility) product.
    Mtf,
}

impl fmt::Display for Product {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Bo => "BO",
            Self::Co => "CO",
            Self::Mis => "MIS",
            Self::Cnc => "CNC",
            Self::Nrml => "NRML",
            Self::Mtf => "MTF",
        })
    }
}

/// Order type for Kite order endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "SCREAMING-KEBAB-CASE")]
pub enum OrderType {
    /// `MARKET` order type.
    Market,
    /// `LIMIT` order type.
    Limit,
    /// `SL` (stop-loss limit) order type.
    Sl,
    /// `SL-M` (stop-loss market) order type.
    SlM,
}

impl fmt::Display for OrderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Market => "MARKET",
            Self::Limit => "LIMIT",
            Self::Sl => "SL",
            Self::SlM => "SL-M",
        })
    }
}

/// Order variety for Kite order endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "lowercase")]
pub enum Variety {
    /// `regular` variety.
    Regular,
    /// `amo` (after market order) variety.
    Amo,
    /// `bo` (bracket order) variety.
    Bo,
    /// `co` (cover order) variety.
    Co,
    /// `iceberg` variety.
    Iceberg,
    /// `auction` variety.
    Auction,
}

impl fmt::Display for Variety {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Regular => "regular",
            Self::Amo => "amo",
            Self::Bo => "bo",
            Self::Co => "co",
            Self::Iceberg => "iceberg",
            Self::Auction => "auction",
        })
    }
}

/// Validity for Kite order endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "UPPERCASE")]
pub enum Validity {
    /// `DAY` validity.
    Day,
    /// `IOC` (immediate or cancel) validity.
    Ioc,
    /// `TTL` (time to live) validity.
    Ttl,
}

impl fmt::Display for Validity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Day => "DAY",
            Self::Ioc => "IOC",
            Self::Ttl => "TTL",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exchange_wire_values() {
        for (value, wire) in [
            (Exchange::Nse, "NSE"),
            (Exchange::Bse, "BSE"),
            (Exchange::Mcx, "MCX"),
            (Exchange::Nfo, "NFO"),
            (Exchange::Bfo, "BFO"),
            (Exchange::Cds, "CDS"),
            (Exchange::Bcd, "BCD"),
            (Exchange::Nco, "NCO"),
        ] {
            assert_eq!(value.to_string(), wire);
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
            assert_eq!(
                serde_json::from_value::<Exchange>(serde_json::json!(wire)).unwrap(),
                value
            );
        }
        assert!(serde_json::from_str::<Exchange>(r#""unknown""#).is_err());
    }

    #[test]
    fn transactiontype_wire_values() {
        for (value, wire) in [
            (TransactionType::Buy, "BUY"),
            (TransactionType::Sell, "SELL"),
        ] {
            assert_eq!(value.to_string(), wire);
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
            assert_eq!(
                serde_json::from_value::<TransactionType>(serde_json::json!(wire)).unwrap(),
                value
            );
        }
        assert!(serde_json::from_str::<TransactionType>(r#""unknown""#).is_err());
    }

    #[test]
    fn product_wire_values() {
        for (value, wire) in [
            (Product::Bo, "BO"),
            (Product::Co, "CO"),
            (Product::Mis, "MIS"),
            (Product::Cnc, "CNC"),
            (Product::Nrml, "NRML"),
            (Product::Mtf, "MTF"),
        ] {
            assert_eq!(value.to_string(), wire);
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
            assert_eq!(
                serde_json::from_value::<Product>(serde_json::json!(wire)).unwrap(),
                value
            );
        }
        assert!(serde_json::from_str::<Product>(r#""unknown""#).is_err());
    }

    #[test]
    fn ordertype_wire_values() {
        for (value, wire) in [
            (OrderType::Market, "MARKET"),
            (OrderType::Limit, "LIMIT"),
            (OrderType::Sl, "SL"),
            (OrderType::SlM, "SL-M"),
        ] {
            assert_eq!(value.to_string(), wire);
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
            assert_eq!(
                serde_json::from_value::<OrderType>(serde_json::json!(wire)).unwrap(),
                value
            );
        }
        assert!(serde_json::from_str::<OrderType>(r#""unknown""#).is_err());
    }

    #[test]
    fn variety_wire_values() {
        for (value, wire) in [
            (Variety::Regular, "regular"),
            (Variety::Amo, "amo"),
            (Variety::Bo, "bo"),
            (Variety::Co, "co"),
            (Variety::Iceberg, "iceberg"),
            (Variety::Auction, "auction"),
        ] {
            assert_eq!(value.to_string(), wire);
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
            assert_eq!(
                serde_json::from_value::<Variety>(serde_json::json!(wire)).unwrap(),
                value
            );
        }
        assert!(serde_json::from_str::<Variety>(r#""unknown""#).is_err());
    }

    #[test]
    fn validity_wire_values() {
        for (value, wire) in [
            (Validity::Day, "DAY"),
            (Validity::Ioc, "IOC"),
            (Validity::Ttl, "TTL"),
        ] {
            assert_eq!(value.to_string(), wire);
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
            assert_eq!(
                serde_json::from_value::<Validity>(serde_json::json!(wire)).unwrap(),
                value
            );
        }
        assert!(serde_json::from_str::<Validity>(r#""unknown""#).is_err());
    }
}
