use super::{DepthEntry, MarketDepth, Tick};

// --- binary protocol -------------------------------------------------------
// Frame: u16 packet count, then per packet a u16 length + payload.
// Payload sizes: 8 = ltp, 28/32 = index quote/full, 44 = quote, 184 = full.
// Prices are integers scaled by 100 (10^7 for CDS, 10^4 for BCD and NCO).

const SEGMENT_CDS: u32 = 3;
const SEGMENT_BCD: u32 = 6;
const SEGMENT_INDICES: u32 = 9;
const SEGMENT_NCO: u32 = 12;

/// Decode a Kite ticker binary frame.
#[must_use]
pub fn parse_binary(data: &[u8]) -> Vec<Tick> {
    if data.len() < 2 {
        return Vec::new();
    }

    let mut ticks = Vec::new();
    let mut offset = 0usize;
    let packet_count = be_u16(data, offset) as usize;
    offset += 2;

    for _ in 0..packet_count {
        if offset + 2 > data.len() {
            break;
        }
        let packet_len = be_u16(data, offset) as usize;
        offset += 2;
        if offset + packet_len > data.len() {
            break;
        }
        let packet = &data[offset..offset + packet_len];
        offset += packet_len;
        if packet_len < 8 {
            continue;
        }
        ticks.push(parse_packet(packet));
    }

    ticks
}

fn parse_packet(packet: &[u8]) -> Tick {
    let instrument_token = be_u32(packet, 0);
    let segment = instrument_token & 0xFF;
    let divisor = match segment {
        SEGMENT_CDS => 10_000_000.0,
        SEGMENT_BCD | SEGMENT_NCO => 10_000.0,
        _ => 100.0,
    };
    let price = |at: usize| f64::from(be_u32(packet, at)) / divisor;

    let mut tick = Tick {
        instrument_token,
        is_index: segment == SEGMENT_INDICES,
        last_price: Some(price(4)),
        ..Tick::default()
    };

    if packet.len() == 8 {
        return tick;
    }

    if tick.is_index {
        if packet.len() < 28 {
            return tick;
        }
        tick.high = Some(price(8));
        tick.low = Some(price(12));
        tick.open = Some(price(16));
        tick.close = Some(price(20));
        if packet.len() >= 32 {
            tick.exchange_time = Some(be_u32(packet, 28));
        }
        return tick;
    }

    if packet.len() < 44 {
        return tick;
    }

    tick.last_quantity = Some(be_u32(packet, 8));
    tick.average_price = Some(price(12));
    tick.volume = Some(u64::from(be_u32(packet, 16)));
    tick.buy_quantity = Some(u64::from(be_u32(packet, 20)));
    tick.sell_quantity = Some(u64::from(be_u32(packet, 24)));
    tick.open = Some(price(28));
    tick.high = Some(price(32));
    tick.low = Some(price(36));
    tick.close = Some(price(40));

    if packet.len() >= 64 {
        tick.last_trade_time = Some(be_u32(packet, 44));
        tick.oi = Some(u64::from(be_u32(packet, 48)));
        tick.oi_day_high = Some(u64::from(be_u32(packet, 52)));
        tick.oi_day_low = Some(u64::from(be_u32(packet, 56)));
        tick.exchange_time = Some(be_u32(packet, 60));
    }

    if packet.len() >= 184 {
        let level = |level: usize| {
            let base = 64 + level * 12;
            DepthEntry {
                quantity: be_u32(packet, base),
                price: price(base + 4),
                orders: be_u16(packet, base + 8),
            }
        };
        tick.depth = Some(MarketDepth {
            buy: (0..5).map(level).collect(),
            sell: (5..10).map(level).collect(),
        });
    }

    tick
}

fn be_u16(bytes: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([bytes[at], bytes[at + 1]])
}

fn be_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

#[cfg(test)]
mod tests {
    use super::parse_binary;
    #[test]
    fn tick_json_preserves_the_cli_stream_output() {
        let ticks = parse_binary(&frame(&[&ltp_packet(408_065, 157_315)]));
        assert_eq!(
            serde_json::to_string(&ticks[0]).unwrap(),
            r#"{"instrument_token":408065,"is_index":false,"last_price":1573.15,"last_quantity":null,"average_price":null,"volume":null,"buy_quantity":null,"sell_quantity":null,"open":null,"high":null,"low":null,"close":null,"oi":null,"oi_day_high":null,"oi_day_low":null,"last_trade_time":null,"exchange_time":null,"depth":null}"#
        );
    }

    #[test]
    fn parses_index_packet() {
        let mut packet = Vec::new();
        packet.extend_from_slice(&256_265u32.to_be_bytes());
        packet.extend_from_slice(&2_431_240u32.to_be_bytes()); // ltp 24312.40
        packet.extend_from_slice(&2_440_000u32.to_be_bytes()); // high
        packet.extend_from_slice(&2_420_000u32.to_be_bytes()); // low
        packet.extend_from_slice(&2_425_000u32.to_be_bytes()); // open
        packet.extend_from_slice(&2_428_500u32.to_be_bytes()); // close
        packet.extend_from_slice(&0u32.to_be_bytes()); // change (unused)
        packet.extend_from_slice(&1_720_000_000u32.to_be_bytes()); // exchange time

        let mut data = vec![0, 1, 0, u8::try_from(packet.len()).unwrap()];
        data.extend_from_slice(&packet);
        let ticks = parse_binary(&data);
        assert_eq!(ticks.len(), 1);
        let tick = &ticks[0];
        assert!(tick.is_index);
        assert_eq!(tick.last_price, Some(24312.40));
        assert_eq!(tick.open, Some(24250.00));
        assert_eq!(tick.close, Some(24285.00));
        assert_eq!(tick.exchange_time, Some(1_720_000_000));
        assert!(tick.volume.is_none());
    }

    #[test]
    fn parses_full_packet_with_depth() {
        let mut packet = Vec::new();
        packet.extend_from_slice(&408_065u32.to_be_bytes()); // token
        packet.extend_from_slice(&152_340u32.to_be_bytes()); // ltp 1523.40
        packet.extend_from_slice(&10u32.to_be_bytes()); // last qty
        packet.extend_from_slice(&152_000u32.to_be_bytes()); // avg 1520.00
        packet.extend_from_slice(&1_000_000u32.to_be_bytes()); // volume
        packet.extend_from_slice(&500u32.to_be_bytes()); // buy qty
        packet.extend_from_slice(&600u32.to_be_bytes()); // sell qty
        packet.extend_from_slice(&151_000u32.to_be_bytes()); // open
        packet.extend_from_slice(&153_000u32.to_be_bytes()); // high
        packet.extend_from_slice(&150_235u32.to_be_bytes()); // low
        packet.extend_from_slice(&151_130u32.to_be_bytes()); // close
        packet.extend_from_slice(&1_720_000_100u32.to_be_bytes()); // last trade time
        packet.extend_from_slice(&0u32.to_be_bytes()); // oi
        packet.extend_from_slice(&0u32.to_be_bytes()); // oi high
        packet.extend_from_slice(&0u32.to_be_bytes()); // oi low
        packet.extend_from_slice(&1_720_000_200u32.to_be_bytes()); // exchange time
        for level in 0..10u32 {
            packet.extend_from_slice(&(100 + level).to_be_bytes()); // qty
            packet.extend_from_slice(&(152_300 + level).to_be_bytes()); // price
            packet.extend_from_slice(&[0, 3]); // orders
            packet.extend_from_slice(&[0, 0]); // padding
        }
        assert_eq!(packet.len(), 184);

        let mut data = vec![0, 1, 0, 184];
        data.extend_from_slice(&packet);
        let ticks = parse_binary(&data);
        assert_eq!(ticks.len(), 1);
        let tick = &ticks[0];
        assert_eq!(tick.last_price, Some(1523.40));
        assert_eq!(tick.volume, Some(1_000_000));
        assert_eq!(tick.buy_quantity, Some(500));
        assert_eq!(tick.exchange_time, Some(1_720_000_200));
        let depth = tick.depth.as_ref().unwrap();
        assert_eq!(depth.buy.len(), 5);
        assert_eq!(depth.sell.len(), 5);
        assert_eq!(depth.buy[0].quantity, 100);
        assert_eq!(depth.buy[0].orders, 3);
        assert_eq!(depth.sell[0].quantity, 105);
    }

    #[test]
    fn cds_prices_use_seven_decimal_divisor() {
        let token: u32 = (1 << 8) | 3;
        let mut data = vec![0, 1, 0, 8];
        data.extend_from_slice(&token.to_be_bytes());
        data.extend_from_slice(&845_525_000u32.to_be_bytes());
        let ticks = parse_binary(&data);
        assert_eq!(ticks[0].last_price, Some(84.5525));
    }

    #[test]
    fn nco_prices_use_four_decimal_divisor() {
        let token: u32 = (1 << 8) | 0x0C;
        let mut data = vec![0, 1, 0, 8];
        data.extend_from_slice(&token.to_be_bytes());
        data.extend_from_slice(&105_650_000u32.to_be_bytes());
        let ticks = parse_binary(&data);
        assert_eq!(ticks[0].last_price, Some(10_565.0));
    }

    fn frame(packets: &[&[u8]]) -> Vec<u8> {
        let mut data = u16::try_from(packets.len()).unwrap().to_be_bytes().to_vec();
        for packet in packets {
            data.extend_from_slice(&u16::try_from(packet.len()).unwrap().to_be_bytes());
            data.extend_from_slice(packet);
        }
        data
    }

    fn ltp_packet(token: u32, price: u32) -> Vec<u8> {
        [token.to_be_bytes(), price.to_be_bytes()].concat()
    }

    /// A 184-byte full packet whose every u32 field is distinct and whose
    /// depth order counts are 0x0102, so a byte-order slip changes values.
    fn full_packet(token: u32) -> Vec<u8> {
        let mut packet = token.to_be_bytes().to_vec();
        for field in 1..16u32 {
            packet.extend_from_slice(&(0x0100_0000 + field).to_be_bytes());
        }
        for level in 0..10u32 {
            packet.extend_from_slice(&(100 + level).to_be_bytes());
            packet.extend_from_slice(&(200 + level).to_be_bytes());
            packet.extend_from_slice(&[0x01, 0x02, 0, 0]);
        }
        packet
    }

    #[test]
    fn malformed_frames_keep_ticks_parsed_before_the_damage() {
        let first = ltp_packet(408_065, 1234);
        let second = ltp_packet(408_321, 5678);

        assert_eq!(parse_binary(&[]).len(), 0);
        assert_eq!(parse_binary(&[0]).len(), 0);
        assert!(parse_binary(&[0, 1]).is_empty(), "count without a length");
        assert!(parse_binary(&[0, 1, 0]).is_empty(), "half a length field");
        assert!(
            parse_binary(&[0, 1, 0, 8, 0, 0, 0]).is_empty(),
            "payload shorter than its declared length"
        );

        let mut truncated_second = frame(&[&first, &second]);
        truncated_second.truncate(truncated_second.len() - 1);
        let ticks = parse_binary(&truncated_second);
        assert_eq!(ticks.len(), 1);
        assert_eq!(ticks[0].instrument_token, 408_065);
        assert_eq!(ticks[0].last_price, Some(12.34));

        let mut missing_second_length = frame(&[&first]);
        missing_second_length[1] = 2;
        missing_second_length.push(0);
        assert_eq!(parse_binary(&missing_second_length).len(), 1);

        let ticks = parse_binary(&frame(&[&[1, 2, 3, 4, 5, 6, 7], &second]));
        assert_eq!(ticks.len(), 1, "a sub-8-byte packet is skipped");
        assert_eq!(ticks[0].instrument_token, 408_321);

        let mut trailing = frame(&[&first]);
        trailing.extend_from_slice(&[0xFF; 5]);
        assert_eq!(
            parse_binary(&trailing).len(),
            1,
            "bytes past the count are ignored"
        );
    }

    #[test]
    fn regular_packet_prefixes_decode_only_complete_sections() {
        let full = full_packet(408_065);
        for len in 8..=full.len() {
            let ticks = parse_binary(&frame(&[&full[..len]]));
            assert_eq!(ticks.len(), 1, "len {len}");
            let tick = &ticks[0];
            assert_eq!(tick.instrument_token, 408_065, "len {len}");
            assert_eq!(
                tick.last_price,
                Some(f64::from(0x0100_0001) / 100.0),
                "len {len}"
            );
            let quote = (len >= 44).then_some(0x0100_0002);
            assert_eq!(tick.last_quantity, quote, "len {len}");
            assert_eq!(
                tick.close,
                quote.map(|_| f64::from(0x0100_000A) / 100.0),
                "len {len}"
            );
            let extended = (len >= 64).then_some(0x0100_000F);
            assert_eq!(tick.exchange_time, extended, "len {len}");
            assert_eq!(tick.oi_day_low, extended.map(|_| 0x0100_000E), "len {len}");
            assert_eq!(tick.depth.is_some(), len >= 184, "len {len}");
        }

        let depth = parse_binary(&frame(&[&full]))[0].depth.clone().unwrap();
        assert_eq!(depth.buy[0].quantity, 100);
        assert_eq!(depth.buy[0].price.to_bits(), f64::to_bits(2.0));
        assert_eq!(depth.buy[0].orders, 0x0102);
        assert_eq!(depth.sell[4].quantity, 109);
    }

    #[test]
    fn index_packet_prefixes_decode_only_complete_sections() {
        let full = full_packet(256_265);
        for len in 8..=44 {
            let ticks = parse_binary(&frame(&[&full[..len]]));
            assert_eq!(ticks.len(), 1, "len {len}");
            let tick = &ticks[0];
            assert!(tick.is_index, "len {len}");
            assert_eq!(
                tick.close,
                (len >= 28).then_some(f64::from(0x0100_0005) / 100.0)
            );
            assert_eq!(tick.exchange_time, (len >= 32).then_some(0x0100_0007));
            assert_eq!(tick.last_quantity, None, "len {len}");
        }
    }

    proptest::proptest! {
        #[test]
        fn arbitrary_bytes_never_panic(data in proptest::collection::vec(proptest::num::u8::ANY, 0..512)) {
            let _ = parse_binary(&data);
        }

        #[test]
        fn well_framed_packets_yield_one_tick_per_packet_of_eight_or_more_bytes(
            packets in proptest::collection::vec(
                proptest::collection::vec(proptest::num::u8::ANY, 0..200),
                0..6,
            ),
        ) {
            let slices: Vec<&[u8]> = packets.iter().map(Vec::as_slice).collect();
            let expected = packets.iter().filter(|packet| packet.len() >= 8).count();
            proptest::prop_assert_eq!(parse_binary(&frame(&slices)).len(), expected);
        }
    }
}
