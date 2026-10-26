use std::{sync::LazyLock, time::Duration};

#[derive(Clone)]
pub struct Settings {
    /// The number of seconds worth of packets to store in the
    /// buffer
    pub buf_duration: Duration,

    /// The expected time period between the server sending
    /// any two snapshots.
    pub period: Duration,

    /// Dynamically adjust the playback offset to adjust for measured
    /// network jitter. i.e. When the network becomes jittery, slow the
    /// playback such that jitter is more easily accounted for. When
    /// the network jitter eases, speed back up.
    pub dynamic_playback_time: bool,

    /// When dynamic playback is enabled, this configures the window
    /// in seconds of packets from which to measure network jitter.
    /// i.e. When set to 2, the network jitter is calculated from the
    /// last 2 seconds of received packets.
    pub dynamic_playback_jitter_duration: Duration,

    /// How far behind should the playback be? In multiples of the period
    pub playback_offset_periods: f32,

    /// Clamp the playback time this multiple of the time period
    /// about the targetted playback time
    pub playback_clamp_periods: f32,

    /// Begin decelerating the playback when the playback time is
    /// this many multiples of the time period ahead of the target
    /// time. (i.e. 1.05 begins decelerating at 5% past the target playback time)
    pub playback_slow_periods: f32,
    /// The factor by which playback time should be decelerated
    /// when playback time is decelerating
    pub playback_slow_speed: f32,

    /// Begin accelerating the playback when the playback time is
    /// this many multiples of the time period behind of the target
    /// time (i.e. 0.95 begins accelerating at 5% behind the target playback time)
    pub playback_fast_periods: f32,
    /// The factor by which playback time should be accelerated
    /// when playback time is accelerating
    pub playback_fast_speed: f32,
}

pub static SNAPSHOT_SETTINGS_DEFAULT: LazyLock<Settings> = LazyLock::new(Settings::default);

impl Default for Settings {
    fn default() -> Self {
        Settings {
            buf_duration: Duration::from_secs(2),
            period: Duration::from_millis(200),

            dynamic_playback_time: true,
            dynamic_playback_jitter_duration: Duration::from_secs(2),

            playback_clamp_periods: 1.0,
            playback_fast_periods: 0.5,
            playback_fast_speed: 1.0 + 0.02,

            playback_slow_periods: -0.5,
            playback_slow_speed: 1.0 - 0.04,

            playback_offset_periods: 1.0,
        }
    }
}

impl Settings {
    pub fn playback_offset(&self) -> Duration {
        self.period.mul_f32(self.playback_offset_periods)
    }

    pub fn playback_clamp(&self) -> Duration {
        self.period.mul_f32(self.playback_clamp_periods)
    }

    pub fn fast_threshold(&self) -> Duration {
        self.period.mul_f32(self.playback_fast_periods)
    }

    pub fn slow_threshold(&self) -> Duration {
        self.period.mul_f32(-1.0 * self.playback_slow_periods)
    }

    /// (expected) packets per second
    pub fn send_rate(&self) -> f64 {
        1.0 / (self.period.as_secs_f64())
    }
}
