use num_traits::{Euclid, Float};

pub trait Snapshot: Clone {
    fn interpolate(t: f64, from: &Self, to: &Self) -> Self;

    /// Indicate the time, in seconds, at which this packet was
    /// constructed by the remote. Only used to track the passing of
    /// the remote's time, so from which point this is measured doesn't
    /// matter, so long as it's consistent.
    fn remote_time(&self) -> f64;
}

/// Interpolate an angle in degrees always taking the shortest distance around a circle
// TODO: Could be much better branch prediction wise?
pub fn lerp_angle<F: Float + Euclid>(a: F, b: F, mut t: F) -> F {
    let mut low = a;
    let mut high = b;
    let mut delta = high - low;
    if delta > (F::from(180.0)).unwrap() {
        t = F::one() + (t * -F::one());
        low = b;
        high = a + F::from(360.0).unwrap();
        delta = high - low;
    } else if delta < -F::from(180.0).unwrap() {
        low = a;
        high = b + F::from(360.0).unwrap();
        delta = high - low;
    }
    (low + (t * delta)).rem_euclid(&F::from(360.0).unwrap())
}

pub fn lerp<F: Float>(a: F, b: F, t: F) -> F {
    a + (t * (b - a))
}

pub fn linear_map<F: Float>(x: F, a: F, b: F, c: F, d: F) -> F {
    c + (x - a) * (d - c) / (b - a)
}

#[cfg(test)]
mod tests {
    use crate::snapshot::{lerp, lerp_angle, linear_map};

    #[test]
    fn linear_map_test() {
        assert_eq!(linear_map(-3000.0, -3000.0, 3000.0, 0.0, 1.0), 0.0);
        assert_eq!(linear_map(0.0, -3000.0, 3000.0, 0.0, 1.0), 0.5);
        assert_eq!(linear_map(3000.0, -3000.0, 3000.0, 0.0, 1.0), 1.0);
        assert_eq!(linear_map(6000.0, -3000.0, 3000.0, 0.0, 1.0), 1.5);
        assert_eq!(linear_map(6000.0, 0.0, 3000.0, 0.0, 1.0), 2.0);
    }

    #[test]
    fn lerp_test() {
        assert_eq!(lerp(0.0, 4.0, 1.0), 4.0);
        assert_eq!(lerp(0.0, 4.0, 0.0), 0.0);
        assert_eq!(lerp(0.0, 4.0, 0.5), 2.0);
        assert_eq!(lerp(0.0, 4.0, 0.25), 1.0);
        assert_eq!(lerp(0.0, 4.0, 0.75), 3.0);
        assert_eq!(lerp(0.0, 4.0, -2.0), -8.0);
        assert_eq!(lerp(0.0, 4.0, 2.0), 8.0);

        assert_eq!(lerp(4.0, 0.0, 1.0), 0.0);
        assert_eq!(lerp(4.0, 0.0, 0.0), 4.0);
        assert_eq!(lerp(4.0, 0.0, 0.5), 2.0);
        assert_eq!(lerp(4.0, 0.0, 0.25), 3.0);
        assert_eq!(lerp(4.0, 0.0, 0.75), 1.0);
        assert_eq!(lerp(4.0, 0.0, -2.0), 12.0);
        assert_eq!(lerp(4.0, 0.0, 2.0), -4.0);
    }

    #[test]
    fn heading_test() {
        //Normal lerp (without any negatives though)
        assert_eq!(lerp_angle(0.0, 4.0, 1.0), 4.0);
        assert_eq!(lerp_angle(0.0, 4.0, 0.0), 0.0);
        assert_eq!(lerp_angle(0.0, 4.0, 0.5), 2.0);
        assert_eq!(lerp_angle(0.0, 4.0, 0.25), 1.0);
        assert_eq!(lerp_angle(0.0, 4.0, 0.75), 3.0);
        assert_eq!(lerp_angle(0.0, 4.0, 2.0), 8.0);

        assert_eq!(lerp_angle(4.0, 0.0, 1.0), 0.0);
        assert_eq!(lerp_angle(4.0, 0.0, 0.0), 4.0);
        assert_eq!(lerp_angle(4.0, 0.0, 0.5), 2.0);
        assert_eq!(lerp_angle(4.0, 0.0, 0.25), 3.0);
        assert_eq!(lerp_angle(4.0, 0.0, 0.75), 1.0);
        assert_eq!(lerp_angle(4.0, 0.0, -2.0), 12.0);

        //Heading lerp
        assert_eq!(lerp_angle(350.0, 40.0, 0.5), 15.0);
        assert_eq!(
            lerp_angle(350.0, 40.0, 0.2),
            lerp(350.0, 40.0 + 360.0, 0.2) - 360.0
        );
        assert_eq!(lerp_angle(350.0, 40.0, 0.2), 0.0);
        assert_eq!(lerp_angle(350.0, 40.0, 0.1), 355.0);

        assert_eq!(lerp_angle(40.0, 350.0, 0.5), 15.0);
        assert_eq!(lerp_angle(40.0, 350.0, 0.2), 30.0);
        assert_eq!(lerp_angle(40.0, 350.0, 0.1), 35.0);
    }
}
