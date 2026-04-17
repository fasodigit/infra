// SPDX-License-Identifier: AGPL-3.0-or-later
//! RESP3 command handlers for KAYA Geo commands.
//!
//! Implements GEOADD, GEOPOS, GEODIST, GEOSEARCH, GEOSEARCHSTORE,
//! GEORADIUS (deprecated legacy), GEORADIUSBYMEMBER (deprecated legacy),
//! GEOHASH, and GEOREM (KAYA extension).
//!
//! All handlers return a [`kaya_protocol::Frame`] and follow the same error
//! propagation rules as the rest of `kaya-commands`: no panics, no
//! `unwrap()`, errors wrapped in `CommandError`.

use bytes::Bytes;
use kaya_protocol::{Command, Frame};
use kaya_store::geo::{
    GeoAddOpts, GeoPoint, GeoSearchQuery, Shape, SortOrder, Unit,
};

use crate::{CommandContext, CommandError};

// ---------------------------------------------------------------------------
// Public handler functions (called from router / handler)
// ---------------------------------------------------------------------------

/// GEOADD key [NX|XX] [CH] longitude latitude member [lon lat member …]
///
/// Returns an integer: number of members added (or changed when CH is set).
pub fn handle_geoadd(
    ctx: &CommandContext,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    // Minimum: key + lon + lat + member = 4 args
    if cmd.arg_count() < 4 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let key = cmd.arg_bytes(0)?;

    // Parse optional flags NX | XX | CH before the lon/lat/member triples.
    let mut opts = GeoAddOpts::default();
    let mut idx = 1usize;
    loop {
        let tok = cmd.arg_str(idx)?.to_ascii_uppercase();
        match tok.as_str() {
            "NX" => {
                opts.nx = true;
                idx += 1;
            }
            "XX" => {
                opts.xx = true;
                idx += 1;
            }
            "CH" => {
                opts.ch = true;
                idx += 1;
            }
            _ => break,
        }
        if idx >= cmd.arg_count() {
            return Err(CommandError::Syntax(
                "GEOADD: expected longitude after flags".into(),
            ));
        }
    }

    if opts.nx && opts.xx {
        return Err(CommandError::Syntax(
            "GEOADD: NX and XX options are mutually exclusive".into(),
        ));
    }

    // Remaining args must come in triples: longitude latitude member
    let remaining = cmd.arg_count() - idx;
    if remaining == 0 || remaining % 3 != 0 {
        return Err(CommandError::Syntax(
            "GEOADD: longitude latitude member [lon lat member …]".into(),
        ));
    }

    let mut members: Vec<(GeoPoint, Vec<u8>)> = Vec::with_capacity(remaining / 3);
    while idx < cmd.arg_count() {
        let lon_s = cmd.arg_str(idx)?;
        let lat_s = cmd.arg_str(idx + 1)?;
        let member = cmd.arg_bytes(idx + 2)?.to_vec();

        let lon: f64 = lon_s
            .parse()
            .map_err(|_| CommandError::Syntax(format!("invalid longitude: {lon_s}")))?;
        let lat: f64 = lat_s
            .parse()
            .map_err(|_| CommandError::Syntax(format!("invalid latitude: {lat_s}")))?;

        let point = GeoPoint::new(lat, lon)
            .map_err(|e| CommandError::Syntax(e.to_string()))?;

        members.push((point, member));
        idx += 3;
    }

    let count = ctx
        .store
        .geoadd(key, &members, opts)
        .map_err(|e| CommandError::Syntax(e.to_string()))?;

    Ok(Frame::Integer(count))
}

/// GEOPOS key member [member …]
///
/// Returns an array; each element is either an array [longitude, latitude]
/// or a Null for members not found.
pub fn handle_geopos(
    ctx: &CommandContext,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 2 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let key = cmd.arg_bytes(0)?;
    let members: Vec<&[u8]> = (1..cmd.arg_count())
        .map(|i| cmd.arg_bytes(i).map(|b| b.as_ref()))
        .collect::<Result<Vec<_>, _>>()?;

    let positions = ctx.store.geopos(key, &members);

    let frames: Vec<Frame> = positions
        .into_iter()
        .map(|opt| match opt {
            None => Frame::Null,
            Some(pt) => Frame::Array(vec![
                Frame::BulkString(Bytes::from(format!("{:.17}", pt.lon))),
                Frame::BulkString(Bytes::from(format!("{:.17}", pt.lat))),
            ]),
        })
        .collect();

    Ok(Frame::Array(frames))
}

/// GEODIST key member1 member2 [M|KM|MI|FT]
///
/// Returns a bulk string with the distance or Null if either member is absent.
pub fn handle_geodist(
    ctx: &CommandContext,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 3 || cmd.arg_count() > 4 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let key = cmd.arg_bytes(0)?;
    let m1 = cmd.arg_bytes(1)?;
    let m2 = cmd.arg_bytes(2)?;

    let unit = if cmd.arg_count() == 4 {
        Unit::parse(cmd.arg_str(3)?)
            .map_err(|e| CommandError::Syntax(e.to_string()))?
    } else {
        Unit::M
    };

    match ctx
        .store
        .geodist(key, m1, m2, unit)
        .map_err(|e| CommandError::Syntax(e.to_string()))?
    {
        None => Ok(Frame::Null),
        Some(d) => Ok(Frame::BulkString(Bytes::from(format!("{d:.4}")))),
    }
}

/// GEOSEARCH key FROMMEMBER member | FROMLONLAT lon lat
///           BYRADIUS r [unit] | BYBOX w h [unit]
///           [ASC|DESC] [COUNT n [ANY]]
///           [WITHCOORD] [WITHDIST] [WITHHASH]
pub fn handle_geosearch(
    ctx: &CommandContext,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 5 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let key = cmd.arg_bytes(0)?;
    let (query, _) = parse_geosearch_args(ctx, cmd, 1, key)?;

    let with_coord = query.with_coord;
    let with_dist = query.with_dist;
    let with_hash = query.with_hash;
    let unit = query.unit;

    let results = ctx
        .store
        .geosearch(key, query)
        .map_err(|e| CommandError::Syntax(e.to_string()))?;

    Ok(encode_geosearch_results(results, with_coord, with_dist, with_hash, unit))
}

/// GEOSEARCHSTORE dest src FROMMEMBER … | FROMLONLAT … BYRADIUS … | BYBOX …
///               [ASC|DESC] [COUNT n]
///
/// Returns an integer: number of elements stored in the destination key.
pub fn handle_geosearchstore(
    ctx: &CommandContext,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 6 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let dest = cmd.arg_bytes(0)?;
    let src = cmd.arg_bytes(1)?;
    let (query, _) = parse_geosearch_args(ctx, cmd, 2, src)?;

    let count = ctx.store.geosearchstore(dest, src, query);
    Ok(Frame::Integer(count))
}

/// GEORADIUS key longitude latitude radius [M|KM|MI|FT]
///           [WITHCOORD] [WITHDIST] [WITHHASH] [COUNT n] [ASC|DESC]
///
/// **Deprecated** — kept for backward compatibility. Internally delegates
/// to GEOSEARCH FROMLONLAT BYRADIUS.
#[deprecated(
    since = "0.1.0",
    note = "Use GEOSEARCH FROMLONLAT … BYRADIUS … instead"
)]
pub fn handle_georadius(
    ctx: &CommandContext,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    // GEORADIUS key lon lat radius [unit] [options…]
    if cmd.arg_count() < 5 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let key = cmd.arg_bytes(0)?;
    let lon: f64 = cmd
        .arg_str(1)?
        .parse()
        .map_err(|_| CommandError::Syntax("GEORADIUS: invalid longitude".into()))?;
    let lat: f64 = cmd
        .arg_str(2)?
        .parse()
        .map_err(|_| CommandError::Syntax("GEORADIUS: invalid latitude".into()))?;
    let radius: f64 = cmd
        .arg_str(3)?
        .parse()
        .map_err(|_| CommandError::Syntax("GEORADIUS: invalid radius".into()))?;

    let unit = Unit::parse(cmd.arg_str(4)?)
        .map_err(|e| CommandError::Syntax(e.to_string()))?;

    let center = GeoPoint::new(lat, lon)
        .map_err(|e| CommandError::Syntax(e.to_string()))?;

    let (with_coord, with_dist, with_hash, count, sort) =
        parse_search_flags(cmd, 5)?;

    let query = GeoSearchQuery {
        shape: Shape::Radius {
            center,
            radius_m: unit.to_meters(radius),
        },
        unit,
        count,
        sort,
        with_coord,
        with_dist,
        with_hash,
    };

    let results = ctx
        .store
        .geosearch(key, query)
        .map_err(|e| CommandError::Syntax(e.to_string()))?;

    Ok(encode_geosearch_results(results, with_coord, with_dist, with_hash, unit))
}

/// GEORADIUSBYMEMBER key member radius [M|KM|MI|FT] [options…]
///
/// **Deprecated** — kept for backward compatibility. Internally delegates
/// to GEOSEARCH FROMMEMBER … BYRADIUS ….
#[deprecated(
    since = "0.1.0",
    note = "Use GEOSEARCH FROMMEMBER … BYRADIUS … instead"
)]
pub fn handle_georadiusbymember(
    ctx: &CommandContext,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 4 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let key = cmd.arg_bytes(0)?;
    let member = cmd.arg_bytes(1)?;
    let radius: f64 = cmd
        .arg_str(2)?
        .parse()
        .map_err(|_| CommandError::Syntax("GEORADIUSBYMEMBER: invalid radius".into()))?;
    let unit = Unit::parse(cmd.arg_str(3)?)
        .map_err(|e| CommandError::Syntax(e.to_string()))?;

    // Look up the member's position to convert FROMMEMBER to FROMLONLAT.
    let positions = ctx.store.geopos(key, &[member.as_ref()]);
    let center = match positions.into_iter().next().flatten() {
        None => {
            return Err(CommandError::Syntax(format!(
                "GEORADIUSBYMEMBER: member not found"
            )))
        }
        Some(pt) => pt,
    };

    let (with_coord, with_dist, with_hash, count, sort) =
        parse_search_flags(cmd, 4)?;

    let query = GeoSearchQuery {
        shape: Shape::Radius {
            center,
            radius_m: unit.to_meters(radius),
        },
        unit,
        count,
        sort,
        with_coord,
        with_dist,
        with_hash,
    };

    let results = ctx
        .store
        .geosearch(key, query)
        .map_err(|e| CommandError::Syntax(e.to_string()))?;

    Ok(encode_geosearch_results(results, with_coord, with_dist, with_hash, unit))
}

/// GEOHASH key member [member …]
///
/// Returns an array of bulk strings (11-char base32 geohashes) or Null for
/// missing members.
pub fn handle_geohash(
    ctx: &CommandContext,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 2 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let key = cmd.arg_bytes(0)?;
    let members: Vec<&[u8]> = (1..cmd.arg_count())
        .map(|i| cmd.arg_bytes(i).map(|b| b.as_ref()))
        .collect::<Result<Vec<_>, _>>()?;

    let hashes = ctx.store.geohash(key, &members);
    let frames: Vec<Frame> = hashes
        .into_iter()
        .map(|opt| match opt {
            None => Frame::Null,
            Some(h) => Frame::BulkString(Bytes::from(h)),
        })
        .collect();

    Ok(Frame::Array(frames))
}

/// GEOREM key member [member …]
///
/// KAYA extension — semantically equivalent to ZREM on the geo index.
/// Returns an integer: number of members removed.
pub fn handle_georem(
    ctx: &CommandContext,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 2 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let key = cmd.arg_bytes(0)?;
    let members: Vec<&[u8]> = (1..cmd.arg_count())
        .map(|i| cmd.arg_bytes(i).map(|b| b.as_ref()))
        .collect::<Result<Vec<_>, _>>()?;

    let removed = ctx.store.georem(key, &members);
    Ok(Frame::Integer(removed))
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Parse GEOSEARCH-style options starting at argument index `start`.
///
/// Handles: FROMMEMBER member | FROMLONLAT lon lat
///          BYRADIUS r [unit] | BYBOX w h [unit]
///          [ASC|DESC] [COUNT n [ANY]]
///          [WITHCOORD] [WITHDIST] [WITHHASH]
///
/// Returns `(GeoSearchQuery, next_idx)`.
fn parse_geosearch_args(
    ctx: &CommandContext,
    cmd: &Command,
    start: usize,
    key: &[u8],
) -> Result<(GeoSearchQuery, usize), CommandError> {
    let mut i = start;

    // -- centre: FROMMEMBER | FROMLONLAT ------------------------------------
    let center: GeoPoint = match cmd.arg_str(i)?.to_ascii_uppercase().as_str() {
        "FROMMEMBER" => {
            i += 1;
            let member = cmd.arg_bytes(i)?;
            i += 1;
            let positions = ctx.store.geopos(key, &[member.as_ref()]);
            positions
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(|| CommandError::Syntax("GEOSEARCH: member not found".into()))?
        }
        "FROMLONLAT" => {
            let lon: f64 = cmd
                .arg_str(i + 1)?
                .parse()
                .map_err(|_| CommandError::Syntax("invalid longitude".into()))?;
            let lat: f64 = cmd
                .arg_str(i + 2)?
                .parse()
                .map_err(|_| CommandError::Syntax("invalid latitude".into()))?;
            i += 3;
            GeoPoint::new(lat, lon).map_err(|e| CommandError::Syntax(e.to_string()))?
        }
        other => {
            return Err(CommandError::Syntax(format!(
                "GEOSEARCH: expected FROMMEMBER or FROMLONLAT, got {other}"
            )))
        }
    };

    // -- shape: BYRADIUS | BYBOX -------------------------------------------
    if i >= cmd.arg_count() {
        return Err(CommandError::Syntax(
            "GEOSEARCH: expected BYRADIUS or BYBOX".into(),
        ));
    }

    let shape: Shape = match cmd.arg_str(i)?.to_ascii_uppercase().as_str() {
        "BYRADIUS" => {
            let radius: f64 = cmd
                .arg_str(i + 1)?
                .parse()
                .map_err(|_| CommandError::Syntax("invalid radius".into()))?;
            let unit = Unit::parse(cmd.arg_str(i + 2)?)
                .map_err(|e| CommandError::Syntax(e.to_string()))?;
            i += 3;
            if radius < 0.0 || !radius.is_finite() {
                return Err(CommandError::Syntax("radius must be positive".into()));
            }
            Shape::Radius { center, radius_m: unit.to_meters(radius) }
        }
        "BYBOX" => {
            let width: f64 = cmd
                .arg_str(i + 1)?
                .parse()
                .map_err(|_| CommandError::Syntax("invalid width".into()))?;
            let height: f64 = cmd
                .arg_str(i + 2)?
                .parse()
                .map_err(|_| CommandError::Syntax("invalid height".into()))?;
            let unit = Unit::parse(cmd.arg_str(i + 3)?)
                .map_err(|e| CommandError::Syntax(e.to_string()))?;
            i += 4;
            Shape::Box {
                center,
                width_m: unit.to_meters(width),
                height_m: unit.to_meters(height),
            }
        }
        other => {
            return Err(CommandError::Syntax(format!(
                "GEOSEARCH: expected BYRADIUS or BYBOX, got {other}"
            )))
        }
    };

    // Determine unit for result formatting (last seen unit).
    let unit = match &shape {
        Shape::Radius { .. } | Shape::Box { .. } => {
            // Re-derive from the shape parse above; pass via closure is not clean,
            // so we default to M if we cannot determine.
            Unit::M
        }
    };
    // Parse unit again from the already-consumed position is awkward; we store it.
    // Instead: pull it from the shape arms above via a helper enum.
    let result_unit = extract_unit_from_shape(cmd, start, i)?;

    // -- optional modifiers --------------------------------------------------
    let (with_coord, with_dist, with_hash, count, sort) = parse_search_flags(cmd, i)?;
    let _ = unit; // replaced by result_unit

    Ok((
        GeoSearchQuery {
            shape,
            unit: result_unit,
            count,
            sort,
            with_coord,
            with_dist,
            with_hash,
        },
        i,
    ))
}

/// Re-extract the unit token from the already-parsed arg range.
/// This is a best-effort pass: if parsing fails we fall back to `Unit::M`.
fn extract_unit_from_shape(
    cmd: &Command,
    start: usize,
    end: usize,
) -> Result<Unit, CommandError> {
    // Walk the tokens and find the last unit string.
    let mut result = Unit::M;
    for i in start..end {
        if let Ok(s) = cmd.arg_str(i) {
            if let Ok(u) = Unit::parse(s) {
                result = u;
            }
        }
    }
    Ok(result)
}

/// Parse the optional tail flags: WITHCOORD, WITHDIST, WITHHASH, COUNT n,
/// ASC, DESC. Returns `(with_coord, with_dist, with_hash, count, sort)`.
fn parse_search_flags(
    cmd: &Command,
    start: usize,
) -> Result<(bool, bool, bool, Option<usize>, SortOrder), CommandError> {
    let mut with_coord = false;
    let mut with_dist = false;
    let mut with_hash = false;
    let mut count: Option<usize> = None;
    let mut sort = SortOrder::Unordered;

    let mut i = start;
    while i < cmd.arg_count() {
        match cmd.arg_str(i)?.to_ascii_uppercase().as_str() {
            "WITHCOORD" => {
                with_coord = true;
                i += 1;
            }
            "WITHDIST" => {
                with_dist = true;
                i += 1;
            }
            "WITHHASH" => {
                with_hash = true;
                i += 1;
            }
            "ASC" => {
                sort = SortOrder::Asc;
                i += 1;
            }
            "DESC" => {
                sort = SortOrder::Desc;
                i += 1;
            }
            "COUNT" => {
                let n: usize = cmd
                    .arg_str(i + 1)?
                    .parse()
                    .map_err(|_| CommandError::Syntax("COUNT: expected integer".into()))?;
                count = Some(n);
                i += 2;
                // Optional ANY modifier — consume it but ignore (KAYA does not
                // short-circuit on ANY yet; it always returns exact results).
                if i < cmd.arg_count()
                    && cmd
                        .arg_str(i)
                        .map(|s| s.eq_ignore_ascii_case("ANY"))
                        .unwrap_or(false)
                {
                    i += 1;
                }
            }
            _ => {
                i += 1; // skip unknown modifiers gracefully
            }
        }
    }

    Ok((with_coord, with_dist, with_hash, count, sort))
}

/// Encode GEOSEARCH results into a RESP3 Array frame, honouring WITHCOORD /
/// WITHDIST / WITHHASH flags.
///
/// - No flags: Array of bulk strings (member names only).
/// - With any flag: Array of Arrays: `[member, [dist], [hash], [lon, lat]]`.
fn encode_geosearch_results(
    results: Vec<kaya_store::geo::GeoSearchResult>,
    with_coord: bool,
    with_dist: bool,
    with_hash: bool,
    unit: Unit,
) -> Frame {
    let any_extra = with_coord || with_dist || with_hash;

    let frames: Vec<Frame> = results
        .into_iter()
        .map(|r| {
            if !any_extra {
                return Frame::BulkString(r.member);
            }
            let mut row: Vec<Frame> = vec![Frame::BulkString(r.member)];
            if with_dist {
                let d = unit.from_meters(r.distance_m);
                row.push(Frame::BulkString(Bytes::from(format!("{d:.4}"))));
            }
            if with_hash {
                row.push(Frame::Integer(r.hash as i64));
            }
            if with_coord {
                row.push(Frame::Array(vec![
                    Frame::BulkString(Bytes::from(format!("{:.17}", r.point.lon))),
                    Frame::BulkString(Bytes::from(format!("{:.17}", r.point.lat))),
                ]));
            }
            Frame::Array(row)
        })
        .collect();

    Frame::Array(frames)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use kaya_protocol::Frame;
    use kaya_store::{BloomManager, Store};
    use kaya_streams::StreamManager;

    fn make_ctx() -> CommandContext {
        let store = Arc::new(Store::default());
        let streams = Arc::new(StreamManager::new(1024));
        let blooms = Arc::new(BloomManager::new());
        CommandContext::new(store, streams, blooms)
    }

    fn cmd(args: &[&str]) -> Command {
        Command {
            name: args[0].to_ascii_uppercase(),
            args: args[1..]
                .iter()
                .map(|s| Bytes::from(s.to_string()))
                .collect(),
        }
    }

    // -----------------------------------------------------------------------
    // GEOADD happy path
    // -----------------------------------------------------------------------

    #[test]
    fn geoadd_three_cities() {
        let ctx = make_ctx();
        let c = cmd(&[
            "GEOADD", "cities",
            "2.35", "48.85", "Paris",
            "-74.00", "40.71", "NewYork",
            "139.69", "35.68", "Tokyo",
        ]);
        let frame = handle_geoadd(&ctx, &c).unwrap();
        assert_eq!(frame, Frame::Integer(3));
    }

    // -----------------------------------------------------------------------
    // GEOPOS — existing + missing members
    // -----------------------------------------------------------------------

    #[test]
    fn geopos_existing_and_missing() {
        let ctx = make_ctx();
        handle_geoadd(
            &ctx,
            &cmd(&["GEOADD", "cities", "2.35", "48.85", "Paris"]),
        )
        .unwrap();

        let c = cmd(&["GEOPOS", "cities", "Paris", "Ghost"]);
        let frame = handle_geopos(&ctx, &c).unwrap();
        match frame {
            Frame::Array(elems) => {
                assert_eq!(elems.len(), 2);
                assert!(matches!(elems[0], Frame::Array(_)), "Paris has coords");
                assert_eq!(elems[1], Frame::Null, "Ghost is Null");
            }
            _ => panic!("expected Array"),
        }
    }

    // -----------------------------------------------------------------------
    // GEODIST Paris <-> NewYork ≈ 5836 km
    // -----------------------------------------------------------------------

    #[test]
    fn geodist_paris_new_york_km() {
        let ctx = make_ctx();
        handle_geoadd(
            &ctx,
            &cmd(&[
                "GEOADD", "cities",
                "2.35", "48.85", "Paris",
                "-74.00", "40.71", "NewYork",
            ]),
        )
        .unwrap();

        let c = cmd(&["GEODIST", "cities", "Paris", "NewYork", "KM"]);
        match handle_geodist(&ctx, &c).unwrap() {
            Frame::BulkString(b) => {
                let s = std::str::from_utf8(&b).unwrap();
                let d: f64 = s.parse().unwrap();
                let expected = 5836.0_f64;
                let err_pct = ((d - expected) / expected).abs() * 100.0;
                assert!(err_pct < 0.5, "dist {d:.1} km, err {err_pct:.3}%");
            }
            other => panic!("expected BulkString, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // GEOSEARCH FROMLONLAT BYRADIUS returns all 3 cities at 10000 km
    // -----------------------------------------------------------------------

    #[test]
    fn geosearch_radius_10000km_from_paris() {
        let ctx = make_ctx();
        handle_geoadd(
            &ctx,
            &cmd(&[
                "GEOADD", "cities",
                "2.35", "48.85", "Paris",
                "-74.00", "40.71", "NewYork",
                "139.69", "35.68", "Tokyo",
            ]),
        )
        .unwrap();

        let c = cmd(&[
            "GEOSEARCH", "cities",
            "FROMLONLAT", "2.35", "48.85",
            "BYRADIUS", "10000", "KM",
            "ASC",
        ]);
        match handle_geosearch(&ctx, &c).unwrap() {
            Frame::Array(elems) => {
                assert_eq!(elems.len(), 3, "all 3 cities in 10000 km");
                // First should be Paris (closest to centre)
                if let Frame::BulkString(b) = &elems[0] {
                    assert_eq!(b.as_ref(), b"Paris");
                }
            }
            other => panic!("expected Array, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // GEOSEARCH BYBOX 1000x1000 km → only Paris
    // -----------------------------------------------------------------------

    #[test]
    fn geosearch_box_only_paris() {
        let ctx = make_ctx();
        handle_geoadd(
            &ctx,
            &cmd(&[
                "GEOADD", "cities",
                "2.35", "48.85", "Paris",
                "-74.00", "40.71", "NewYork",
                "139.69", "35.68", "Tokyo",
            ]),
        )
        .unwrap();

        let c = cmd(&[
            "GEOSEARCH", "cities",
            "FROMLONLAT", "2.35", "48.85",
            "BYBOX", "1000", "1000", "KM",
        ]);
        match handle_geosearch(&ctx, &c).unwrap() {
            Frame::Array(elems) => {
                assert_eq!(elems.len(), 1);
                if let Frame::BulkString(b) = &elems[0] {
                    assert_eq!(b.as_ref(), b"Paris");
                }
            }
            other => panic!("expected Array, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // GEOHASH — valid 11-char string for Paris, Null for unknown
    // -----------------------------------------------------------------------

    #[test]
    fn geohash_paris_and_unknown() {
        let ctx = make_ctx();
        handle_geoadd(
            &ctx,
            &cmd(&["GEOADD", "cities", "2.35", "48.85", "Paris"]),
        )
        .unwrap();

        let c = cmd(&["GEOHASH", "cities", "Paris", "Unknown"]);
        match handle_geohash(&ctx, &c).unwrap() {
            Frame::Array(elems) => {
                assert_eq!(elems.len(), 2);
                if let Frame::BulkString(b) = &elems[0] {
                    let h = std::str::from_utf8(b).unwrap();
                    assert_eq!(h.len(), 11);
                    assert!(h.starts_with('u'), "Paris hash starts with u");
                }
                assert_eq!(elems[1], Frame::Null);
            }
            other => panic!("expected Array, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // GEOREM — removes known members
    // -----------------------------------------------------------------------

    #[test]
    fn georem_removes_member() {
        let ctx = make_ctx();
        handle_geoadd(
            &ctx,
            &cmd(&["GEOADD", "cities", "2.35", "48.85", "Paris"]),
        )
        .unwrap();
        let c = cmd(&["GEOREM", "cities", "Paris", "Ghost"]);
        assert_eq!(handle_georem(&ctx, &c).unwrap(), Frame::Integer(1));

        // Verify Paris is gone.
        let pos_cmd = cmd(&["GEOPOS", "cities", "Paris"]);
        match handle_geopos(&ctx, &pos_cmd).unwrap() {
            Frame::Array(elems) => assert_eq!(elems[0], Frame::Null),
            _ => panic!("expected Array"),
        }
    }

    // -----------------------------------------------------------------------
    // GEORADIUS backward-compat (deprecated)
    // -----------------------------------------------------------------------

    #[test]
    fn georadius_compat() {
        let ctx = make_ctx();
        handle_geoadd(
            &ctx,
            &cmd(&[
                "GEOADD", "cities",
                "2.35", "48.85", "Paris",
                "-74.00", "40.71", "NewYork",
            ]),
        )
        .unwrap();

        // GEORADIUS cities 2.35 48.85 6000 KM ASC
        let c = cmd(&["GEORADIUS", "cities", "2.35", "48.85", "6000", "KM", "ASC"]);
        #[allow(deprecated)]
        match handle_georadius(&ctx, &c).unwrap() {
            Frame::Array(elems) => {
                assert_eq!(elems.len(), 2, "Paris + NewYork within 6000 km");
            }
            other => panic!("expected Array, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Edge cases: invalid lat/lon in GEOADD → syntax error
    // -----------------------------------------------------------------------

    #[test]
    fn geoadd_invalid_lat_returns_error() {
        let ctx = make_ctx();
        let c = cmd(&["GEOADD", "cities", "0.0", "91.0", "BadCity"]);
        let err = handle_geoadd(&ctx, &c).unwrap_err();
        assert!(
            matches!(err, CommandError::Syntax(_)),
            "expected Syntax error for lat=91"
        );
    }

    #[test]
    fn geoadd_invalid_lon_returns_error() {
        let ctx = make_ctx();
        let c = cmd(&["GEOADD", "cities", "200.0", "0.0", "BadCity"]);
        let err = handle_geoadd(&ctx, &c).unwrap_err();
        assert!(
            matches!(err, CommandError::Syntax(_)),
            "expected Syntax error for lon=200"
        );
    }

    // -----------------------------------------------------------------------
    // GEOSEARCHSTORE copies correct number of members
    // -----------------------------------------------------------------------

    #[test]
    fn geosearchstore_handler() {
        let ctx = make_ctx();
        handle_geoadd(
            &ctx,
            &cmd(&[
                "GEOADD", "src",
                "2.35", "48.85", "Paris",
                "-74.00", "40.71", "NewYork",
                "139.69", "35.68", "Tokyo",
            ]),
        )
        .unwrap();

        let c = cmd(&[
            "GEOSEARCHSTORE", "dst", "src",
            "FROMLONLAT", "2.35", "48.85",
            "BYRADIUS", "10000", "KM",
        ]);
        assert_eq!(handle_geosearchstore(&ctx, &c).unwrap(), Frame::Integer(3));
    }
}
