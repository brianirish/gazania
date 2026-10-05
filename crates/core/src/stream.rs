//! The engine behind `gazania watch`: one loop that emits typed events for
//! volumes, throughput and health on a schedule, pulls volumes forward when
//! udisks2 reports a change, and reconnects with backoff when the system bus
//! goes away. Every failure becomes an `error` event; the loop only ends when
//! its consumer does.

use crate::client::{backoff_delay, ChangeStream, Client};
use crate::error::{Error, Result};
use crate::health::Health;
use crate::io::{IoRate, IoSampler};
use crate::types::Drive;
use futures_lite::StreamExt;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::time::{Duration, Instant};

pub const PROTOCOL: u32 = 1;
/// How soon after a udisks2 change the volumes event goes out.
pub const DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum Event {
    Hello { protocol: u32, version: String },
    Volumes { drives: Vec<Drive> },
    Io { drives: Vec<IoRate> },
    Health { drives: Vec<Health> },
    Error { message: String },
}

impl Event {
    pub fn hello() -> Self {
        Event::Hello {
            protocol: PROTOCOL,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Event::Error {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kinds {
    pub volumes: bool,
    pub io: bool,
    pub health: bool,
}

impl Kinds {
    pub const ALL: Kinds = Kinds {
        volumes: true,
        io: true,
        health: true,
    };

    /// A comma-separated subset of `volumes,io,health`.
    pub fn parse(list: &str) -> Result<Kinds> {
        let mut kinds = Kinds {
            volumes: false,
            io: false,
            health: false,
        };
        for part in list.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            match part {
                "volumes" => kinds.volumes = true,
                "io" => kinds.io = true,
                "health" => kinds.health = true,
                other => {
                    return Err(Error::InvalidArgument(format!(
                        "unknown kind '{other}', expected volumes, io or health"
                    )))
                }
            }
        }
        if !(kinds.volumes || kinds.io || kinds.health) {
            return Err(Error::InvalidArgument(
                "expected at least one of volumes, io, health".into(),
            ));
        }
        Ok(kinds)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Intervals {
    pub io: Duration,
    pub health: Duration,
    pub usage: Duration,
}

impl Default for Intervals {
    fn default() -> Self {
        Self {
            io: Duration::from_secs(1),
            health: Duration::from_secs(60),
            usage: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    Volumes,
    Io,
    Health,
}

/// When each kind of event is next due. Pure, so the cadence is testable
/// without a clock or a bus.
#[derive(Debug, Clone)]
pub struct Schedule {
    intervals: Intervals,
    next_volumes: Option<Instant>,
    next_io: Option<Instant>,
    next_health: Option<Instant>,
}

impl Schedule {
    /// Volumes and health are due at `start`; io one interval later, because
    /// the first throughput sample only primes the counters.
    pub fn new(start: Instant, kinds: Kinds, intervals: Intervals) -> Self {
        Self {
            intervals,
            next_volumes: kinds.volumes.then_some(start),
            next_io: if kinds.io {
                start.checked_add(intervals.io)
            } else {
                None
            },
            next_health: kinds.health.then_some(start),
        }
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        [self.next_volumes, self.next_io, self.next_health]
            .into_iter()
            .flatten()
            .min()
    }

    /// The jobs due at `now`, in Volumes, Health, Io order; each is
    /// rescheduled one interval after `now`.
    pub fn take_due(&mut self, now: Instant) -> Vec<Job> {
        let mut jobs = Vec::new();
        if self.next_volumes.is_some_and(|t| t <= now) {
            jobs.push(Job::Volumes);
            self.next_volumes = now.checked_add(self.intervals.usage);
        }
        if self.next_health.is_some_and(|t| t <= now) {
            jobs.push(Job::Health);
            self.next_health = now.checked_add(self.intervals.health);
        }
        if self.next_io.is_some_and(|t| t <= now) {
            jobs.push(Job::Io);
            self.next_io = now.checked_add(self.intervals.io);
        }
        jobs
    }

    /// A udisks2 change: volumes go out within `DEBOUNCE`, never later than
    /// already planned, so a burst of signals produces one event.
    pub fn volumes_changed(&mut self, now: Instant) {
        if let Some(t) = self.next_volumes {
            self.next_volumes = Some(now.checked_add(DEBOUNCE).map_or(t, |d| t.min(d)));
        }
    }

    /// After a reconnect, volumes and health go out again right away.
    pub fn refresh_now(&mut self, now: Instant) {
        if self.next_volumes.is_some() {
            self.next_volumes = Some(now);
        }
        if self.next_health.is_some() {
            self.next_health = Some(now);
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Wake {
    Timer,
    Change,
    StreamEnded,
}

/// Sleeps until `deadline` or the next udisks2 change, whichever is first.
async fn wait(deadline: Option<Instant>, changes: Option<&mut ChangeStream>) -> Wake {
    let timer = async {
        match deadline {
            Some(at) => {
                async_io::Timer::at(at).await;
            }
            None => futures_lite::future::pending::<()>().await,
        }
        Wake::Timer
    };
    match changes {
        Some(stream) => {
            let change = async {
                match stream.next().await {
                    Some(_) => Wake::Change,
                    None => Wake::StreamEnded,
                }
            };
            futures_lite::future::or(change, timer).await
        }
        None => timer.await,
    }
}

/// Reconnect bookkeeping: when to try next and how far the backoff has grown.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Reconnect {
    attempt: u32,
    at: Option<Instant>,
    was_down: bool,
}

impl Reconnect {
    fn starting(now: Instant) -> Self {
        Self {
            attempt: 0,
            at: Some(now),
            was_down: false,
        }
    }

    fn due(&self, now: Instant) -> bool {
        self.at.is_some_and(|t| t <= now)
    }

    /// A failed connect or a lost connection: try again after the next
    /// backoff step.
    fn failed(&mut self, now: Instant) {
        self.was_down = true;
        self.at = Some(now + backoff_delay(self.attempt));
        self.attempt = self.attempt.saturating_add(1);
    }

    /// udisks2 answered. Returns whether the stream had been down, so the
    /// caller re-sends volumes and health.
    fn succeeded(&mut self) -> bool {
        self.attempt = 0;
        self.at = None;
        std::mem::take(&mut self.was_down)
    }
}

/// How long the stream waits for udisks2 before treating it as gone.
const UDISKS_TIMEOUT: Duration = Duration::from_secs(10);

/// `fut`, or a D-Bus error if it has not finished within `limit`.
async fn within<T>(limit: Duration, fut: impl Future<Output = Result<T>>) -> Result<T> {
    futures_lite::future::or(fut, async {
        async_io::Timer::after(limit).await;
        Err(Error::Dbus(format!(
            "udisks2 did not answer within {} s",
            limit.as_secs()
        )))
    })
    .await
}

/// A connection counts only once udisks2 has answered: subscribe to changes
/// first, so none slip past the snapshot, then fetch the drive list.
async fn connect_udisks(kinds: Kinds) -> Result<(Client, Option<ChangeStream>, Vec<Drive>)> {
    let client = Client::connect().await?;
    let changes = if kinds.volumes {
        Some(client.changes().await?)
    } else {
        None
    };
    let drives = client.drives().await?;
    Ok((client, changes, drives))
}

/// Runs the stream until `emit` fails, which means the consumer went away.
pub async fn run(
    kinds: Kinds,
    intervals: Intervals,
    emit: &mut dyn FnMut(&Event) -> std::io::Result<()>,
) {
    if emit(&Event::hello()).is_err() {
        return;
    }
    if !(kinds.volumes || kinds.io || kinds.health) {
        return;
    }
    let mut schedule = Schedule::new(Instant::now(), kinds, intervals);
    let mut sampler = IoSampler::new();
    if kinds.io {
        // Prime the counters; the first real rate comes one interval later.
        let _ = sampler.sample(&[]);
    }
    let mut drives: Vec<Drive> = Vec::new();
    let mut client: Option<Client> = None;
    let mut changes: Option<ChangeStream> = None;
    let mut reconnect = Reconnect::starting(Instant::now());

    loop {
        if client.is_none() && reconnect.due(Instant::now()) {
            match within(UDISKS_TIMEOUT, connect_udisks(kinds)).await {
                Ok((c, ch, d)) => {
                    client = Some(c);
                    changes = ch;
                    drives = d;
                    if reconnect.succeeded() {
                        schedule.refresh_now(Instant::now());
                    }
                }
                Err(e) => {
                    reconnect.failed(Instant::now());
                    let msg = match e {
                        Error::Dbus(_) | Error::DbusUnavailable(_) => {
                            format!("udisks2 unavailable ({e}), retrying")
                        }
                        _ => format!("startup failed ({e}), retrying"),
                    };
                    if emit(&Event::error(msg)).is_err() {
                        return;
                    }
                }
            }
        }

        for job in schedule.take_due(Instant::now()) {
            let event = match job {
                Job::Io => match sampler.sample(&drives) {
                    Ok(rates) => Event::Io { drives: rates },
                    Err(e) => Event::error(e.to_string()),
                },
                Job::Volumes | Job::Health => {
                    let Some(c) = client.as_ref() else {
                        continue;
                    };
                    let result = match job {
                        Job::Volumes => within(UDISKS_TIMEOUT, c.drives()).await.map(|d| {
                            drives = d.clone();
                            Event::Volumes { drives: d }
                        }),
                        _ => within(UDISKS_TIMEOUT, c.health())
                            .await
                            .map(|h| Event::Health { drives: h }),
                    };
                    match result {
                        Ok(event) => event,
                        Err(e @ (Error::Dbus(_) | Error::DbusUnavailable(_))) => {
                            client = None;
                            changes = None;
                            reconnect.failed(Instant::now());
                            Event::error(format!("udisks2 connection lost ({e}), reconnecting"))
                        }
                        Err(e) => Event::error(e.to_string()),
                    }
                }
            };
            if emit(&event).is_err() {
                return;
            }
        }

        let deadline = [schedule.next_deadline(), reconnect.at]
            .into_iter()
            .flatten()
            .min();
        match wait(deadline, changes.as_mut()).await {
            Wake::Timer => {}
            Wake::Change => schedule.volumes_changed(Instant::now()),
            Wake::StreamEnded => {
                client = None;
                changes = None;
                reconnect.failed(Instant::now());
                if emit(&Event::error("udisks2 signal stream ended, reconnecting")).is_err() {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::volumes::Change;

    const SEC: Duration = Duration::from_secs(1);

    #[test]
    fn hello_is_the_documented_first_line() {
        let line = serde_json::to_string(&Event::hello()).unwrap();
        assert_eq!(
            line,
            format!(
                "{{\"event\":\"hello\",\"protocol\":1,\"version\":\"{}\"}}",
                env!("CARGO_PKG_VERSION")
            )
        );
    }

    #[test]
    fn io_event_uses_the_documented_field_names() {
        let event = Event::Io {
            drives: vec![IoRate {
                device: "/dev/nvme0n1".into(),
                read_bps: 5,
                write_bps: 6,
            }],
        };
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            r#"{"event":"io","drives":[{"device":"/dev/nvme0n1","read_bps":5,"write_bps":6}]}"#
        );
    }

    #[test]
    fn every_event_round_trips_as_one_line() {
        let events = vec![
            Event::hello(),
            Event::Volumes { drives: vec![] },
            Event::Io {
                drives: vec![IoRate {
                    device: "/dev/sda".into(),
                    read_bps: 1,
                    write_bps: 2,
                }],
            },
            Event::Health {
                drives: vec![Health {
                    drive_id: "/d".into(),
                    device: None,
                    model: "M".into(),
                    temperature_c: Some(41.9),
                    power_on_hours: None,
                    failing: false,
                    warnings: vec![],
                    updated: None,
                }],
            },
            Event::Error {
                message: "udisks2 connection lost".into(),
            },
        ];
        for event in events {
            let line = serde_json::to_string(&event).unwrap();
            assert!(!line.contains('\n'));
            let back: Event = serde_json::from_str(&line).unwrap();
            assert_eq!(back, event);
        }
    }

    #[test]
    fn kinds_parse_subsets_and_reject_unknown_or_empty() {
        assert_eq!(Kinds::parse("volumes,io,health").unwrap(), Kinds::ALL);
        assert_eq!(
            Kinds::parse(" io ").unwrap(),
            Kinds {
                volumes: false,
                io: true,
                health: false
            }
        );
        assert!(Kinds::parse("io,disk")
            .unwrap_err()
            .to_string()
            .contains("disk"));
        assert!(Kinds::parse("").is_err());
        assert!(Kinds::parse(",").is_err());
    }

    #[test]
    fn reconnect_backs_off_until_udisks2_answers() {
        let start = Instant::now();
        let mut r = Reconnect::starting(start);
        assert!(r.due(start));
        r.failed(start);
        assert_eq!(r.at, Some(start + SEC));
        r.failed(start + SEC);
        assert_eq!(r.at, Some(start + 3 * SEC));
        r.failed(start + 3 * SEC);
        assert_eq!(r.at, Some(start + 7 * SEC));
        assert!(!r.due(start + 6 * SEC));
        assert!(r.succeeded());
        assert_eq!(r.at, None);
        assert!(!r.succeeded());
        r.failed(start + 10 * SEC);
        assert_eq!(r.at, Some(start + 11 * SEC));
    }

    #[test]
    fn a_first_successful_connect_asks_for_no_refresh() {
        let mut r = Reconnect::starting(Instant::now());
        assert!(!r.succeeded());
    }

    #[test]
    fn default_intervals_match_the_protocol() {
        let i = Intervals::default();
        assert_eq!((i.io, i.health, i.usage), (SEC, 60 * SEC, 30 * SEC));
    }

    #[test]
    fn volumes_and_health_are_due_at_start_and_io_after_one_interval() {
        let start = Instant::now();
        let mut s = Schedule::new(start, Kinds::ALL, Intervals::default());
        assert_eq!(s.next_deadline(), Some(start));
        assert_eq!(s.take_due(start), vec![Job::Volumes, Job::Health]);
        assert_eq!(s.next_deadline(), Some(start + SEC));
        assert_eq!(s.take_due(start + SEC), vec![Job::Io]);
        assert_eq!(s.take_due(start + SEC), vec![]);
    }

    #[test]
    fn jobs_reschedule_one_interval_after_they_ran() {
        let start = Instant::now();
        let mut s = Schedule::new(start, Kinds::ALL, Intervals::default());
        s.take_due(start);
        let late = start + 2 * SEC + Duration::from_millis(500);
        assert_eq!(s.take_due(late), vec![Job::Io]);
        assert_eq!(s.next_deadline(), Some(late + SEC));
    }

    #[test]
    fn only_requested_kinds_are_scheduled() {
        let start = Instant::now();
        let kinds = Kinds {
            volumes: false,
            io: true,
            health: false,
        };
        let mut s = Schedule::new(start, kinds, Intervals::default());
        assert_eq!(s.take_due(start), vec![]);
        assert_eq!(s.take_due(start + 61 * SEC), vec![Job::Io]);
    }

    #[test]
    fn a_change_pulls_volumes_forward_and_a_burst_coalesces() {
        let start = Instant::now();
        let kinds = Kinds {
            volumes: true,
            io: false,
            health: false,
        };
        let mut s = Schedule::new(start, kinds, Intervals::default());
        s.take_due(start);
        assert_eq!(s.next_deadline(), Some(start + 30 * SEC));
        s.volumes_changed(start + 5 * SEC);
        s.volumes_changed(start + 5 * SEC + Duration::from_millis(100));
        assert_eq!(s.next_deadline(), Some(start + 5 * SEC + DEBOUNCE));
    }

    #[test]
    fn a_change_is_ignored_when_volumes_are_not_requested() {
        let start = Instant::now();
        let kinds = Kinds {
            volumes: false,
            io: true,
            health: false,
        };
        let mut s = Schedule::new(start, kinds, Intervals::default());
        s.volumes_changed(start);
        assert_eq!(s.next_deadline(), Some(start + SEC));
    }

    #[test]
    fn refresh_now_makes_volumes_and_health_due_again() {
        let start = Instant::now();
        let mut s = Schedule::new(start, Kinds::ALL, Intervals::default());
        s.take_due(start);
        let later = start + 3 * SEC;
        s.refresh_now(later);
        assert_eq!(s.take_due(later), vec![Job::Volumes, Job::Health, Job::Io]);
    }

    #[test]
    fn wait_wakes_on_the_deadline_without_a_change_stream() {
        let start = Instant::now();
        let wake = zbus::block_on(wait(Some(start + Duration::from_millis(20)), None));
        assert_eq!(wake, Wake::Timer);
        assert!(start.elapsed() >= Duration::from_millis(20));
    }

    #[test]
    fn wait_reports_a_change_before_the_deadline() {
        let mut changes: ChangeStream = Box::pin(futures_lite::stream::once(Change::ObjectsAdded));
        let wake = zbus::block_on(wait(Some(Instant::now() + 10 * SEC), Some(&mut changes)));
        assert_eq!(wake, Wake::Change);
    }

    #[test]
    fn wait_reports_an_ended_change_stream() {
        let mut changes: ChangeStream = Box::pin(futures_lite::stream::empty());
        let wake = zbus::block_on(wait(Some(Instant::now() + 10 * SEC), Some(&mut changes)));
        assert_eq!(wake, Wake::StreamEnded);
    }

    #[test]
    fn within_passes_a_ready_result_through() {
        let out = zbus::block_on(within(SEC, async { Ok::<u32, Error>(7) }));
        assert_eq!(out.unwrap(), 7);
    }

    #[test]
    fn within_turns_a_hang_into_a_dbus_error() {
        let hang = futures_lite::future::pending::<Result<u32>>();
        let out = zbus::block_on(within(Duration::from_millis(20), hang));
        assert!(matches!(out, Err(Error::Dbus(m)) if m.contains("did not answer")));
    }

    #[test]
    fn huge_intervals_do_not_panic() {
        let start = Instant::now();
        let huge = Intervals {
            io: Duration::MAX,
            health: Duration::MAX,
            usage: Duration::MAX,
        };
        let mut s = Schedule::new(start, Kinds::ALL, huge);
        assert_eq!(s.take_due(start), vec![Job::Volumes, Job::Health]);
        s.volumes_changed(start);
        let _ = s.next_deadline();
    }

    #[test]
    fn run_with_no_kinds_says_hello_and_returns() {
        let mut lines: Vec<String> = Vec::new();
        {
            let mut emit = |e: &Event| -> std::io::Result<()> {
                lines.push(serde_json::to_string(e).unwrap());
                Ok(())
            };
            let none = Kinds {
                volumes: false,
                io: false,
                health: false,
            };
            zbus::block_on(run(none, Intervals::default(), &mut emit));
        }
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("{\"event\":\"hello\""));
    }

    #[test]
    fn run_returns_when_the_consumer_is_gone_before_hello() {
        let mut emit =
            |_: &Event| -> std::io::Result<()> { Err(std::io::ErrorKind::BrokenPipe.into()) };
        zbus::block_on(run(Kinds::ALL, Intervals::default(), &mut emit));
    }
}
