// Based on Mirror for Unity's snapshot interpolation

use std::{collections::VecDeque, marker::PhantomData, time::Instant};

use crate::{ExponentialMovingAverage, Settings, Snapshot, linear_map};

/// Buffers snapshots as they come in from the network so that
/// they may be played back by a 'Playback' in live time, some
/// configured number of periods behind the live data on the
/// remote, such that network jitter may be accounted for.
///
/// `Buffer` and `Playback` are split in order to allow a caller
/// to insert snapshots and step the interpolator from two different
/// threads without a lock.
pub struct Buffer<T> {
    settings: &'static Settings,

    pub(crate) buf: VecDeque<T>,

    last_remote_time: f64,
    last_remote_instant: Instant,
    last_remote_counter: u128,

    /// Measure the network jitter to dynamically adjust the playback
    /// offset.
    ///
    /// A moving average of the time between the latest two packets
    pub remote_delta_time: ExponentialMovingAverage,
}

/// Playsback buffered snapshots in steady time, accelerating and
/// deccelerating the local timescale in order to stay in tune with
/// the remote remote timescale, also accounting for network jitter.
///
/// `Buffer` and `Playback` are split in order to allow a caller
/// to insert snapshots and step the interpolator from two different
/// threads without a lock.
pub struct Playback<T> {
    settings: &'static Settings,
    _phantom: PhantomData<T>,

    remote_counter: u128,

    /// Aims to be remote_time - BUF_OFFSET
    pub playback_time: f64,

    /// Rate at which time passes in order to maintain
    pub timescale: f64,

    /// Measure any drift between the local timescale and the remote timescale,
    /// in order to accelerate/deccelerate the local timescale to get back on
    /// track.
    ///
    /// A moving average of the difference between the actual playback time
    /// and the targetted playback time (x periods behind the remote time)
    pub catchup_time: ExponentialMovingAverage,

    /// A debugging measure of how much the last 10 seconds
    /// have relied on extrapolation, between 1.0 - all, and
    /// 0.0 - none. (None is healthy)
    pub db_extrapolating_ema: ExponentialMovingAverage,

    /// A debugging measure of how much the last 10 seconds
    /// have relied on clamping the local timescale, between
    /// 1.0 - all, and 0.0 - none. (None is healthy)
    pub db_clamping_ema: ExponentialMovingAverage,

    /// A debugging measure of how much the last 10 seconds
    /// have relied on time scaling, between 1.0 - all, and
    /// 0.0 - none. (None is healthy, some is expected)
    pub db_scaling_ema: ExponentialMovingAverage,
}

impl<T: Snapshot> Buffer<T> {
    pub fn new(settings: &'static Settings) -> Self {
        let send_rate = 1.0 / (settings.period as f64 / 1000.0);

        Self {
            settings,

            buf: VecDeque::with_capacity(settings.buf_size),
            last_remote_time: 0.0,
            last_remote_instant: Instant::now(),
            last_remote_counter: 0,

            remote_delta_time: ExponentialMovingAverage::new(
                send_rate * settings.dynamic_playback_jitter_duration as f64,
            ),
        }
    }

    /// Retrieve the latest snapshot
    pub fn latest(&self) -> Option<&T> {
        self.buf.front()
    }

    /// Insert a new snapshot from the net
    pub fn insert_snapshot(&mut self, snapshot: T) {
        // 2. Insert snapshot
        self.insert(snapshot);

        let mut buf_iter = self.buf.iter();
        if let Some(ss_to) = buf_iter.next() {
            // 3. Add snapshot delta time to moving average
            // (Assumes that the received snapshot went to the front of the buf)
            if let Some(ss_from) = buf_iter.next() {
                let delta_time = ss_to.remote_time() - ss_from.remote_time();
                self.remote_delta_time.add(delta_time);
            }

            self.last_remote_instant = Instant::now();
            self.last_remote_time = ss_to.remote_time();
            self.last_remote_counter = self.last_remote_counter.wrapping_add(1);
        }
    }

    /// Compute the playback offset dynamically to adjust for
    /// measured network jitter. Exposed publically for debugging.
    pub fn dynamic_playback_offset(&self) -> f64 {
        let playback_offset = self.settings.playback_offset() as f64;

        if self.settings.dynamic_playback_time {
            // Account for recent network jitter
            playback_offset + self.remote_delta_time.std_dev
        } else {
            playback_offset
        }
    }

    /// Insert a snapshot into the buffer, maintaining the buffer size,
    /// the correct order and skipping duplicates.
    fn insert(&mut self, item: T) {
        if self
            .buf
            .iter()
            .any(|b| b.remote_time() == item.remote_time())
        {
            //Skip duplicates
            tracing::debug!("skipping duplicate position");
            return;
        }

        if let Some(position) = self
            .buf
            .iter()
            .position(|b| b.remote_time() < item.remote_time())
        {
            self.buf.insert(position, item);

            if self.buf.len() > self.settings.buf_size {
                self.buf.pop_back();
            }
        } else if self.buf.is_empty() {
            self.buf.insert(0, item);
        } else {
            tracing::debug!("packet too old");
        }
    }
}

impl<T: Snapshot> Playback<T> {
    pub fn new(buf: &Buffer<T>) -> Self {
        let settings = buf.settings;
        let send_rate = settings.send_rate();

        Self {
            settings,
            _phantom: PhantomData,

            remote_counter: buf.last_remote_counter,
            playback_time: 0.0,
            timescale: 1.0,

            catchup_time: ExponentialMovingAverage::new(send_rate), // 1 seconds worth of duration,
            db_extrapolating_ema: ExponentialMovingAverage::new(send_rate * 10.0), // 10 seconds worth of duration,
            db_clamping_ema: ExponentialMovingAverage::new(send_rate * 10.0), // 10 seconds worth of duration,
            db_scaling_ema: ExponentialMovingAverage::new(send_rate * 10.0), // 10 seconds worth of duration,
        }
    }

    /// Draw a new interpolated snapshot by passing in how much time
    /// has passed since the last step (seconds).
    pub fn step(&mut self, delta_time: f64, buf: &Buffer<T>) -> Option<T> {
        let playback_offset = buf.dynamic_playback_offset();
        let playback_clamp = self.settings.playback_clamp() as f64;

        // 1. Step playback time
        self.playback_time += delta_time * self.timescale;

        if self.remote_counter != buf.last_remote_counter {
            self.remote_counter = buf.last_remote_counter;
            // A new network packet has arrived into the buffer

            // 2. Clamp playback time about the target time +- the configured playback_clamp
            let remote_time = buf.last_remote_time
                // Account for any time which has passed since we, the local client, first
                // saw this packet arrive in the buffer.
                + buf.last_remote_instant.elapsed().as_millis() as f64;
            let playback_target_time = remote_time - playback_offset;
            let clamped_playback_time = self.playback_time.clamp(
                playback_target_time - playback_clamp,
                playback_target_time + playback_clamp,
            );

            if self.playback_time == clamped_playback_time {
                self.db_clamping_ema.add(0.0);
            } else {
                self.db_clamping_ema.add(1.0);
            }

            self.playback_time = clamped_playback_time;

            // 3. Add catchup time to moving average
            let catchup_time = playback_target_time - self.playback_time;
            self.catchup_time.add(catchup_time);

            // 4. Correct the timescale in order to best track the remote's timescale
            self.timescale = self.timescale(self.catchup_time.value.unwrap_or(0.0));
        }

        // Now the actual interpolation:
        // 5. Find the packets which playback time must be between
        let ss_from_pos = buf
            .buf
            .iter()
            .position(|b| b.remote_time() < self.playback_time);
        if let Some((ss_from, ss_to)) = match ss_from_pos {
            Some(0) => {
                self.db_extrapolating_ema.add(1.0);

                let ss_to = buf.buf.get(0);
                let ss_from = buf.buf.get(1);
                match (ss_from, ss_to) {
                    (Some(ss_from), Some(ss_to)) => {
                        debug_assert!(self.playback_time >= ss_from.remote_time());
                        debug_assert!(self.playback_time >= ss_to.remote_time());

                        Some((ss_from, ss_to))
                    }
                    _ => None,
                }
            }
            Some(ss_from_pos) => {
                self.db_extrapolating_ema.add(0.0);

                let ss_to_pos = ss_from_pos - 1;
                let ss_from = buf.buf.get(ss_from_pos);
                let ss_to = buf.buf.get(ss_to_pos);
                match (ss_from, ss_to) {
                    (Some(ss_from), Some(ss_to)) => {
                        debug_assert!(self.playback_time <= ss_to.remote_time());
                        debug_assert!(self.playback_time >= ss_from.remote_time());

                        Some((ss_from, ss_to))
                    }
                    _ => None,
                }
            }
            _ => None,
        } {
            let t = linear_map(
                self.playback_time,
                ss_from.remote_time(),
                ss_to.remote_time(),
                0.0,
                1.0,
            );
            tracing::trace!(?ss_from_pos, "{}", t);

            Some(Snapshot::interpolate(t.clamp(0.0, 2.5), ss_from, ss_to))
        } else {
            None
        }
    }

    pub fn timescale(&mut self, catchup_time: f64) -> f64 {
        if catchup_time < self.settings.slow_threshold() as f64 {
            self.db_scaling_ema.add(1.0);
            return self.settings.playback_slow_speed as f64;
        }

        if catchup_time > self.settings.fast_threshold() as f64 {
            self.db_scaling_ema.add(1.0);
            return self.settings.playback_fast_speed as f64;
        }

        self.db_scaling_ema.add(0.0);
        1.0
    }
}
