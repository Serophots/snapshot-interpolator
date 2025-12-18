extern crate sdl3;

use rand::Rng;
use rand_distr::{Distribution, Normal};
use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::pixels::Color;
use sdl3::rect::Rect;
use std::{
    sync::LazyLock,
    time::{Duration, Instant},
};

#[derive(Clone)]
struct Position {
    x: f64,
    y: f64,
    remote_time: f64,
}

impl snapshot::Snapshot for Position {
    fn interpolate(t: f64, from: &Self, to: &Self) -> Self {
        Position {
            x: snapshot::lerp(from.x, to.x, t),
            y: snapshot::lerp(from.y, to.y, t),
            remote_time: 0.0,
        }
    }

    fn remote_time(&self) -> f64 {
        self.remote_time
    }
}

static SETTINGS: LazyLock<snapshot::Settings> = LazyLock::new(|| snapshot::Settings {
    // playback_clamp_periods: 3.0,
    // playback_fast_speed: 1.0 + 0.02,
    // playback_slow_speed: 1.0 - 0.02,
    dynamic_playback_time: false,
    // playback_offset_periods: 0.2,
    ..Default::default()
});

#[allow(dead_code)]
enum NetState {
    Instant,
    Good,
    Fair,
    Far,
    Poor,
}

const NET_STATE: NetState = NetState::Poor;

impl NetState {
    fn sample_ping<R: Rng + ?Sized>(&self, rng: &mut R) -> f64 {
        match self {
            NetState::Instant => 0.0,
            NetState::Good => Normal::new(10.0, 2.0).unwrap().sample(rng),
            NetState::Fair => Normal::new(80.0, 20.0).unwrap().sample(rng),
            NetState::Far => Normal::new(250.0, 60.0).unwrap().sample(rng),
            NetState::Poor => Normal::new(300.0, 150.0).unwrap().sample(rng),
        }
    }

    fn droprate(&self) -> f64 {
        match self {
            NetState::Instant => 0.0,
            NetState::Good => 0.001,
            NetState::Fair => 0.005,
            NetState::Far => 0.015,
            NetState::Poor => 0.2,
        }
    }
}

fn main() {
    snapshot_example();

    // test_clock_drift();
}

#[allow(dead_code)]
fn test_clock_drift() {
    let start = Instant::now();

    let mut playback_time = 0.0;
    let mut last_net = Instant::now();

    loop {
        let now = Instant::now();
        let elapsed = now.duration_since(last_net);
        if elapsed >= Duration::from_millis(16) {
            last_net = now;
            playback_time += elapsed.as_secs_f64();

            println!(
                "in {}s delta {}ms",
                start.elapsed().as_secs(),
                (start.elapsed().as_secs_f64() - playback_time) * 1000.0
            );
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}

fn snapshot_example() {
    let mut buf = snapshot::Buffer::<Position>::new(&*SETTINGS);
    let mut play = snapshot::Playback::new(&buf);

    let sdl_context = sdl3::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("snapshot interpolation demo", 800, 600)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas();

    canvas.set_draw_color(Color::RGB(0, 255, 255));
    canvas.clear();
    canvas.present();

    let mut event_pump = sdl_context.event_pump().unwrap();

    const CUBE_SIZE: i32 = 50;
    const CUBE_SPEED: f32 = 3.0;

    let mut green_rect = Rect::new(0, 0, CUBE_SIZE as u32, CUBE_SIZE as u32);
    let mut red_rect = Rect::new(
        (800 - CUBE_SIZE) / 2,
        (600 - CUBE_SIZE) / 2,
        CUBE_SIZE as u32,
        CUBE_SIZE as u32,
    );
    let mut dx: f32 = 0.0;
    let mut dy: f32 = 0.0;

    let start = Instant::now();
    let mut last_snapshot_send = Instant::now();
    let mut last_step_time = Instant::now();

    let mut pipeline_snapshots = Vec::new();

    let mut rng = rand::rng();

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(keycode),
                    ..
                } => match keycode {
                    Keycode::Left => {
                        dx = -CUBE_SPEED;
                        // dy = 0;
                    }
                    Keycode::Right => {
                        dx = CUBE_SPEED;
                        // dy = 0;
                    }
                    Keycode::Up => {
                        dy = -CUBE_SPEED;
                        // dx = 0;
                    }
                    Keycode::Down => {
                        dy = CUBE_SPEED;
                        // dx = 0;
                    }
                    _ => {}
                },

                _ => {}
            }
        }

        // Update cube position
        red_rect.x += dx as i32;
        red_rect.y += dy as i32;

        let now = Instant::now();
        let delta_time = now.duration_since(last_step_time);
        last_step_time = now;

        if let Some(pos) = play.step(delta_time.as_secs_f64(), &buf) {
            green_rect.x = pos.x.round() as i32;
            green_rect.y = pos.y.round() as i32;
        }

        // Implement bouncing behavior for the red cube
        if red_rect.x < 0 {
            red_rect.x = 0;
            dx = -dx;
        } else if red_rect.x > 800 - CUBE_SIZE {
            red_rect.x = 800 - CUBE_SIZE;
            dx = -dx;
        }

        if red_rect.y < 0 {
            red_rect.y = 0;
            dy = -dy;
        } else if red_rect.y > 600 - CUBE_SIZE {
            red_rect.y = 600 - CUBE_SIZE;
            dy = -dy;
        }

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();

        canvas.set_draw_color(Color::RGB(0, 255, 0));
        canvas.fill_rect(green_rect).unwrap();

        canvas.set_draw_color(Color::RGB(255, 0, 0));
        canvas.fill_rect(red_rect).unwrap();

        canvas.present();

        for _ in 0..4 {
            // Send a snapshot of the cube's position
            if last_snapshot_send.elapsed().as_secs_f64() >= SETTINGS.period {
                last_snapshot_send = Instant::now();
                // Simulate the remote sending a snapshot
                pipeline_snapshots.push((
                    Position {
                        x: red_rect.x as f64,
                        y: red_rect.y as f64,
                        remote_time: start.elapsed().as_secs_f64(),
                    },
                    Instant::now(),
                    Duration::from_millis(NET_STATE.sample_ping(&mut rng) as u64),
                ));
            }

            pipeline_snapshots.retain(|snapshot| {
                if snapshot.1.elapsed() >= snapshot.2 {
                    // Simulate the local client receiving this snapshot

                    if !rng.random_bool(NET_STATE.droprate()) {
                        buf.insert_snapshot(snapshot.0.clone());
                    }

                    return false;
                }

                true
            });

            ::std::thread::sleep(Duration::from_millis(2));
        }

        println!(
            "dbg_extrapolating {} (GOOD 0 - 10 BAD)",
            (play.db_extrapolating_ema.value.unwrap_or_default() * 10.0).round()
        );
        println!(
            "dbg_clamping      {} (GOOD 0 - 10 BAD)",
            (play.db_clamping_ema.value.unwrap_or_default() * 10.0).round()
        );
        println!(
            "dbg_scaling       {} (GOOD 0 - 10 BAD)",
            (play.db_scaling_ema.value.unwrap_or_default() * 10.0).round()
        );
        println!(
            "catchup time  ({} <=) {}ms (<= {}) - targets 0 ms (time scaling: {} - {})",
            -(SETTINGS.playback_clamp() * 1000.0),
            (play.catchup_time.value.unwrap_or_default() * 1000.0).round(),
            SETTINGS.playback_clamp() * 1000.0,
            SETTINGS.slow_threshold() * 1000.0,
            SETTINGS.fast_threshold() * 1000.0,
        );
        println!(
            "dyn playback time {}ms",
            (buf.dynamic_playback_offset() * 1000.0).round()
        );
        println!(
            "remote delta time {}ms - targets time period + latency (+ frame time)",
            (buf.remote_delta_time.value.unwrap_or_default() * 1000.0).round()
        );
        println!("timescale     {}", play.timescale);
    }
}
