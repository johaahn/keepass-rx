use std::ffi::CString;
use std::ffi::c_void;
use std::os::raw::{c_char, c_double};

const ZXCVBN_ESTIMATE_THRESHOLD: usize = 256;

unsafe extern "C" {
    fn ZxcvbnMatch(
        passwd: *const c_char,
        user_dict: *const *const c_char,
        info: *mut *mut c_void,
    ) -> c_double;
}

pub fn calculate_entropy(password: &str) -> f64 {
    if password.is_empty() {
        return 0.0;
    }

    let password_len = password.chars().count();
    let threshold_input: String = password.chars().take(ZXCVBN_ESTIMATE_THRESHOLD).collect();
    let threshold_input = threshold_input
        .split('\0')
        .next()
        .unwrap_or_default()
        .to_string();

    let threshold_input = match CString::new(threshold_input) {
        Ok(value) => value,
        Err(_) => return 0.0,
    };

    // SAFETY: We pass a valid, null-terminated C string. User dictionary
    // and info output are null pointers as permitted by ZxcvbnMatch API.
    let mut entropy = unsafe {
        ZxcvbnMatch(
            threshold_input.as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
        )
    };

    if password_len > ZXCVBN_ESTIMATE_THRESHOLD {
        // KeePassXC extends entropy for very long passwords by using
        // average entropy per character above the estimate threshold.
        let average_entropy_per_char = entropy / ZXCVBN_ESTIMATE_THRESHOLD as f64;
        entropy +=
            average_entropy_per_char * (password_len - ZXCVBN_ESTIMATE_THRESHOLD) as f64;
    }
    entropy
}

pub enum PasswordQuality {
    Bad,
    Poor,
    Weak,
    Good,
    Excellent,
}

impl From<f64> for PasswordQuality {
    fn from(entropy: f64) -> Self {
        match entropy {
            ent if ent <= 0.0 => PasswordQuality::Bad,
            ent if ent < 40.0 => PasswordQuality::Poor,
            ent if ent < 75.0 => PasswordQuality::Weak,
            ent if ent < 100.0 => PasswordQuality::Good,
            _ => PasswordQuality::Excellent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PasswordQuality, calculate_entropy};

    #[test]
    fn password_entropy_empty_is_zero() {
        assert_eq!(calculate_entropy(""), 0.0);
    }

    #[test]
    fn password_entropy_test_password_matches_kpxc_zxcvbn() {
        let entropy = calculate_entropy("test password");
        assert!((entropy - 16.17).abs() < 0.1);
    }

    #[test]
    fn password_quality_thresholds_match_keepassxc() {
        assert!(matches!(PasswordQuality::from(0.0), PasswordQuality::Bad));
        assert!(matches!(PasswordQuality::from(39.9), PasswordQuality::Poor));
        assert!(matches!(PasswordQuality::from(40.0), PasswordQuality::Weak));
        assert!(matches!(PasswordQuality::from(75.0), PasswordQuality::Good));
        assert!(matches!(
            PasswordQuality::from(100.0),
            PasswordQuality::Excellent
        ));
    }
}
