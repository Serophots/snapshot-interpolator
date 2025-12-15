#[derive(Clone)]
pub struct ExponentialMovingAverage {
    alpha: f64,
    pub var: f64,
    pub std_dev: f64,
    pub value: Option<f64>,
}

impl ExponentialMovingAverage {
    /// Traditionally `n` is taken as an integer. The window of
    /// historic values that are relevant to the moving average
    pub fn new(n: f64) -> ExponentialMovingAverage {
        ExponentialMovingAverage {
            alpha: 2.0 / (n + 1.0),
            var: 0.0,
            std_dev: 0.0,
            value: None,
        }
    }

    pub fn add(&mut self, v: f64) {
        if let Some(value) = self.value {
            let delta = v - value;
            self.value = Some(value + self.alpha * delta);
            self.var = (1.0 - self.alpha) * (self.var + self.alpha * delta * delta);
            self.std_dev = self.var.sqrt();
        } else {
            self.value = Some(v);
        }
    }

    pub fn reset(&mut self) {
        self.value = None;
        self.var = 0.0;
        self.std_dev = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use crate::ExponentialMovingAverage;

    #[test]
    fn test_ema() {
        let mut ema = ExponentialMovingAverage::new(10.0);
        ema.add(3.0);

        assert_eq!(ema.value, Some(3.0));
        assert_eq!(ema.var, 0.0);

        ema.reset();
        ema.add(5.0);
        ema.add(6.0);

        assert_eq!((ema.value.unwrap() * 10000.0).round(), 51818.0);
        assert_eq!((ema.var * 10000.0).round(), 1488.0);

        ema.reset();
        ema.add(5.0);
        ema.add(6.0);
        ema.add(7.0);

        assert_eq!((ema.var * 10000.0).round(), 6135.0);

        ema.reset();
        ema.add(5.0);
        ema.add(600.0);
        ema.add(70.0);

        assert_eq!((ema.std_dev * 10000.0).round(), 2082470.0);
    }
}
