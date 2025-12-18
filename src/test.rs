#[cfg(test)]
mod tests {
    use crate::{SNAPSHOT_SETTINGS_DEFAULT, Snapshot};

    #[derive(Copy, Clone, Debug)]
    struct TestSnapshot {
        time: f64,
        number: usize,
    }

    impl Snapshot for TestSnapshot {
        fn interpolate(_: f64, _: &Self, to: &Self) -> Self {
            *to
        }

        fn remote_time(&self) -> f64 {
            self.time
        }
    }

    #[test]
    fn test_snapshot_insertion() {
        let mut buf = crate::Buffer::new(&*SNAPSHOT_SETTINGS_DEFAULT);
        // let mut play = snapshot::Playback::new(&buf);

        let one = TestSnapshot {
            time: 10.0,
            number: 1,
        };
        let two = TestSnapshot {
            time: 20.0,
            number: 2,
        };
        let three = TestSnapshot {
            time: 30.0,
            number: 3,
        };
        let four = TestSnapshot {
            time: 40.0,
            number: 4,
        };

        buf.insert_snapshot(one);
        buf.insert_snapshot(two);
        buf.insert_snapshot(four);
        buf.insert_snapshot(four);
        buf.insert_snapshot(three);

        assert_eq!(
            buf.buf.iter().map(|s| s.number).collect::<Vec<_>>(),
            vec![4, 3, 2, 1]
        );
    }
}
