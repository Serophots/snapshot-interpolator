// Based on Mirror for Unity's snapshot interpolation

use std::collections::VecDeque;

use crate::{ExponentialMovingAverage, Snapshot, SnapshotSettings, linear_map};

#[derive(Clone)]
pub struct SnapshotInterpolation<T> {
    settings: &'static SnapshotSettings,

    /// Buffer of interpolatables, ordered by remote time
    pub(crate) buf: VecDeque<T>,

    /// Aims to be remote_time - BUF_OFFSET
    pub playback_time: f64,
    pub remote_time: f64,

    /// Rate at which time passes in order to maintain
    pub timescale: f64,

    /// Measure the jitter in delta times
    pub remote_delta_time: ExponentialMovingAverage,
    pub catchup_time: ExponentialMovingAverage,
    pub extrapolating_ema: ExponentialMovingAverage,
}

impl<T> SnapshotInterpolation<T> {
    pub fn new(settings: &'static SnapshotSettings) -> Self {
        let send_rate = 1.0 / (settings.period as f64 / 1000.0);

        Self {
            settings,

            buf: VecDeque::new(),

            playback_time: 0.0,
            remote_time: 0.0,

            timescale: 1.0,

            remote_delta_time: ExponentialMovingAverage::new(
                send_rate * settings.dynamic_playback_jitter_duration as f64,
            ),
            catchup_time: ExponentialMovingAverage::new(send_rate), // 1 seconds worth of duration
            extrapolating_ema: ExponentialMovingAverage::new(send_rate * 10.0), // 10 seconds worth of duration
        }
    }
}

impl<T: Snapshot> SnapshotInterpolation<T> {
    /// Retrieve the latest snapshot
    pub fn latest(&self) -> Option<&T> {
        self.buf.front()
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

    /// Compute the playback offset dynamically to adjust for
    /// measured network jitter
    pub fn dynamic_playback_offset(&self) -> f64 {
        let playback_offset = self.settings.playback_offset() as f64;

        if self.settings.dynamic_playback_time {
            // Account for recent network jitter
            playback_offset + self.remote_delta_time.std_dev
        } else {
            playback_offset
        }
    }

    /// Insert a new snapshot from the net
    pub fn insert_snapshot(&mut self, snapshot: T) {
        // 1. Recompute the dynamic playback_offset to dynamically acount for jitter
        let playback_offset = self.dynamic_playback_offset();
        let playback_clamp = self.settings.playback_clamp() as f64;

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

            // 4. Clamp playback time about the target time +- the configured playback_clamp
            self.remote_time = ss_to.remote_time();
            let playback_target_time = ss_to.remote_time() - playback_offset;
            self.playback_time = self.playback_time.clamp(
                playback_target_time - playback_clamp,
                playback_target_time + playback_clamp,
            );

            // 4. Add catchup time to moving average
            let catchup_time = playback_target_time - self.playback_time;
            self.catchup_time.add(catchup_time);

            // 5. Compute our timescale using the smoothed catchup time
            self.timescale = self.timescale(self.catchup_time.value.unwrap());
        }
    }

    fn timescale(&self, catchup_time: f64) -> f64 {
        if catchup_time < self.settings.slow_threshold() as f64 {
            tracing::trace!(catchup_time, "decel");
            return self.settings.playback_slow_speed as f64;
        }

        if catchup_time > self.settings.fast_threshold() as f64 {
            tracing::trace!(catchup_time, "accel");
            return self.settings.playback_fast_speed as f64;
        }

        tracing::trace!(catchup_time, "constant");
        1.0
    }

    /// Draw a new interpolated snapshot by passing in how much time
    /// has passed since the last step.
    pub fn step(&mut self, delta_time: f64) -> Option<T> {
        //1. Step time
        self.playback_time += delta_time * self.timescale;

        //2. Find the packets which playback time must be between
        let ss_from_pos = self
            .buf
            .iter()
            .position(|b| b.remote_time() < self.playback_time);
        if let Some((ss_from, ss_to)) = match ss_from_pos {
            Some(0) => {
                // 3x sensitivity :)
                self.extrapolating_ema.add(0.0);
                self.extrapolating_ema.add(0.0);
                self.extrapolating_ema.add(0.0);
                tracing::debug!("extrapolating");

                let ss_to = self.buf.get(0);
                let ss_from = self.buf.get(1);
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
                self.extrapolating_ema.add(1.0);

                let ss_to_pos = ss_from_pos - 1;
                let ss_from = self.buf.get(ss_from_pos);
                let ss_to = self.buf.get(ss_to_pos);
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
}
