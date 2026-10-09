# Had gippity make up these distributions
# for modelling a good, decent and poor
# internet connection (by its ping)

import math
import numpy as np
import matplotlib.pyplot as plt

# Illustrative profiles, not empirical measurements.
# baseline_ms: minimum/base round-trip latency
# shape: Weibull shape parameter k
# mean_jitter_ms: mean additional delay
PROFILES = {
    "Good": {
        "baseline_ms": 15,
        "shape": 2.0,
        "mean_jitter_ms": 2,
    },
    "Decent": {
        "baseline_ms": 40,
        "shape": 1.5,
        "mean_jitter_ms": 8,
    },
    "Poor": {
        "baseline_ms": 80,
        "shape": 1.2,
        "mean_jitter_ms": 20,
    },
}

N = 20_000
TIME_SERIES_LENGTH = 300
rng = np.random.default_rng(42)


def sample_ping(profile, n):
    """Sample RTT ping values in milliseconds."""
    baseline = profile["baseline_ms"]
    k = profile["shape"]
    mean_jitter = profile["mean_jitter_ms"]

    # Select scale so the Weibull jitter has the desired mean.
    scale = mean_jitter / math.gamma(1 + (1 / k))

    # numpy's weibull() has unit scale.
    jitter = scale * rng.weibull(k, size=n)

    return baseline + jitter


samples = {}

for name, profile in PROFILES.items():
    ping = sample_ping(profile, N)
    samples[name] = ping

    print(f"\n{name} connection")
    print(f"  Baseline: {profile['baseline_ms']} ms")
    print(f"  Shape k:  {profile['shape']}")
    print(f"  Mean:     {np.mean(ping):.1f} ms")
    print(f"  Std dev:  {np.std(ping):.1f} ms")
    print(f"  Median:   {np.percentile(ping, 50):.1f} ms")
    print(f"  P95:      {np.percentile(ping, 95):.1f} ms")
    print(f"  P99:      {np.percentile(ping, 99):.1f} ms")
    print(f"  Max:      {np.max(ping):.1f} ms")

# Plot distributions and short sequences of ping measurements.
fig, axes = plt.subplots(3, 2, figsize=(13, 11))

for row, (name, ping) in enumerate(samples.items()):
    # Histogram
    ax = axes[row, 0]
    ax.hist(ping, bins=100, density=True, alpha=0.8)
    ax.set_title(f"{name}: ping distribution")
    ax.set_xlabel("Round-trip time (ms)")
    ax.set_ylabel("Density")
    ax.grid(alpha=0.25)

    # Example time series
    ax = axes[row, 1]
    series = ping[:TIME_SERIES_LENGTH]
    ax.plot(np.arange(len(series)), series, linewidth=1)
    ax.set_title(f"{name}: example ping sequence")
    ax.set_xlabel("Packet number")
    ax.set_ylabel("Ping (ms)")
    ax.grid(alpha=0.25)

plt.tight_layout()
plt.show()
