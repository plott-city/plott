/// Statistical distribution helpers for pattern analysis.

/// Compute the arithmetic mean of a slice.
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Compute the standard deviation of a slice.
pub fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let avg = mean(values);
    let variance = values.iter().map(|v| (v - avg).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}

/// Compute the percentile rank of a value within a sorted distribution.
pub fn percentile_rank(sorted_values: &[f64], value: f64) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }
    let count_below = sorted_values.iter().filter(|&&v| v < value).count();
    let count_equal = sorted_values.iter().filter(|&&v| (v - value).abs() < f64::EPSILON).count();
    (count_below as f64 + 0.5 * count_equal as f64) / sorted_values.len() as f64 * 100.0
}

/// Compute the z-score of a value given mean and standard deviation.
pub fn z_score(value: f64, mean_val: f64, std_val: f64) -> f64 {
    if std_val.abs() < f64::EPSILON {
        return 0.0;
    }
    (value - mean_val) / std_val
}

/// Compute the simple moving average over a window.
pub fn moving_average(values: &[f64], window: usize) -> Vec<f64> {
    if window == 0 || values.len() < window {
        return Vec::new();
    }
    let mut result = Vec::with_capacity(values.len() - window + 1);
    let mut sum: f64 = values[..window].iter().sum();
    result.push(sum / window as f64);
    for i in window..values.len() {
        sum += values[i] - values[i - window];
        result.push(sum / window as f64);
    }
    result
}

/// Compute the exponential weighted moving average.
pub fn ewma(values: &[f64], alpha: f64) -> Vec<f64> {
    if values.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::with_capacity(values.len());
    result.push(values[0]);
    for i in 1..values.len() {
        let prev = result[i - 1];
        result.push(alpha * values[i] + (1.0 - alpha) * prev);
    }
    result
}

/// Compute the Pearson correlation coefficient between two slices.
pub fn correlation(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len().min(y.len());
    if n < 2 {
        return 0.0;
    }
    let mean_x = mean(&x[..n]);
    let mean_y = mean(&y[..n]);
    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;
    for i in 0..n {
        let dx = x[i] - mean_x;
        let dy = y[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }
    let denom = (var_x * var_y).sqrt();
    if denom.abs() < f64::EPSILON {
        return 0.0;
    }
    cov / denom
}

/// Compute a histogram from values with specified bin count.
pub fn histogram(values: &[f64], bins: usize) -> Vec<(f64, f64, usize)> {
    if values.is_empty() || bins == 0 {
        return Vec::new();
    }
    let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let range = max_val - min_val;
    if range.abs() < f64::EPSILON {
        return vec![(min_val, max_val, values.len())];
    }
    let bin_width = range / bins as f64;
    let mut counts = vec![0usize; bins];
    for &v in values {
        let idx = ((v - min_val) / bin_width).floor() as usize;
        let clamped = idx.min(bins - 1);
        counts[clamped] += 1;
    }
    counts
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            let lo = min_val + i as f64 * bin_width;
            let hi = lo + bin_width;
            (lo, hi, c)
        })
        .collect()
}

/// Compute the volatility index (coefficient of variation).
pub fn volatility_index(values: &[f64]) -> f64 {
    let m = mean(values);
    if m.abs() < f64::EPSILON {
        return 0.0;
    }
    std_dev(values) / m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mean() {
        assert!((mean(&[1.0, 2.0, 3.0, 4.0, 5.0]) - 3.0).abs() < f64::EPSILON);
        assert!((mean(&[]) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_std_dev() {
        let sd = std_dev(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
        assert!((sd - 2.138).abs() < 0.01);
    }

    #[test]
    fn test_percentile_rank() {
        let sorted = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let rank = percentile_rank(&sorted, 3.0);
        assert!((rank - 50.0).abs() < 1.0);
    }

    #[test]
    fn test_z_score() {
        let z = z_score(75.0, 70.0, 10.0);
        assert!((z - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_moving_average() {
        let ma = moving_average(&[1.0, 2.0, 3.0, 4.0, 5.0], 3);
        assert_eq!(ma.len(), 3);
        assert!((ma[0] - 2.0).abs() < f64::EPSILON);
        assert!((ma[2] - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ewma() {
        let result = ewma(&[1.0, 2.0, 3.0], 0.5);
        assert_eq!(result.len(), 3);
        assert!((result[0] - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_correlation() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let r = correlation(&x, &y);
        assert!((r - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_histogram() {
        let h = histogram(&[1.0, 2.0, 3.0, 4.0, 5.0], 5);
        assert_eq!(h.len(), 5);
    }

    #[test]
    fn test_volatility_index() {
        let vi = volatility_index(&[10.0, 10.0, 10.0]);
        assert!((vi - 0.0).abs() < f64::EPSILON);
    }
}

// add benchmark tests for large datasets

// correct bin boundary in histogram edge (#4)

// add negative value tests for z-score

// add weighted mean calculation

// guard against division by zero in volatility

// correct rounding in percentile rank

// normalize comment formatting in math lib
