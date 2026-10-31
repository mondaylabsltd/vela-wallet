//! What a client uses of a relay, counted exactly per UTC day
//! (`docs/self-serve.md`, "Metering and alerts").
//!
//! Resource use only, never content: connections, how long they lasted, the
//! frames and bytes that crossed, and how many the relay refused. No room IDs,
//! no keys, no addresses. A relay keeps one [`Day`] per client per day and
//! writes it out when the day is over.
//!
//! Pure: times are the caller's (epoch milliseconds).

use serde::{Deserialize, Serialize};

const DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// One client's use on one UTC day.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Day {
    /// `YYYY-MM-DD`.
    pub day: String,
    /// Connections the relay admitted.
    #[serde(default)]
    pub connections: u64,
    /// Milliseconds those connections were open.
    #[serde(default)]
    pub connected_ms: u64,
    /// Frames and bytes from the client, and forwarded to it.
    #[serde(default)]
    pub frames_in: u64,
    #[serde(default)]
    pub frames_out: u64,
    #[serde(default)]
    pub bytes_in: u64,
    #[serde(default)]
    pub bytes_out: u64,
    /// Connections refused for a rotated or deleted token.
    #[serde(default)]
    pub rejections: u64,
    /// Connections refused for opening too many, or for the trial's limits.
    #[serde(default)]
    pub limited: u64,
}

/// What a room hands a client's object: its connections' use since it last did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Traffic {
    #[serde(default)]
    pub frames_in: u64,
    #[serde(default)]
    pub frames_out: u64,
    #[serde(default)]
    pub bytes_in: u64,
    #[serde(default)]
    pub bytes_out: u64,
}

impl Traffic {
    pub fn is_empty(&self) -> bool {
        *self == Traffic::default()
    }

    pub fn add(&mut self, other: Traffic) {
        self.frames_in += other.frames_in;
        self.frames_out += other.frames_out;
        self.bytes_in += other.bytes_in;
        self.bytes_out += other.bytes_out;
    }
}

impl Day {
    pub fn new(day: String) -> Day {
        Day { day, ..Day::default() }
    }

    pub fn traffic(&mut self, t: Traffic) {
        self.frames_in += t.frames_in;
        self.frames_out += t.frames_out;
        self.bytes_in += t.bytes_in;
        self.bytes_out += t.bytes_out;
    }

    /// Bytes both ways.
    pub fn bytes(&self) -> u64 {
        self.bytes_in + self.bytes_out
    }
}

/// A day's use beyond which a client is worth a look (`docs/self-serve.md`,
/// "Metering and alerts"). Not limits: nothing is refused for crossing one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Thresholds {
    pub connections: u64,
    pub bytes: u64,
    pub rejections: u64,
    pub limited: u64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds { connections: 10_000, bytes: 10_000_000_000, rejections: 100, limited: 1_000 }
    }
}

impl Thresholds {
    /// The thresholds that going from `before` to `after` crossed, as
    /// `(kind, value, threshold)`: each fires once a day, when it is crossed.
    pub fn crossed(&self, before: &Day, after: &Day) -> Vec<(&'static str, u64, u64)> {
        [
            ("connections", before.connections, after.connections, self.connections),
            ("bytes", before.bytes(), after.bytes(), self.bytes),
            ("rejections", before.rejections, after.rejections, self.rejections),
            ("limited", before.limited, after.limited, self.limited),
        ]
        .into_iter()
        .filter(|&(_, was, now, at)| at > 0 && was < at && now >= at)
        .map(|(kind, _, now, at)| (kind, now, at))
        .collect()
    }
}

/// The UTC day of `epoch_ms`, as `YYYY-MM-DD`.
pub fn day_of(epoch_ms: u64) -> String {
    let days = (epoch_ms / DAY_MS) as i64;
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The first millisecond of the UTC day after `epoch_ms`.
pub fn next_day_ms(epoch_ms: u64) -> u64 {
    (epoch_ms / DAY_MS + 1) * DAY_MS
}

/// Time from `from_ms` to `to_ms`, split across the UTC days it spans:
/// `(day, milliseconds)` in order.
pub fn split_days(from_ms: u64, to_ms: u64) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let mut at = from_ms;
    while at < to_ms {
        let end = next_day_ms(at).min(to_ms);
        out.push((day_of(at), end - at));
        at = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_are_utc_dates() {
        assert_eq!(day_of(0), "1970-01-01");
        assert_eq!(day_of(1_793_059_200_000), "2026-10-27");
        assert_eq!(day_of(1_793_059_200_000 - 1), "2026-10-26");
        assert_eq!(day_of(951_782_400_000), "2000-02-29");
        assert_eq!(next_day_ms(1_793_059_200_000 + 5), 1_793_059_200_000 + DAY_MS);
    }

    #[test]
    fn time_is_counted_on_each_day_it_spans() {
        let midnight = 1_793_059_200_000;
        let split = split_days(midnight - 1_000, midnight + 2_500);
        assert_eq!(split, vec![("2026-10-26".to_owned(), 1_000), ("2026-10-27".to_owned(), 2_500)]);
        assert!(split_days(5, 5).is_empty());
    }

    #[test]
    fn a_threshold_fires_once_when_crossed() {
        let t = Thresholds { connections: 3, bytes: 1_000, rejections: 1, limited: 0 };
        let mut day = Day::new("2026-10-28".into());
        let step = |day: &mut Day, f: &dyn Fn(&mut Day)| {
            let before = day.clone();
            f(day);
            t.crossed(&before, day)
        };
        assert!(step(&mut day, &|d| d.connections += 2).is_empty());
        assert_eq!(step(&mut day, &|d| d.connections += 1), vec![("connections", 3, 3)]);
        assert!(step(&mut day, &|d| d.connections += 1).is_empty(), "once a day");
        assert_eq!(
            step(&mut day, &|d| {
                d.bytes_out += 2_000;
                d.rejections += 1;
            }),
            vec![("bytes", 2_000, 1_000), ("rejections", 1, 1)]
        );
        assert!(step(&mut day, &|d| d.limited += 5).is_empty(), "0 turns a threshold off");
    }

    #[test]
    fn traffic_adds_up_and_reads_back() {
        let mut day = Day::new("2026-10-27".into());
        day.traffic(Traffic { frames_in: 2, frames_out: 3, bytes_in: 100, bytes_out: 2_000 });
        day.traffic(Traffic { frames_in: 1, bytes_in: 5, ..Traffic::default() });
        assert_eq!((day.frames_in, day.frames_out, day.bytes(), day.bytes_in), (3, 3, 2_105, 105));
        let text = serde_json::to_string(&day).unwrap();
        assert_eq!(serde_json::from_str::<Day>(&text).unwrap(), day);
        assert_eq!(serde_json::from_str::<Day>(r#"{"day":"x"}"#).unwrap(), Day::new("x".into()));
    }
}
